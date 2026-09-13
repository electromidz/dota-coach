//! Steam login, logout, and the current-session endpoint.

use std::collections::BTreeMap;

use axum::extract::{RawQuery, State};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Json;
use axum_extra::extract::CookieJar;
use serde::Serialize;

use crate::api::extract::CurrentUser;
use crate::domain::session::{hash_token, LOGIN_STATE_COOKIE, SESSION_COOKIE};
use crate::domain::user::User;
use crate::error::{AppError, AppResult};
use crate::repositories;
use crate::services::auth::{
    self, constant_time_eq, establish_session, expired_login_state_cookie, expired_session_cookie,
    login_state_cookie, session_cookie,
};
use crate::state::AppState;

/// `GET /auth/steam/login` — start the OpenID flow.
///
/// A random nonce is stored in a short-lived cookie and echoed through
/// `return_to`, so an attacker cannot drive a victim's browser through a login
/// the victim did not start.
pub async fn login(State(state): State<AppState>, jar: CookieJar) -> AppResult<Response> {
    let nonce = crate::domain::session::NewToken::generate().plaintext;

    let redirect = state.steam.authorization_url(&nonce).map_err(|e| {
        tracing::error!(error = %e, "could not build the Steam login URL");
        AppError::Internal("steam login url".into())
    })?;

    let jar = jar.add(login_state_cookie(nonce, &state.config.auth));
    Ok((jar, Redirect::to(&redirect)).into_response())
}

/// `GET /auth/steam/callback` — Steam sends the user back here.
///
/// Every failure lands the browser back on the frontend with an `error` code
/// rather than rendering an API error page: this endpoint is reached by
/// top-level navigation, not by `fetch`.
pub async fn callback(
    State(state): State<AppState>,
    jar: CookieJar,
    RawQuery(query): RawQuery,
) -> Response {
    // Read the nonce before clearing it: `remove` takes it out of this jar too.
    let expected_nonce = jar.get(LOGIN_STATE_COOKIE).map(|c| c.value().to_string());
    let jar = jar.remove(expired_login_state_cookie(&state.config.auth));

    match complete_login(
        &state,
        expected_nonce.as_deref(),
        query.as_deref().unwrap_or_default(),
    )
    .await
    {
        Ok(token) => {
            let jar = jar.add(session_cookie(token, &state.config.auth));
            (jar, Redirect::to(&state.config.auth.frontend_base_url)).into_response()
        }
        Err(reason) => {
            tracing::warn!(reason = reason.log_detail(), "steam login failed");
            let target = format!(
                "{}/?error={}",
                state.config.auth.frontend_base_url,
                reason.code()
            );
            (jar, Redirect::to(&target)).into_response()
        }
    }
}

/// Why a login attempt did not complete. The variant name never reaches the
/// user; only the stable code in the redirect does.
enum LoginFailure {
    StateMismatch,
    AssertionRejected,
    SteamUnavailable(String),
    NoDotaAccount,
    Database(sqlx::Error),
}

impl LoginFailure {
    fn code(&self) -> &'static str {
        match self {
            LoginFailure::StateMismatch => "login_expired",
            LoginFailure::AssertionRejected => "steam_rejected",
            LoginFailure::SteamUnavailable(_) => "steam_unavailable",
            LoginFailure::NoDotaAccount => "no_dota_account",
            LoginFailure::Database(_) => "server_error",
        }
    }

    fn log_detail(&self) -> String {
        match self {
            LoginFailure::StateMismatch => "login nonce missing or mismatched".into(),
            LoginFailure::AssertionRejected => "steam did not validate the assertion".into(),
            LoginFailure::SteamUnavailable(e) => format!("steam unreachable: {e}"),
            LoginFailure::NoDotaAccount => "steam id has no usable Dota account".into(),
            LoginFailure::Database(e) => format!("database error: {e}"),
        }
    }
}

async fn complete_login(
    state: &AppState,
    expected_nonce: Option<&str>,
    raw_query: &str,
) -> Result<String, LoginFailure> {
    let params = parse_query(raw_query);

    // The nonce must match the cookie set when the flow started.
    let expected = expected_nonce.ok_or(LoginFailure::StateMismatch)?;
    let received = params.get("state").ok_or(LoginFailure::StateMismatch)?;
    if !constant_time_eq(expected, received) {
        return Err(LoginFailure::StateMismatch);
    }

    // Only Steam's own confirmation makes the claimed id trustworthy.
    let steam_id = state
        .steam_verifier
        .verify(&params)
        .await
        .map_err(|e| match e {
            crate::services::auth::steam_openid::OpenIdError::Unavailable(detail) => {
                LoginFailure::SteamUnavailable(detail)
            }
            _ => LoginFailure::AssertionRejected,
        })?;

    let logged_in = establish_session(&state.db, steam_id, state.config.auth.session_ttl_hours)
        .await
        .map_err(|e| match e {
            auth::AuthError::UnresolvableDotaAccount(_) => LoginFailure::NoDotaAccount,
            auth::AuthError::Database(e) => LoginFailure::Database(e),
        })?;

    tracing::info!(
        user_id = %logged_in.user.id,
        dota_account_id = logged_in.dota_player.dota_account_id,
        "steam login complete"
    );

    Ok(logged_in.token)
}

/// Steam returns its parameters in the query string; collect them verbatim
/// because verification must echo every one of them back.
fn parse_query(raw: &str) -> BTreeMap<String, String> {
    form_urlencoded::parse(raw.as_bytes())
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect()
}

#[derive(Serialize)]
pub struct SessionResponse {
    pub user: User,
}

/// `GET /api/auth/session` — who am I?
pub async fn session(CurrentUser(user): CurrentUser) -> AppResult<Json<SessionResponse>> {
    Ok(Json(SessionResponse { user }))
}

/// `POST /api/auth/logout` — destroy the session server-side and clear the cookie.
///
/// Deliberately succeeds even without a valid session: logging out is not an
/// operation that should ever fail for the caller.
pub async fn logout(State(state): State<AppState>, jar: CookieJar) -> AppResult<Response> {
    if let Some(cookie) = jar.get(SESSION_COOKIE) {
        repositories::session::delete_by_token_hash(&state.db, &hash_token(cookie.value())).await?;
    }

    let jar = jar.remove(expired_session_cookie(&state.config.auth));
    Ok((jar, Json(serde_json::json!({ "ok": true }))).into_response())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_parsing_keeps_every_openid_parameter() {
        let params = parse_query(
            "openid.mode=id_res&openid.sig=abc%2Bdef&state=nonce123&openid.signed=mode%2Csig",
        );

        assert_eq!(params.get("openid.mode").unwrap(), "id_res");
        // Percent-encoding must be undone before the values are echoed back.
        assert_eq!(params.get("openid.sig").unwrap(), "abc+def");
        assert_eq!(params.get("openid.signed").unwrap(), "mode,sig");
        assert_eq!(params.get("state").unwrap(), "nonce123");
    }

    #[test]
    fn an_empty_query_yields_no_parameters() {
        assert!(parse_query("").is_empty());
    }

    #[test]
    fn failure_codes_are_stable_and_leak_nothing() {
        let failures = [
            (LoginFailure::StateMismatch, "login_expired"),
            (LoginFailure::AssertionRejected, "steam_rejected"),
            (
                LoginFailure::SteamUnavailable("dns failure for steamcommunity.com".into()),
                "steam_unavailable",
            ),
            (LoginFailure::NoDotaAccount, "no_dota_account"),
        ];

        for (failure, expected) in failures {
            assert_eq!(failure.code(), expected);
            // The code is what reaches the browser; it must not carry detail.
            assert!(!failure.code().contains("dns"));
        }
    }
}

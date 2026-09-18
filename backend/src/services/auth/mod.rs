//! Authentication: Steam OpenID login and server-side sessions.

pub mod steam_openid;

use axum_extra::extract::cookie::{Cookie, SameSite};
use chrono::{Duration, Utc};
use sqlx::PgPool;

use crate::config::AuthConfig;
use crate::domain::event::EventType;
use crate::domain::player::{DotaPlayer, PlayerIdentity};
use crate::domain::session::{NewToken, LOGIN_STATE_COOKIE, SESSION_COOKIE};
use crate::domain::user::User;
use crate::repositories;
use crate::services::events;

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("steam id {0} has no usable Dota account")]
    UnresolvableDotaAccount(i64),
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

/// Everything a completed login needs to establish.
pub struct LoggedIn {
    pub user: User,
    pub dota_player: DotaPlayer,
    /// Plaintext session token. Goes into the cookie and is then discarded.
    pub token: String,
}

/// Turn a proven SteamID64 into an account, a linked Dota identity and a session.
///
/// Idempotent: logging in again reuses the same account and player rows and
/// only adds a session.
pub async fn establish_session(
    pool: &PgPool,
    steam_id: i64,
    ttl_hours: i64,
) -> Result<LoggedIn, AuthError> {
    // Derived from the authenticated Steam identity, never from a request body.
    let identity = PlayerIdentity::from_steam_id(steam_id)
        .map_err(|_| AuthError::UnresolvableDotaAccount(steam_id))?;

    let mut tx = pool.begin().await?;

    let user = repositories::user::upsert_by_steam_id(&mut tx, steam_id).await?;
    let dota_player = repositories::dota_player::link(&mut tx, user.id, identity).await?;

    let token = NewToken::generate();
    let expires_at = Utc::now() + Duration::hours(ttl_hours);
    repositories::session::create(&mut tx, user.id, &token.hash, expires_at).await?;

    tx.commit().await?;

    events::track(pool, user.id, EventType::Login, serde_json::json!({})).await;

    Ok(LoggedIn {
        user,
        dota_player,
        token: token.plaintext,
    })
}

/// Build the session cookie.
///
/// `HttpOnly` keeps it away from scripts, `SameSite=Lax` blocks cross-site
/// POSTs while still surviving the top-level redirect back from Steam, and
/// `Secure` is on wherever the deployment is HTTPS.
pub fn session_cookie<'a>(token: String, config: &AuthConfig) -> Cookie<'a> {
    let mut cookie = Cookie::new(SESSION_COOKIE, token);
    cookie.set_http_only(true);
    cookie.set_same_site(SameSite::Lax);
    cookie.set_secure(config.cookie_secure);
    cookie.set_path("/");
    cookie.set_max_age(time::Duration::hours(config.session_ttl_hours));
    cookie
}

/// A removal cookie for logout. Must match the original's path and flags or the
/// browser keeps the old one.
pub fn expired_session_cookie<'a>(config: &AuthConfig) -> Cookie<'a> {
    let mut cookie = Cookie::new(SESSION_COOKIE, "");
    cookie.set_http_only(true);
    cookie.set_same_site(SameSite::Lax);
    cookie.set_secure(config.cookie_secure);
    cookie.set_path("/");
    cookie.set_max_age(time::Duration::seconds(0));
    cookie
}

/// Short-lived cookie holding the login nonce, so the callback can prove the
/// flow started on this site rather than being replayed by an attacker.
pub fn login_state_cookie<'a>(state: String, config: &AuthConfig) -> Cookie<'a> {
    let mut cookie = Cookie::new(LOGIN_STATE_COOKIE, state);
    cookie.set_http_only(true);
    cookie.set_same_site(SameSite::Lax);
    cookie.set_secure(config.cookie_secure);
    cookie.set_path("/");
    cookie.set_max_age(time::Duration::minutes(10));
    cookie
}

pub fn expired_login_state_cookie<'a>(config: &AuthConfig) -> Cookie<'a> {
    let mut cookie = Cookie::new(LOGIN_STATE_COOKIE, "");
    cookie.set_http_only(true);
    cookie.set_same_site(SameSite::Lax);
    cookie.set_secure(config.cookie_secure);
    cookie.set_path("/");
    cookie.set_max_age(time::Duration::seconds(0));
    cookie
}

/// Constant-time comparison for the login nonce.
pub fn constant_time_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }

    a.bytes()
        .zip(b.bytes())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(secure: bool) -> AuthConfig {
        AuthConfig {
            public_base_url: "http://localhost:8080".into(),
            frontend_base_url: "http://localhost:3000".into(),
            steam_openid_url: "https://steamcommunity.com/openid/login".into(),
            session_ttl_hours: 720,
            cookie_secure: secure,
        }
    }

    #[test]
    fn the_session_cookie_is_not_readable_by_scripts() {
        let cookie = session_cookie("token".into(), &config(true));

        assert_eq!(cookie.http_only(), Some(true));
        assert_eq!(cookie.secure(), Some(true));
        assert_eq!(cookie.same_site(), Some(SameSite::Lax));
        assert_eq!(cookie.path(), Some("/"));
    }

    #[test]
    fn secure_follows_configuration_so_local_http_still_works() {
        assert_eq!(
            session_cookie("t".into(), &config(false)).secure(),
            Some(false)
        );
    }

    #[test]
    fn logout_clears_the_cookie() {
        let cookie = expired_session_cookie(&config(false));

        assert_eq!(cookie.value(), "");
        assert_eq!(cookie.max_age(), Some(time::Duration::seconds(0)));
    }

    #[test]
    fn nonce_comparison_rejects_mismatches_of_any_shape() {
        assert!(constant_time_eq("abc123", "abc123"));
        assert!(!constant_time_eq("abc123", "abc124"));
        assert!(!constant_time_eq("abc123", "abc12"));
        assert!(!constant_time_eq("", "a"));
        assert!(constant_time_eq("", ""));
    }
}

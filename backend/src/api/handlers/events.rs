//! Client-reported analytics. The only events the backend cannot observe on
//! its own, because they describe navigation inside the frontend rather than
//! anything that reaches the API.

use axum::extract::State;
use axum::Json;
use serde::Deserialize;
use utoipa::ToSchema;

use crate::api::extract::{AppJson, CurrentUser};
use crate::domain::event::EventType;
use crate::error::{AppError, AppResult};
use crate::services::events;
use crate::state::AppState;

/// Long enough for any real route in this app; short enough that a client
/// bug cannot turn one page view into an unbounded row.
const MAX_PATH_LEN: usize = 200;

#[derive(Deserialize, ToSchema)]
pub struct PageViewRequest {
    /// A frontend route, e.g. `/coach`. Never interpreted as a URL to fetch
    /// or a template — and never stored as sent, either: see `sanitize_path`.
    pub path: String,
}

/// Query strings and fragments are dropped before storage.
///
/// This is the one client-supplied string that reaches `events.metadata`,
/// and a route is never supposed to carry a secret — but a session token,
/// a Steam OpenID assertion, or a password-reset code living in `?...` is
/// exactly the kind of thing a route *does* end up carrying by accident.
/// Stripped here rather than trusted from the frontend, because this
/// boundary is the one place that can actually guarantee it.
fn sanitize_path(raw: &str) -> String {
    let no_query = raw.split(['?', '#']).next().unwrap_or("");
    no_query.chars().take(MAX_PATH_LEN).collect()
}

/// `POST /api/events/page-view`
#[utoipa::path(
    post, path = "/api/events/page-view", tag = "events",
    summary = "Record a page view",
    description = "The frontend calls this on navigation. Session-gated because every event row requires a user; there is no anonymous analytics path.",
    security(("session" = [])),
    request_body = PageViewRequest,
    responses(
        (status = 200, description = "Recorded (or silently dropped on a storage failure — tracking never fails the caller)"),
        (status = 401, description = "No session cookie, or it has expired", body = crate::error::ErrorBody),
    )
)]
pub async fn page_view(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    AppJson(body): AppJson<PageViewRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let path = sanitize_path(&body.path);
    if path.is_empty() {
        return Err(AppError::BadRequest("path must not be empty.".into()));
    }

    events::track(
        &state.db,
        user.id,
        EventType::PageView,
        serde_json::json!({ "path": path }),
    )
    .await;

    Ok(Json(serde_json::json!({ "ok": true })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_query_string_or_fragment_never_reaches_storage() {
        assert_eq!(sanitize_path("/coach"), "/coach");
        assert_eq!(sanitize_path("/reset?token=abc123"), "/reset");
        assert_eq!(sanitize_path("/coach#section"), "/coach");
        assert_eq!(sanitize_path("/auth/callback?openid.sig=xyz&state=1"), "/auth/callback");
    }

    #[test]
    fn an_unbounded_path_is_capped_rather_than_stored_whole() {
        let huge = "/".to_string() + &"a".repeat(1000);
        assert_eq!(sanitize_path(&huge).chars().count(), MAX_PATH_LEN);
    }

    #[test]
    fn truncation_never_splits_a_multibyte_character() {
        // Each `é` is two bytes in UTF-8; a byte-indexed slice at the limit
        // would panic here if it landed inside one.
        let path: String = std::iter::repeat_n('é', MAX_PATH_LEN + 5).collect();
        let sanitized = sanitize_path(&path);
        assert_eq!(sanitized.chars().count(), MAX_PATH_LEN);
    }
}

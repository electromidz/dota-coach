//! Coaching history: what was measured, and when.
//!
//! Sessions are written elsewhere — by a sync that brought in enough new games,
//! and by a generation that needed a snapshot to bind its analysis to. This
//! module only reads them, and it reads them **verbatim**: a stored session is
//! served exactly as it was recorded, never rehydrated with today's figures.
//! Rewriting session #1 with the numbers the player has now would erase the
//! improvement it exists to demonstrate.
//!
//! Every query is scoped through the session cookie to the caller's own player
//! row. No route accepts a user, player or session id it has not first proved
//! ownership of.

use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::api::extract::{AppPath, AppQuery, CurrentUser};
use crate::api::handlers::coach;
use crate::domain::coaching_session::{CoachingSession, SessionSummary};
use crate::domain::player::DotaPlayer;
use crate::domain::role::CoachableRole;
use crate::domain::user::User;
use crate::error::{AppError, AppResult};
use crate::repositories;
use crate::state::AppState;
use utoipa::ToSchema;

const DEFAULT_LIMIT: i64 = 20;
const MAX_LIMIT: i64 = 100;

#[derive(Deserialize)]
pub struct HistoryQuery {
    pub page: Option<i64>,
    pub limit: Option<i64>,
    /// Role slug. Omit to read the role currently being coached.
    pub role: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct SessionHistoryResponse {
    pub sessions: Vec<SessionSummary>,
    pub page: i64,
    pub limit: i64,
    pub total: i64,
    pub total_pages: i64,
    /// The role this page describes.
    pub role: CoachableRole,
    pub role_label: &'static str,
}

#[derive(Serialize, ToSchema)]
pub struct SessionResponse {
    pub session: CoachingSession,
}

/// `GET /api/coach/sessions`
#[utoipa::path(
    get, path = "/api/coach/sessions", tag = "coaching",
    summary = "Coaching history for one role",
    description = "Each session is an immutable snapshot of what was measured at one point in \
time, newest first. Summaries only — `GET /api/coach/sessions/{id}` carries the full \
snapshot.\n\n\
Defaults to the role currently being coached; `?role=` reads a role you are no longer \
coaching, because reading your own history should not require re-selecting it.",
    security(("session" = [])),
    params(
        ("page" = Option<i64>, Query,
            description = "1-based page number. Defaults to 1.",
            example = 1, minimum = 1),
        ("limit" = Option<i64>, Query,
            description = "Sessions per page. Defaults to 20.",
            example = 20, minimum = 1, maximum = 100),
        ("role" = Option<String>, Query,
            description = "Role slug — `carry`, `mid`, `offlane`, `soft_support`, `hard_support`. \
Omit to read the role currently being coached.",
            example = "carry"),
    ),
    responses(
        (status = 200, description = "One page of coaching history, newest first", body = SessionHistoryResponse),
        (status = 400, description = "Invalid pagination or role slug", body = crate::error::ErrorBody),
        (status = 409, description = "No Dota account linked, or no role chosen yet", body = crate::error::ErrorBody),
        (status = 401, description = "No session cookie, or it has expired", body = crate::error::ErrorBody),
        (status = 500, description = "Database or internal failure", body = crate::error::ErrorBody),
    )
)]
pub async fn list(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    AppQuery(query): AppQuery<HistoryQuery>,
) -> AppResult<Json<SessionHistoryResponse>> {
    let player = load_linked_player(&state, &user).await?;
    let role = resolve_role(&state, &player, query.role.as_deref()).await?;
    let (page, limit) = validate_pagination(query.page, query.limit)?;
    let offset = (page - 1) * limit;

    let sessions =
        repositories::coaching_session::list_summaries(&state.db, player.id, role, limit, offset)
            .await?;
    let total = repositories::coaching_session::count(&state.db, player.id, role).await?;

    Ok(Json(SessionHistoryResponse {
        sessions,
        page,
        limit,
        total,
        total_pages: total_pages(total, limit),
        role,
        role_label: role.label(),
    }))
}

/// `GET /api/coach/sessions/:id`
///
/// A session belonging to someone else answers 404, not 403: whether an id
/// exists is not information another player is entitled to.
#[utoipa::path(
    get, path = "/api/coach/sessions/{id}", tag = "coaching",
    summary = "One coaching session, exactly as it was recorded",
    description = "Served verbatim. A historical session is never rehydrated with today's \
figures — the numbers it shows are the numbers that were measured when it was written.",
    security(("session" = [])),
    params(
        ("id" = Uuid, Path,
            description = "Session id — the `id` from `/api/coach/sessions`.",
            example = "3fa85f64-5717-4562-b3fc-2c963f66afa6"),
    ),
    responses(
        (status = 200, description = "The stored snapshot", body = SessionResponse),
        (status = 404, description = "No such session, or it belongs to another player", body = crate::error::ErrorBody),
        (status = 409, description = "No Dota account linked to this user yet", body = crate::error::ErrorBody),
        (status = 401, description = "No session cookie, or it has expired", body = crate::error::ErrorBody),
        (status = 500, description = "Database or internal failure", body = crate::error::ErrorBody),
    )
)]
pub async fn get(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    AppPath(id): AppPath<Uuid>,
) -> AppResult<Json<SessionResponse>> {
    let player = load_linked_player(&state, &user).await?;

    let session = repositories::coaching_session::find_owned(&state.db, id, player.id)
        .await?
        .ok_or_else(|| AppError::NotFound("Coaching session not found.".into()))?;

    Ok(Json(SessionResponse { session }))
}

/// Which role's history to read.
///
/// An explicit slug wins, including one the player is no longer coached on —
/// history belongs to the player, and reading it should not mean changing
/// their current choice. Without one, the coached role is the answer, and
/// having chosen none is the same precondition `/api/coach` states.
async fn resolve_role(
    state: &AppState,
    player: &DotaPlayer,
    requested: Option<&str>,
) -> AppResult<CoachableRole> {
    if let Some(slug) = requested.map(str::trim).filter(|s| !s.is_empty()) {
        return CoachableRole::parse(slug).ok_or_else(|| {
            AppError::BadRequest(format!(
                "'{slug}' is not a role that can be coached. Choose one of: {}.",
                CoachableRole::ALL
                    .iter()
                    .map(|r| r.slug())
                    .collect::<Vec<_>>()
                    .join(", "),
            ))
        });
    }

    match coach::scope_for_checkpoint(state, player).await? {
        Some(scope) => Ok(scope.role),
        None => Err(AppError::PreconditionUnmet(
            "Choose the role you want to improve first, or ask for one by name with ?role=.".into(),
        )),
    }
}

async fn load_linked_player(state: &AppState, user: &User) -> AppResult<DotaPlayer> {
    repositories::dota_player::find_by_user_id(&state.db, user.id)
        .await?
        .ok_or(AppError::DotaAccountNotLinked)
}

/// Reject out-of-range pagination rather than silently clamping it, so a client
/// asking for page 0 or 5000 rows learns it asked for the wrong thing. The same
/// rule `/api/matches` holds.
fn validate_pagination(page: Option<i64>, limit: Option<i64>) -> AppResult<(i64, i64)> {
    let page = page.unwrap_or(1);
    let limit = limit.unwrap_or(DEFAULT_LIMIT);

    if page < 1 {
        return Err(AppError::BadRequest("page must be 1 or greater.".into()));
    }
    if !(1..=MAX_LIMIT).contains(&limit) {
        return Err(AppError::BadRequest(format!(
            "limit must be between 1 and {MAX_LIMIT}."
        )));
    }
    if page
        .checked_sub(1)
        .and_then(|p| p.checked_mul(limit))
        .is_none()
    {
        return Err(AppError::BadRequest("page is too large.".into()));
    }

    Ok((page, limit))
}

fn total_pages(total: i64, limit: i64) -> i64 {
    if total <= 0 {
        0
    } else {
        (total + limit - 1) / limit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pagination_is_rejected_rather_than_clamped() {
        assert!(validate_pagination(Some(0), None).is_err());
        assert!(validate_pagination(None, Some(0)).is_err());
        assert!(validate_pagination(None, Some(MAX_LIMIT + 1)).is_err());
        assert!(validate_pagination(Some(i64::MAX), Some(100)).is_err());
    }

    #[test]
    fn pagination_defaults_to_the_first_page() {
        assert_eq!(validate_pagination(None, None).unwrap(), (1, DEFAULT_LIMIT));
        assert_eq!(validate_pagination(Some(3), Some(50)).unwrap(), (3, 50));
    }

    #[test]
    fn total_pages_rounds_up_and_handles_an_empty_history() {
        assert_eq!(total_pages(0, 20), 0);
        assert_eq!(total_pages(1, 20), 1);
        assert_eq!(total_pages(20, 20), 1);
        assert_eq!(total_pages(21, 20), 2);
    }
}

//! Match history for the signed-in player.
//!
//! Both handlers scope every query to the caller's own Dota player id, so
//! there is no code path that reads another user's matches.

use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::api::extract::{AppPath, AppQuery, CurrentUser};
use crate::domain::player::DotaPlayer;
use crate::domain::r#match::Match;
use crate::domain::user::User;
use crate::error::{AppError, AppResult};
use crate::repositories;
use crate::state::AppState;
use utoipa::ToSchema;

const DEFAULT_LIMIT: i64 = 20;
const MAX_LIMIT: i64 = 100;

#[derive(Deserialize)]
pub struct PageQuery {
    pub page: Option<i64>,
    pub limit: Option<i64>,
}

#[derive(Serialize, ToSchema)]
pub struct MatchListResponse {
    pub matches: Vec<Match>,
    pub page: i64,
    pub limit: i64,
    pub total: i64,
    pub total_pages: i64,
}

/// `GET /api/matches?page=1&limit=20`
#[utoipa::path(
    get, path = "/api/matches", tag = "matches",
    summary = "Stored match history",
    security(("session" = [])),
    params(
        ("page" = Option<i64>, Query,
            description = "1-based page number. Defaults to 1.",
            example = 1, minimum = 1),
        ("limit" = Option<i64>, Query,
            description = "Matches per page. Defaults to 20.",
            example = 20, minimum = 1, maximum = 100),
    ),
    responses(
        (status = 200, description = "One page of matches, newest first", body = MatchListResponse),
        (status = 409, description = "No Dota account linked to this user yet", body = crate::error::ErrorBody),
        (status = 401, description = "No session cookie, or it has expired", body = crate::error::ErrorBody),
        (status = 500, description = "Database or internal failure", body = crate::error::ErrorBody),
    )
)]
pub async fn list(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    AppQuery(query): AppQuery<PageQuery>,
) -> AppResult<Json<MatchListResponse>> {
    let player = load_linked_player(&state, &user).await?;
    let (page, limit) = validate_pagination(query.page, query.limit)?;

    let matches =
        repositories::r#match::list_by_player(&state.db, player.id, limit, (page - 1) * limit)
            .await?;
    let total = repositories::r#match::count_by_player(&state.db, player.id).await?;

    Ok(Json(MatchListResponse {
        matches,
        page,
        limit,
        total,
        total_pages: total_pages(total, limit),
    }))
}

/// `GET /api/matches/:id`
///
/// A match belonging to someone else answers 404, not 403: whether an id
/// exists is not information another user is entitled to.
#[utoipa::path(
    get, path = "/api/matches/{id}", tag = "matches",
    summary = "One match with its metrics",
    security(("session" = [])),
    params(
        ("id" = Uuid, Path,
            description = "Internal match id — the `id` from `/api/matches`, not the Dota match id.",
            example = "3fa85f64-5717-4562-b3fc-2c963f66afa6"),
    ),
    responses(
        (status = 200, description = "The match and its computed metrics", body = MatchResponse),
        (status = 404, description = "No such match, or it belongs to another player", body = crate::error::ErrorBody),
        (status = 409, description = "No Dota account linked to this user yet", body = crate::error::ErrorBody),
        (status = 401, description = "No session cookie, or it has expired", body = crate::error::ErrorBody),
        (status = 500, description = "Database or internal failure", body = crate::error::ErrorBody),
    )
)]
pub async fn get(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    AppPath(id): AppPath<Uuid>,
) -> AppResult<Json<MatchResponse>> {
    let player = load_linked_player(&state, &user).await?;

    let match_ = repositories::r#match::find_owned(&state.db, id, player.id)
        .await?
        .ok_or_else(|| AppError::NotFound("Match not found.".into()))?;

    Ok(Json(MatchResponse { match_ }))
}

#[derive(Serialize, ToSchema)]
pub struct MatchResponse {
    #[serde(rename = "match")]
    pub match_: Match,
}

async fn load_linked_player(state: &AppState, user: &User) -> AppResult<DotaPlayer> {
    repositories::dota_player::find_by_user_id(&state.db, user.id)
        .await?
        .ok_or(AppError::DotaAccountNotLinked)
}

/// Reject out-of-range pagination rather than silently clamping it, so a
/// client asking for page 0 or 5000 rows learns it asked for the wrong thing.
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
    // (page - 1) * limit must stay inside i64 before it reaches SQL.
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
        // Signed `div_ceil` is still unstable; `limit` is validated >= 1.
        (total + limit - 1) / limit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pagination_defaults_to_the_first_page() {
        assert_eq!(validate_pagination(None, None).unwrap(), (1, DEFAULT_LIMIT));
    }

    #[test]
    fn explicit_pagination_is_honoured() {
        assert_eq!(validate_pagination(Some(3), Some(50)).unwrap(), (3, 50));
    }

    #[test]
    fn a_page_below_one_is_rejected() {
        assert!(validate_pagination(Some(0), None).is_err());
        assert!(validate_pagination(Some(-1), None).is_err());
    }

    #[test]
    fn an_out_of_range_limit_is_rejected_rather_than_clamped() {
        assert!(validate_pagination(None, Some(0)).is_err());
        assert!(validate_pagination(None, Some(-5)).is_err());
        assert!(validate_pagination(None, Some(MAX_LIMIT + 1)).is_err());
        assert!(validate_pagination(None, Some(i64::MAX)).is_err());
    }

    #[test]
    fn the_boundary_values_are_accepted() {
        assert!(validate_pagination(Some(1), Some(1)).is_ok());
        assert!(validate_pagination(Some(1), Some(MAX_LIMIT)).is_ok());
    }

    #[test]
    fn an_offset_that_would_overflow_is_rejected() {
        assert!(validate_pagination(Some(i64::MAX), Some(100)).is_err());
    }

    #[test]
    fn total_pages_rounds_up_and_handles_an_empty_history() {
        assert_eq!(total_pages(0, 20), 0);
        assert_eq!(total_pages(1, 20), 1);
        assert_eq!(total_pages(20, 20), 1);
        assert_eq!(total_pages(21, 20), 2);
        assert_eq!(total_pages(40, 20), 2);
    }
}

//! Match history for the signed-in player.
//!
//! Both handlers scope every query to the caller's own Dota player id, so
//! there is no code path that reads another user's matches.

use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::api::extract::{AppPath, AppQuery, CurrentUser};
use crate::domain::eligibility;
use crate::domain::player::DotaPlayer;
use crate::domain::r#match::Match;
use crate::domain::scope::MatchScope;
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
    pub scope: Option<String>,
}

/// Which population a caller wants listed.
///
/// The default is every stored match, because the match list is a record of
/// what the player actually played — hiding their Turbo games from their own
/// history would be a strange thing for a Dota app to do. `competitive` exists
/// for the callers that are *analysing* rather than browsing: the dashboard's
/// trend line and form strip read the same games the dashboard's numbers do,
/// which is what stops a chart and the statistic above it disagreeing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ListScope {
    All,
    Competitive,
}

impl ListScope {
    fn slug(self) -> &'static str {
        match self {
            ListScope::All => "all",
            ListScope::Competitive => "competitive",
        }
    }

    /// Rejected rather than defaulted: a client that misspells the scope is
    /// asking for a population it will not get, and silently serving the other
    /// one is how a Turbo game ends up on a competitive chart.
    fn parse(value: Option<&str>) -> AppResult<Self> {
        match value.map(str::trim) {
            None | Some("") | Some("all") => Ok(ListScope::All),
            Some("competitive") => Ok(ListScope::Competitive),
            Some(other) => Err(AppError::BadRequest(format!(
                "Unknown scope '{other}'. Use 'all' or 'competitive'."
            ))),
        }
    }
}

/// A match with the two facts a list row needs and the row cannot derive.
///
/// Eligibility is one rule in one place, and the client is not a second copy
/// of it: rather than shipping the mode tables to the browser, the server says
/// what each match was and whether coaching reads it. That is also what lets
/// the match list and the dashboard visibly agree — a player looking at nine
/// Turbo games can see exactly why their analysis says thirty matches.
#[derive(Serialize, ToSchema)]
pub struct MatchView {
    #[serde(flatten)]
    pub match_: Match,
    /// Whether this match is part of the competitive population.
    pub eligible: bool,
    /// What it was: `Ranked All Pick`, `Turbo`, `Other mode`…
    pub mode_label: &'static str,
}

impl MatchView {
    fn of(match_: Match) -> Self {
        Self {
            eligible: eligibility::is_eligible(match_.game_mode, match_.lobby_type),
            mode_label: eligibility::mode_label(match_.game_mode, match_.lobby_type),
            match_,
        }
    }
}

#[derive(Serialize, ToSchema)]
pub struct MatchListResponse {
    pub matches: Vec<MatchView>,
    pub page: i64,
    pub limit: i64,
    pub total: i64,
    pub total_pages: i64,
    /// Which population this page was drawn from.
    pub scope: &'static str,
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
        ("scope" = Option<String>, Query,
            description = "`all` (default) lists every stored match. `competitive` lists only the \
latest eligible Ranked and public All Pick matches — the same population `/api/stats` reads.",
            example = "competitive"),
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
    let scope = ListScope::parse(query.scope.as_deref())?;
    let offset = (page - 1) * limit;

    let (matches, total) = match scope {
        ListScope::All => (
            repositories::r#match::list_by_player(&state.db, player.id, limit, offset).await?,
            repositories::r#match::count_by_player(&state.db, player.id).await?,
        ),
        ListScope::Competitive => {
            let window = MatchScope::competitive(state.config.roles.analysis_match_limit);
            (
                repositories::r#match::list_by_player_scoped(
                    &state.db, player.id, &window, limit, offset,
                )
                .await?,
                repositories::r#match::count_by_player_scoped(&state.db, player.id, &window)
                    .await?,
            )
        }
    };

    Ok(Json(MatchListResponse {
        matches: matches.into_iter().map(MatchView::of).collect(),
        page,
        limit,
        total,
        total_pages: total_pages(total, limit),
        scope: scope.slug(),
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

    Ok(Json(MatchResponse {
        match_: MatchView::of(match_),
    }))
}

#[derive(Serialize, ToSchema)]
pub struct MatchResponse {
    #[serde(rename = "match")]
    pub match_: MatchView,
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
    fn the_default_scope_is_the_players_whole_history() {
        assert_eq!(ListScope::parse(None).unwrap(), ListScope::All);
        assert_eq!(ListScope::parse(Some("")).unwrap(), ListScope::All);
        assert_eq!(ListScope::parse(Some("all")).unwrap(), ListScope::All);
    }

    #[test]
    fn the_competitive_scope_is_requested_by_name() {
        assert_eq!(
            ListScope::parse(Some("competitive")).unwrap(),
            ListScope::Competitive
        );
        assert_eq!(
            ListScope::parse(Some(" competitive ")).unwrap(),
            ListScope::Competitive
        );
    }

    #[test]
    fn an_unknown_scope_is_rejected_rather_than_served_the_other_population() {
        assert!(ListScope::parse(Some("ranked")).is_err());
        assert!(ListScope::parse(Some("turbo")).is_err());
        assert!(ListScope::parse(Some("Competitive")).is_err());
    }

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

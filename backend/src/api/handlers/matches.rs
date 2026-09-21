//! Match history for the signed-in player.
//!
//! Both handlers scope every query to the caller's own Dota player id, so
//! there is no code path that reads another user's matches.

use std::collections::HashMap;

use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::api::extract::{AppPath, AppQuery, CurrentUser};
use crate::api::handlers::benchmark;
use crate::domain::benchmark::{BenchmarkContext, ResolvedBracket};
use crate::domain::eligibility;
use crate::domain::match_comparison::{MatchComparison, Standing};
use crate::domain::player::DotaPlayer;
use crate::domain::r#match::Match;
use crate::domain::role::CoachableRole;
use crate::domain::scope::MatchScope;
use crate::domain::user::User;
use crate::error::{AppError, AppResult};
use crate::repositories;
use crate::repositories::r#match::{MatchFilter, MatchSort};
use crate::services::match_comparison;
use crate::state::AppState;
use utoipa::ToSchema;

const DEFAULT_LIMIT: i64 = 20;
const MAX_LIMIT: i64 = 100;

#[derive(Deserialize)]
pub struct PageQuery {
    pub page: Option<i64>,
    pub limit: Option<i64>,
    pub scope: Option<String>,
    /// Dota hero id. Omit for every hero.
    pub hero_id: Option<i32>,
    /// Role slug, or `all`. Omit for every role.
    pub role: Option<String>,
    /// `win`, `loss`, or `all`.
    pub result: Option<String>,
    /// One of the [`MatchSort`] slugs. Defaults to `newest`.
    pub sort: Option<String>,
}

impl PageQuery {
    /// The display filter this request asks for.
    ///
    /// Every unrecognised value is rejected rather than ignored, for the same
    /// reason [`ListScope::parse`] rejects one: a client that misspells a filter
    /// would otherwise be handed a page that quietly does not match what it
    /// asked for, and would have no way to tell.
    fn filter(&self) -> AppResult<MatchFilter> {
        let role = match self.role.as_deref().map(str::trim) {
            None | Some("") | Some("all") => None,
            Some(slug) => Some(CoachableRole::parse(slug).ok_or_else(|| {
                AppError::BadRequest(format!(
                    "'{slug}' is not a role. Use one of: {}, or 'all'.",
                    CoachableRole::ALL
                        .iter()
                        .map(|r| r.slug())
                        .collect::<Vec<_>>()
                        .join(", "),
                ))
            })?),
        };

        let won = match self.result.as_deref().map(str::trim) {
            None | Some("") | Some("all") => None,
            Some("win") | Some("wins") => Some(true),
            Some("loss") | Some("losses") => Some(false),
            Some(other) => {
                return Err(AppError::BadRequest(format!(
                    "Unknown result '{other}'. Use 'win', 'loss' or 'all'."
                )))
            }
        };

        let sort = match self.sort.as_deref().map(str::trim) {
            None | Some("") => MatchSort::default(),
            Some(slug) => MatchSort::parse(slug).ok_or_else(|| {
                AppError::BadRequest(format!(
                    "Unknown sort '{slug}'. Use one of: newest, oldest, gpm_desc, gpm_asc, \
                     kda_desc."
                ))
            })?,
        };

        // A hero id is a foreign identifier, not a code path: an id the player
        // has never played is an empty page, which is a legitimate answer.
        if self.hero_id.is_some_and(|id| id < 1) {
            return Err(AppError::BadRequest("hero_id must be positive.".into()));
        }

        Ok(MatchFilter {
            hero_id: self.hero_id,
            won,
            role,
            sort,
        })
    }
}

/// One value a filter can take, with how many matches it would leave.
///
/// Counted over the population the page is browsing and *not* over the current
/// filters, so the options never collapse to whatever is already selected.
#[derive(Serialize, ToSchema)]
pub struct FilterOption {
    /// The value to send back: a hero id as a string, or a role slug.
    pub value: String,
    pub label: String,
    pub matches: i64,
}

/// What this population can be filtered by.
#[derive(Serialize, ToSchema)]
pub struct FilterOptions {
    /// Heroes the player has actually played in this scope, most-played first.
    pub heroes: Vec<FilterOption>,
    /// Coachable roles present in this scope, in position order. A match the
    /// estimator could not attribute to one of the five is absent here rather
    /// than filed under a guess — see `domain::role`.
    pub roles: Vec<FilterOption>,
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
    /// How this page was filtered and ordered — echoed back so a client can
    /// tell a page it asked for from one it inherited.
    pub filtered: bool,
    pub sort: &'static str,
    /// What this population *can* be filtered by, from the player's own rows.
    pub filters: FilterOptions,
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
        ("hero_id" = Option<i32>, Query,
            description = "Show only matches on this hero. Omit for every hero.",
            example = 26, minimum = 1),
        ("role" = Option<String>, Query,
            description = "Show only matches in this role — `carry`, `mid`, `offlane`, \
`soft_support`, `hard_support` — or `all`. Matches the estimator could not attribute to one of \
the five are excluded by any role filter rather than guessed at.",
            example = "carry"),
        ("result" = Option<String>, Query,
            description = "`win`, `loss`, or `all` (default).",
            example = "loss"),
        ("sort" = Option<String>, Query,
            description = "`newest` (default), `oldest`, `gpm_desc`, `gpm_asc` or `kda_desc`. \
Matches with no computed metrics sort last under `kda_desc` rather than counting as zero.",
            example = "gpm_desc"),
    ),
    responses(
        (status = 200, description = "One page of matches, newest first unless sorted otherwise", body = MatchListResponse),
        (status = 400, description = "An unknown scope, role, result or sort value", body = crate::error::ErrorBody),
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
    let filter = query.filter()?;
    let offset = (page - 1) * limit;

    // The scope is the population; the filter is a view of it. Both are pushed
    // into SQL rather than trimmed afterwards, so `total` and `total_pages`
    // describe the filtered list and the client never has to hold a page it
    // was not shown.
    let window = match scope {
        ListScope::All => MatchScope::career(),
        ListScope::Competitive => MatchScope::competitive(state.config.roles.analysis_match_limit),
    };

    let (matches, total) = if window.is_career() {
        (
            repositories::r#match::list_by_player(&state.db, player.id, &filter, limit, offset)
                .await?,
            repositories::r#match::count_by_player(&state.db, player.id, &filter).await?,
        )
    } else {
        (
            repositories::r#match::list_by_player_scoped(
                &state.db, player.id, &window, &filter, limit, offset,
            )
            .await?,
            repositories::r#match::count_by_player_scoped(&state.db, player.id, &window, &filter)
                .await?,
        )
    };

    Ok(Json(MatchListResponse {
        matches: matches.into_iter().map(MatchView::of).collect(),
        page,
        limit,
        total,
        total_pages: total_pages(total, limit),
        scope: scope.slug(),
        filtered: !filter.is_empty(),
        sort: filter.sort.slug(),
        filters: filter_options(&state, player.id, &window).await?,
    }))
}

/// The values the filters can take in this population.
///
/// Two grouped counts over the player's own rows. Nothing about heroes or roles
/// is hard-coded here or shipped to the client as a list: a filter that offers
/// a hero the player has never played leads to a guaranteed empty page, and the
/// counts are what let someone see that before they click.
async fn filter_options(
    state: &AppState,
    dota_player_id: Uuid,
    window: &MatchScope,
) -> AppResult<FilterOptions> {
    let heroes = repositories::r#match::hero_facets(&state.db, dota_player_id, window).await?;
    let roles = repositories::r#match::role_facets(&state.db, dota_player_id, window).await?;

    // Several stored labels can map onto one coachable role, so they are summed
    // rather than listed — and a label that maps to none of the five is dropped,
    // which is the same rule the role scope applies.
    let mut by_role: HashMap<CoachableRole, i64> = HashMap::new();
    for facet in roles {
        if let Some(role) = CoachableRole::from_stored(&facet.role) {
            *by_role.entry(role).or_default() += facet.matches;
        }
    }

    Ok(FilterOptions {
        heroes: heroes
            .into_iter()
            .map(|hero| FilterOption {
                value: hero.hero_id.to_string(),
                label: hero.hero_name,
                matches: hero.matches,
            })
            .collect(),
        // Position order, not count order: a list of roles that reshuffles as
        // the player's history changes is harder to use than a fixed one.
        roles: CoachableRole::ALL
            .into_iter()
            .filter_map(|role| {
                let matches = *by_role.get(&role)?;
                Some(FilterOption {
                    value: role.slug().to_string(),
                    label: role.label().to_string(),
                    matches,
                })
            })
            .collect(),
    })
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

/// `GET /api/matches/:id/comparison`
///
/// How this game went against players in the same rank bracket on the same
/// hero, the player's own trend on that hero, and what the numbers say to work
/// on. Every figure is computed here; the language model is not involved.
///
/// Like `get`, a match belonging to someone else answers 404.
#[utoipa::path(
    get, path = "/api/matches/{id}/comparison", tag = "matches",
    summary = "This match against same-rank peers on the same hero",
    description = "Places one match in the peer distribution for its hero **and the player's own \
rank bracket**, beside the player's average on that hero, with a trend over the games leading up \
to it.\n\n\
The two percentiles are different claims and are labelled as such: the single match is a fact \
about a game that was played and carries no sample floor, while the hero average is an estimate \
about the player and carries the usual `confidence`.\n\n\
Turbo and other ineligible matches answer `comparable: false` — their raw values are served, but \
no percentile is claimed against a distribution drawn from ranked public matches. A benchmark \
provider outage degrades the response to raw values and a `note` rather than failing it.",
    security(("session" = [])),
    params(
        ("id" = Uuid, Path,
            description = "Internal match id — the `id` from `/api/matches`, not the Dota match id.",
            example = "3fa85f64-5717-4562-b3fc-2c963f66afa6"),
    ),
    responses(
        (status = 200, description = "The comparison, possibly degraded with a note", body = MatchComparison),
        (status = 404, description = "No such match, or it belongs to another player", body = crate::error::ErrorBody),
        (status = 409, description = "No Dota account linked to this user yet", body = crate::error::ErrorBody),
        (status = 401, description = "No session cookie, or it has expired", body = crate::error::ErrorBody),
        (status = 500, description = "Database or internal failure", body = crate::error::ErrorBody),
    )
)]
pub async fn comparison(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    AppPath(id): AppPath<Uuid>,
) -> AppResult<Json<MatchComparison>> {
    let player = load_linked_player(&state, &user).await?;

    let match_ = repositories::r#match::find_owned(&state.db, id, player.id)
        .await?
        .ok_or_else(|| AppError::NotFound("Match not found.".into()))?;

    // No role filter. The page is about this hero in this game, and the
    // player's other games on it are the right comparison whichever position
    // they ran — `Match.role` is a lane-priority guess over a different set of
    // labels than the five the coaching engine uses, so forcing one onto the
    // other would mislabel rather than narrow.
    let scope = MatchScope::competitive(state.config.roles.analysis_match_limit);

    // Unscoped on purpose: a Turbo game, or one older than the analysis
    // window, still has numbers worth showing on its own page.
    let figures = repositories::metrics::match_figures(&state.db, match_.id)
        .await?
        .unwrap_or_default();
    let match_values = figures.values();

    let averages =
        repositories::metrics::hero_averages(&state.db, player.id, match_.hero_id, &scope).await?;
    let average_values = averages.figures.values();

    let history =
        repositories::metrics::hero_match_values(&state.db, player.id, match_.hero_id, &scope)
            .await?;

    // Whether this match may be *compared*, as opposed to displayed. A Turbo
    // game's gold per minute against a distribution drawn from ranked public
    // matches is the fabricated precision the product exists to avoid.
    let eligible = eligibility::is_eligible(match_.game_mode, match_.lobby_type);

    let context = BenchmarkContext {
        hero_id: match_.hero_id,
        role: None,
        rank_tier: player.rank_tier,
        // A single match is compared against the player's own bracket. "How
        // did this game go against my peers" has one right peer group.
        bracket: None,
        patch: None,
    };

    let distribution = match state.benchmarks.get_distribution(&context).await {
        Ok(d) => Some(d),
        Err(e) => {
            tracing::warn!(error = %e, hero_id = match_.hero_id, "benchmark distribution unavailable");
            None
        }
    };

    let provider_note = match &distribution {
        Some(_) => None,
        None => Some(
            "Peer comparison is unavailable right now, so this match is shown without \
             percentiles. Your own figures are unaffected."
                .to_string(),
        ),
    };

    let bracket = distribution
        .as_ref()
        .map(|d| d.bracket)
        .unwrap_or_else(|| ResolvedBracket::requested_for(player.rank_tier));

    let (match_percentiles, average_percentiles) = match (&distribution, eligible) {
        (Some(d), true) => (
            match_comparison::percentiles_for(&match_values, d),
            match_comparison::percentiles_for(&average_values, d),
        ),
        // The hero average is drawn from eligible games only, so it keeps its
        // percentiles even when the match on screen is a Turbo one.
        (Some(d), false) => (
            HashMap::new(),
            match_comparison::percentiles_for(&average_values, d),
        ),
        (None, _) => (HashMap::new(), HashMap::new()),
    };

    let this_standing = match_comparison::standing(&match_percentiles);
    let average_standing = match_comparison::standing(&average_percentiles);

    let (trend, delta_vs_previous) = match &distribution {
        Some(d) => {
            let window = match_comparison::trend_window(&history, match_.id);
            let points = match_comparison::trend(window, d, match_.id);
            let delta = match_comparison::delta_vs_previous(&points, match_.id);
            (points, delta)
        }
        None => (Vec::new(), None),
    };

    let (pros, cons, suggestion) = match (&distribution, eligible) {
        (Some(d), true) => {
            let (pros, cons) = match_comparison::pros_and_cons(
                &match_percentiles,
                &match_values,
                d,
                bracket.label,
            );
            let suggestion = match_comparison::suggestion(
                &match_percentiles,
                &match_values,
                d,
                match_.duration_seconds,
                &match_.hero_name,
                bracket.label,
            );
            (pros, cons, suggestion)
        }
        // Nothing to say about a game that was not compared. Saying it anyway
        // would be reading strengths out of an empty percentile map.
        _ => (Vec::new(), Vec::new(), None),
    };

    let note = provider_note.or_else(|| {
        (!eligible).then(|| {
            format!(
                "This was a {} game. The peer distribution covers ranked public matches, so its \
                 figures are shown without percentiles; the trend and your average below come \
                 from your eligible games on this hero.",
                eligibility::mode_label(match_.game_mode, match_.lobby_type),
            )
        })
    });

    let segmented_by = distribution
        .as_ref()
        .map(|d| d.segmented_by.clone())
        .unwrap_or_default();

    Ok(Json(MatchComparison {
        hero_id: match_.hero_id,
        bracket,
        comparable: eligible && distribution.is_some(),
        standing: Standing {
            this_match: this_standing,
            hero_average: average_standing,
            metrics_counted: if this_standing.is_some() {
                match_percentiles.len()
            } else {
                average_percentiles.len()
            },
            peer_sample_size: distribution.as_ref().and_then(|d| d.sample_size),
        },
        metrics: match_comparison::metric_comparisons(
            &match_values,
            &match_percentiles,
            &average_values,
            &average_percentiles,
            averages.sample,
            distribution.as_ref(),
        ),
        trend,
        delta_vs_previous,
        pros,
        cons,
        suggestion,
        context: benchmark::context_info(
            match_.hero_id,
            &match_.hero_name,
            None,
            &player,
            &scope,
            &segmented_by,
            bracket,
        ),
        hero_name: match_.hero_name,
        note,
    }))
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

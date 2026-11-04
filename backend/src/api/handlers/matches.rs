//! Match history for the signed-in player.
//!
//! Both handlers scope every query to the caller's own Dota player id, so
//! there is no code path that reads another user's matches.

use std::collections::HashMap;

use axum::extract::State;
use axum::Json;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::api::extract::{AppPath, AppQuery, CurrentUser, EntitledUser};
use crate::api::handlers::{benchmark, players};
use crate::config::CalibrationConfig;
use crate::domain::benchmark::{BenchmarkContext, BenchmarkMetric, ResolvedBracket};
use crate::domain::eligibility;
use crate::domain::match_comparison::{MatchComparison, Standing};
use crate::domain::player::DotaPlayer;
use crate::domain::r#match::Match;
use crate::domain::role::CoachableRole;
use crate::domain::scope::MatchScope;
use crate::domain::user::User;
use crate::error::{AppError, AppResult};
use crate::repositories;
use crate::repositories::r#match::{MatchFilter, MatchSort, ModeFilter};
use crate::services::metrics::rating;
use crate::services::benchmarks::percentile;
use crate::services::{calibration, match_comparison, sync};
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
    /// One of the [`ModeFilter`] slugs: `all`, `ranked` or `turbo`.
    pub mode: Option<String>,
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

        // Set from the list's mode by the handler; see `ListMode`.
        let mode = ModeFilter::All;
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
            mode,
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
/// Which games the list shows. One control rather than a scope and a mode:
///
///   - `eligible` (default): Ranked and public All Pick, the newest N, the
///     exact window Coach, Heroes and Benchmark read, so a hero's count here
///     matches theirs. Eligibility is `domain::eligibility`, applied in SQL.
///   - `ranked`: the ranked-lobby part of `eligible`, the games that move a
///     medal. For the rank page.
///   - `turbo`: Turbo games only. Stored, never coached.
///   - `all`: every stored game.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ListMode {
    Eligible,
    Ranked,
    Turbo,
    All,
}

impl ListMode {
    fn slug(self) -> &'static str {
        match self {
            ListMode::Eligible => "eligible",
            ListMode::Ranked => "ranked",
            ListMode::Turbo => "turbo",
            ListMode::All => "all",
        }
    }

    /// Rejected rather than defaulted: a misspelt mode is asking for a
    /// population it will not get, and silently serving another is how a
    /// Turbo game ends up on a competitive chart.
    fn parse(value: Option<&str>) -> AppResult<Self> {
        match value.map(str::trim) {
            None | Some("") | Some("eligible") => Ok(ListMode::Eligible),
            Some("ranked") => Ok(ListMode::Ranked),
            Some("turbo") => Ok(ListMode::Turbo),
            Some("all") => Ok(ListMode::All),
            Some(other) => Err(AppError::BadRequest(format!(
                "Unknown mode '{other}'. Use 'eligible', 'ranked', 'turbo' or 'all'."
            ))),
        }
    }

    /// The window of matches, and the narrowing applied inside it.
    fn population(self, window: i64) -> (MatchScope, ModeFilter) {
        match self {
            ListMode::Eligible => (MatchScope::competitive(window), ModeFilter::All),
            ListMode::Ranked => (MatchScope::competitive(window), ModeFilter::Ranked),
            ListMode::Turbo => (MatchScope::career(), ModeFilter::Turbo),
            ListMode::All => (MatchScope::career(), ModeFilter::All),
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
    /// The position as one of the five, or `null` when it cannot be told
    /// (a stored "Core" could be any of three). The one role taxonomy.
    pub position: Option<CoachableRole>,
    /// The position's label, or "Unknown".
    pub position_label: &'static str,
    /// How the position was decided: `user` (corrected by the player),
    /// `lane` (from the provider's lane data, the stronger estimate),
    /// `estimated` (from farm priority alone), or `unknown`.
    pub position_source: &'static str,
    /// Peer medians for this hero in the player's bracket, for eligible games
    /// only: there is no peer distribution for Turbo to measure against.
    pub peer_gpm: Option<f32>,
    pub peer_xpm: Option<f32>,
    /// How this game went, 1.0–10.0, against the player's own typical game on
    /// this hero. Computed here, never by the model — see
    /// [`crate::services::metrics::rating`].
    pub rating: f32,
    /// Estimated ladder movement, in MMR.
    ///
    /// `null` for a game that cannot move a medal — Turbo, an unranked lobby —
    /// which is a different statement from zero. Where it is present it is the
    /// *same* disclosed estimate the calibrating screen's momentum curve plots,
    /// and every rendering of it has to say it is an estimate: Valve publishes
    /// no per-match MMR.
    pub mmr_delta_estimate: Option<i32>,
}

impl MatchView {
    fn of(match_: Match, context: &RowContext, config: &CalibrationConfig) -> Self {
        let eligible = eligibility::is_eligible(match_.game_mode, match_.lobby_type);
        let position = CoachableRole::from_stored(&match_.role);
        let peer = eligible
            .then(|| context.peers.get(&match_.hero_id).copied())
            .flatten();
        Self {
            eligible,
            position,
            position_label: position.map_or("Unknown", CoachableRole::label),
            position_source: position_source(&match_, position.is_some()),
            peer_gpm: peer.and_then(|p| p.0),
            peer_xpm: peer.and_then(|p| p.1),
            mode_label: eligibility::mode_label(match_.game_mode, match_.lobby_type),
            rating: rating::rating(&match_, &context.baselines.for_match(&match_)),
            mmr_delta_estimate: calibration::estimated_mmr_delta(
                &match_,
                context.median_kda,
                config,
            ),
            match_,
        }
    }
}

/// The two player-wide figures every row on a page is measured against.
///
/// Loaded once per request rather than per row: a page of twenty matches would
/// otherwise be forty queries, and — worse — rows would be comparable only to
/// themselves. Both figures are about the player, not about the page, so they do
/// not change as someone flips through it.
fn position_source(match_: &Match, known: bool) -> &'static str {
    if match_.role_source == "user" {
        "user"
    } else if !known {
        "unknown"
    } else if match_.lane_role.is_some() {
        "lane"
    } else {
        "estimated"
    }
}

/// `POST /api/matches/:id/role` body.
#[derive(Deserialize, ToSchema)]
pub struct SetRoleRequest {
    /// A position slug (`carry` ... `hard_support`), or `auto` to hand the
    /// match back to the estimator.
    pub role: String,
}

/// `POST /api/matches/:id/role`
///
/// Correct the position a match was played in. Stored on the match, so every
/// page that filters by position (Coach, Heroes, Benchmark, Matches) uses it.
#[utoipa::path(
    post, path = "/api/matches/{id}/role", tag = "matches",
    summary = "Correct a match's position",
    description = "Overrides the estimated position for one match, or restores the estimate with `auto`. The correction is stored on the match and used everywhere a position matters.",
    security(("session" = [])),
    params(
        ("id" = Uuid, Path,
            description = "Internal match id — the `id` from `/api/matches`.",
            example = "3fa85f64-5717-4562-b3fc-2c963f66afa6"),
    ),
    request_body = SetRoleRequest,
    responses(
        (status = 200, description = "The match with its new position", body = MatchResponse),
        (status = 400, description = "Not a position slug or `auto`", body = crate::error::ErrorBody),
        (status = 404, description = "No such match, or it belongs to another player", body = crate::error::ErrorBody),
        (status = 402, description = "Trial expired and no active subscription", body = crate::error::ErrorBody),
        (status = 401, description = "No session cookie, or it has expired", body = crate::error::ErrorBody),
        (status = 500, description = "Database or internal failure", body = crate::error::ErrorBody),
    )
)]
pub async fn set_role(
    State(state): State<AppState>,
    EntitledUser(user): EntitledUser,
    AppPath(id): AppPath<Uuid>,
    crate::api::extract::AppJson(body): crate::api::extract::AppJson<SetRoleRequest>,
) -> AppResult<Json<MatchResponse>> {
    let player = load_linked_player(&state, &user).await?;
    let label = match body.role.trim() {
        "auto" => None,
        slug => Some(
            CoachableRole::parse(slug)
                .ok_or_else(|| {
                    AppError::BadRequest(format!(
                        "'{slug}' is not a position. Use one of: {}, or 'auto'.",
                        CoachableRole::ALL.iter().map(|r| r.slug()).collect::<Vec<_>>().join(", ")
                    ))
                })?
                // The estimator's own label for the position, so filters
                // treat a correction exactly like an estimate of the same.
                .stored_labels()[0],
        ),
    };
    let updated =
        repositories::r#match::set_role_by_player(&state.db, id, player.id, label).await?;
    if !updated {
        return Err(AppError::NotFound("Match not found.".into()));
    }
    get(State(state), EntitledUser(user), AppPath(id)).await
}

/// `POST /api/matches/:id/parse` response.
#[derive(Serialize, ToSchema)]
pub struct ParseRequestResponse {
    pub requested: bool,
    /// One line for the page.
    pub note: &'static str,
}

/// One parse request per match per this many seconds: OpenDota counts each
/// as ten calls.
const PARSE_COOLDOWN_SECONDS: i64 = 3600;

static PARSE_REQUESTS: std::sync::LazyLock<
    std::sync::Mutex<HashMap<Uuid, DateTime<Utc>>>,
> = std::sync::LazyLock::new(Default::default);

/// `POST /api/matches/:id/parse`
///
/// Ask OpenDota to parse this match's replay, for when no timeline exists.
/// One match's own timeline, for the page's net worth and deaths chart.
#[derive(Debug, Serialize, ToSchema)]
pub struct MatchTimelineResponse {
    /// False when there is no parsed replay (or the provider is down or not
    /// configured). Then the lists are empty and `note` says why.
    pub available: bool,
    pub note: Option<String>,
    pub duration_seconds: i32,
    /// The player's net worth at each completed minute, index 0 = minute 0.
    pub net_worth_per_minute: Vec<i32>,
    /// The player's deaths, in time order.
    pub deaths: Vec<TimelineDeath>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TimelineDeath {
    pub time_seconds: i32,
    pub killer_hero_name: Option<String>,
}

#[utoipa::path(
    get, path = "/api/matches/{id}/timeline", tag = "matches",
    summary = "One match's net worth curve and deaths",
    description = "The player's own net worth per minute and death times from a parsed replay, via the match facts provider (cached). Never fails on a provider problem: `available` is false and `note` says why.",
    security(("session" = [])),
    params(
        ("id" = Uuid, Path,
            description = "Internal match id — the `id` from `/api/matches`.",
            example = "3fa85f64-5717-4562-b3fc-2c963f66afa6"),
    ),
    responses(
        (status = 200, description = "The timeline, or why there is none", body = MatchTimelineResponse),
        (status = 404, description = "No such match, or it belongs to another player", body = crate::error::ErrorBody),
        (status = 402, description = "Trial expired and no active subscription", body = crate::error::ErrorBody),
        (status = 401, description = "No session cookie, or it has expired", body = crate::error::ErrorBody),
        (status = 500, description = "Database or internal failure", body = crate::error::ErrorBody),
    )
)]
pub async fn timeline(
    State(state): State<AppState>,
    EntitledUser(user): EntitledUser,
    AppPath(id): AppPath<Uuid>,
) -> AppResult<Json<MatchTimelineResponse>> {
    let player = load_linked_player(&state, &user).await?;
    let match_ = repositories::r#match::find_owned(&state.db, id, player.id)
        .await?
        .ok_or_else(|| AppError::NotFound("Match not found.".into()))?;

    let empty = |note: &str| MatchTimelineResponse {
        available: false,
        note: Some(note.to_string()),
        duration_seconds: match_.duration_seconds,
        net_worth_per_minute: Vec::new(),
        deaths: Vec::new(),
    };

    let provider = state.match_facts.as_ref();
    if !provider.is_configured() {
        return Ok(Json(empty("No timeline provider is configured.")));
    }
    let facts = match provider
        .get_match_facts(match_.match_id, player.dota_account_id)
        .await
    {
        Ok(facts) => facts,
        Err(e) => {
            tracing::warn!(error = %e, match_id = match_.match_id, "match timeline unavailable");
            return Ok(Json(empty("The timeline provider is unavailable right now.")));
        }
    };
    if !facts.parsed || facts.player.net_worth_per_minute.is_empty() {
        return Ok(Json(empty("This replay has not been parsed.")));
    }

    Ok(Json(MatchTimelineResponse {
        available: true,
        note: None,
        duration_seconds: facts.duration_seconds,
        net_worth_per_minute: facts.player.net_worth_per_minute,
        deaths: facts
            .deaths
            .into_iter()
            .map(|d| TimelineDeath {
                time_seconds: d.time_seconds,
                killer_hero_name: d.killer_hero_name,
            })
            .collect(),
    }))
}

#[utoipa::path(
    post, path = "/api/matches/{id}/parse", tag = "matches",
    summary = "Request a replay parse",
    description = "Submits a parse request to OpenDota for one of the player's matches, at most once an hour per match, and clears the cached \"not parsed yet\" answer so the next read asks again. A timeline appears once a parsed replay is available; that is not immediate and not guaranteed.",
    security(("session" = [])),
    params(
        ("id" = Uuid, Path,
            description = "Internal match id — the `id` from `/api/matches`.",
            example = "3fa85f64-5717-4562-b3fc-2c963f66afa6"),
    ),
    responses(
        (status = 200, description = "The request was submitted", body = ParseRequestResponse),
        (status = 404, description = "No such match, or it belongs to another player", body = crate::error::ErrorBody),
        (status = 429, description = "Already requested for this match within the hour", body = crate::error::ErrorBody),
        (status = 502, description = "The provider refused or is unavailable", body = crate::error::ErrorBody),
        (status = 402, description = "Trial expired and no active subscription", body = crate::error::ErrorBody),
        (status = 401, description = "No session cookie, or it has expired", body = crate::error::ErrorBody),
        (status = 500, description = "Database or internal failure", body = crate::error::ErrorBody),
    )
)]
pub async fn request_parse(
    State(state): State<AppState>,
    EntitledUser(user): EntitledUser,
    AppPath(id): AppPath<Uuid>,
) -> AppResult<Json<ParseRequestResponse>> {
    let player = load_linked_player(&state, &user).await?;
    let match_ = repositories::r#match::find_owned(&state.db, id, player.id)
        .await?
        .ok_or_else(|| AppError::NotFound("Match not found.".into()))?;

    {
        let now = Utc::now();
        let mut requested = PARSE_REQUESTS.lock().unwrap_or_else(|e| e.into_inner());
        if requested
            .get(&id)
            .is_some_and(|at| now - *at < Duration::seconds(PARSE_COOLDOWN_SECONDS))
        {
            return Err(AppError::TooManyRequests(
                "A parse was already requested for this match in the last hour.".into(),
            ));
        }
        requested.insert(id, now);
    }

    state
        .dota
        .request_parse(match_.match_id)
        .await
        .map_err(|e| AppError::Upstream(e.to_string()))?;
    if let Err(e) = repositories::match_facts::forget_unparsed(
        &state.db,
        match_.match_id,
        player.dota_account_id,
    )
    .await
    {
        tracing::warn!(error = %e, "could not clear the unparsed timeline snapshot");
    }

    Ok(Json(ParseRequestResponse {
        requested: true,
        note: "Parse requested. The timeline appears once a parsed replay is available.",
    }))
}

struct RowContext {
    baselines: rating::RatingBaselines,
    /// The median KDA of the newest ranked window, which is exactly what the
    /// momentum curve measures against. Shared so a row's delta and its point on
    /// that curve are one number.
    median_kda: f32,
    /// Hero id -> (peer median GPM, peer median XPM).
    peers: HashMap<i32, (Option<f32>, Option<f32>)>,
}

impl RowContext {
    async fn load(state: &AppState, dota_player_id: Uuid) -> AppResult<Self> {
        let baselines = repositories::metrics::rating_baselines(&state.db, dota_player_id).await?;
        let ranked = repositories::r#match::recent_ranked(
            &state.db,
            dota_player_id,
            calibration::MOMENTUM_WINDOW,
        )
        .await?;

        Ok(Self {
            baselines: rating::RatingBaselines::from_rows(&baselines),
            median_kda: calibration::recent_ranked_median_kda(&ranked),
            peers: HashMap::new(),
        })
    }

    /// Fill in peer medians for these heroes. Best effort: a provider outage
    /// leaves a row without a reference, never fails the page.
    async fn with_peers(mut self, state: &AppState, player: &DotaPlayer, heroes: &[i32]) -> Self {
        let lookups = heroes.iter().copied().map(|hero_id| async move {
            let context = BenchmarkContext {
                hero_id,
                role: None,
                rank_tier: player.rank_tier,
                bracket: None,
                patch: None,
            };
            let lookup = tokio::time::timeout(
                std::time::Duration::from_secs(5),
                state.benchmarks.get_distribution(&context),
            )
            .await;
            let Ok(Ok(distribution)) = lookup else {
                return None;
            };
            let median = |metric: BenchmarkMetric| {
                distribution
                    .buckets
                    .get(&metric)
                    .and_then(|b| percentile::value_at(0.5, b))
            };
            Some((
                hero_id,
                (median(BenchmarkMetric::GoldPerMin), median(BenchmarkMetric::XpPerMin)),
            ))
        });
        self.peers = futures::future::join_all(lookups).await.into_iter().flatten().collect();
        self
    }
}

#[derive(Serialize, ToSchema)]
pub struct MatchListResponse {
    pub matches: Vec<MatchView>,
    pub page: i64,
    pub limit: i64,
    pub total: i64,
    pub total_pages: i64,
    /// How this page was filtered and ordered — echoed back so a client can
    /// tell a page it asked for from one it inherited.
    pub filtered: bool,
    pub sort: &'static str,
    /// Which kind of game this page shows: `all`, `ranked` or `turbo`.
    pub mode: &'static str,
    /// What this population *can* be filtered by, from the player's own rows.
    pub filters: FilterOptions,
    /// Matches in this mode before any hero, role or result filter.
    pub mode_total: i64,
    /// Stored Turbo games this mode leaves out.
    pub hidden_turbo: i64,
    /// Every other stored game this mode leaves out.
    pub hidden_other: i64,
    /// Every match stored for this player, whatever mode, hero or result.
    ///
    /// The denominator behind "20 of 143": `total` describes the filtered list,
    /// this describes the history it was drawn from. Not a Dota lifetime total —
    /// the provider does not publish one, and this is only what has been synced.
    pub lifetime_games: i64,
    pub last_synced_at: Option<DateTime<Utc>>,
    /// True when this request started a background sync because the stored
    /// history had gone stale. The rows served are the cached ones either way;
    /// a client seeing this can come back for the newer page shortly.
    pub syncing: bool,
}

/// `GET /api/matches?page=1&limit=20`
#[utoipa::path(
    get, path = "/api/matches", tag = "matches",
    summary = "Stored match history",
    description = "One page of the player's stored matches, with two derived figures per row.\n\n\
`rating` is 1.0–10.0 and is measured against **this player's own** typical game on that hero, \
not against an absolute scale or a peer distribution; Turbo games are rated only against Turbo \
games. `mmr_delta_estimate` is the same disclosed model the calibrating screen's momentum curve \
plots — Valve publishes no per-match MMR, so it is an estimate and must be labelled as one, and \
it is `null` for any game that cannot move a medal.\n\n\
A read never waits on the Dota provider. When the stored history has gone stale this answers \
`syncing: true` and refreshes behind the response, under the same cooldown \
`POST /api/players/me/sync` enforces.",
    security(("session" = [])),
    params(
        ("page" = Option<i64>, Query,
            description = "1-based page number. Defaults to 1.",
            example = 1, minimum = 1),
        ("limit" = Option<i64>, Query,
            description = "Matches per page. Defaults to 20.",
            example = 20, minimum = 1, maximum = 100),
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
        ("mode" = Option<String>, Query,
            description = "`eligible` (default): Ranked and public All Pick, the newest \
matches in the same window Coach, Heroes and Benchmark read. `turbo`: Turbo games only, \
stored but never coached. `all`: every stored game. `ranked`: the ranked-lobby part of \
`eligible`, the games that move a medal. Filter option counts are taken inside the chosen \
mode.",
            example = "turbo"),
        ("sort" = Option<String>, Query,
            description = "`newest` (default), `oldest`, `gpm_desc`, `gpm_asc` or `kda_desc`. \
Matches with no computed metrics sort last under `kda_desc` rather than counting as zero.",
            example = "gpm_desc"),
    ),
    responses(
        (status = 200, description = "One page of matches, newest first unless sorted otherwise", body = MatchListResponse),
        (status = 400, description = "An unknown role, result, mode or sort value", body = crate::error::ErrorBody),
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
    let mode = ListMode::parse(query.mode.as_deref())?;
    let mut filter = query.filter()?;
    let filtered = filter.hero_id.is_some() || filter.won.is_some() || filter.role.is_some();
    let offset = (page - 1) * limit;

    // The mode is the population; the filter is a view of it. Both are pushed
    // into SQL rather than trimmed afterwards, so `total` and `total_pages`
    // describe the filtered list and the client never has to hold a page it
    // was not shown.
    let (window, mode_filter) = mode.population(state.config.roles.analysis_match_limit);
    filter.mode = mode_filter;
    let unfiltered = MatchFilter {
        mode: mode_filter,
        ..MatchFilter::default()
    };

    let count = |f: MatchFilter| {
        let (state, window) = (&state, &window);
        async move {
            if window.is_career() {
                repositories::r#match::count_by_player(&state.db, player.id, &f).await
            } else {
                repositories::r#match::count_by_player_scoped(&state.db, player.id, window, &f)
                    .await
            }
        }
    };

    let matches = if window.is_career() {
        repositories::r#match::list_by_player(&state.db, player.id, &filter, limit, offset).await?
    } else {
        repositories::r#match::list_by_player_scoped(
            &state.db, player.id, &window, &filter, limit, offset,
        )
        .await?
    };
    let total = count(filter.clone()).await?;
    let mode_total = if filtered { count(unfiltered).await? } else { total };

    // What this mode leaves out, so the header can say "+M Turbo hidden".
    let lifetime_games =
        repositories::r#match::count_by_player(&state.db, player.id, &MatchFilter::default())
            .await?;
    let hidden_turbo = match mode {
        ListMode::Eligible | ListMode::Ranked => {
            repositories::r#match::count_by_player(
                &state.db,
                player.id,
                &MatchFilter {
                    mode: ModeFilter::Turbo,
                    ..MatchFilter::default()
                },
            )
            .await?
        }
        ListMode::Turbo | ListMode::All => 0,
    };
    let hidden_other = (lifetime_games - mode_total - hidden_turbo).max(0);

    // Peer references for the heroes on this page, eligible games only.
    let mut heroes: Vec<i32> = matches
        .iter()
        .filter(|m| eligibility::is_eligible(m.game_mode, m.lobby_type))
        .map(|m| m.hero_id)
        .collect();
    heroes.sort_unstable();
    heroes.dedup();
    let context = RowContext::load(&state, player.id)
        .await?
        .with_peers(&state, &player, &heroes)
        .await;
    let syncing = refresh_in_background(&state, &user, &player);

    Ok(Json(MatchListResponse {
        matches: matches
            .into_iter()
            .map(|m| MatchView::of(m, &context, &state.config.calibration))
            .collect(),
        page,
        limit,
        total,
        total_pages: total_pages(total, limit),
        filtered,
        sort: filter.sort.slug(),
        mode: mode.slug(),
        filters: filter_options(&state, player.id, &window, mode_filter).await?,
        mode_total,
        hidden_turbo,
        hidden_other,
        lifetime_games,
        last_synced_at: player.last_synced_at,
        syncing,
    }))
}

/// Pull newer matches behind this response, if the stored history has gone stale.
///
/// Returns whether a sync was started. Reading a match list is the one moment
/// the product knows the player is looking at their history, which makes it the
/// right moment to notice it is a day old — but a read must not *wait* on a
/// provider, so the cached rows are served either way.
///
/// Three gates, and all three matter:
///
///   * staleness, so a page view does not mean a provider call;
///   * the existing sync cooldown, so this can never exceed the rate limit that
///     `POST /api/players/me/sync` enforces — one limiter, not two;
///   * an in-flight set, so twenty tabs open at once are one sync.
///
/// Deliberately *only* the sync. Unlike the explicit endpoint, this does not
/// checkpoint a coaching session: a GET that quietly created one would make the
/// coaching history depend on who opened which page.
fn refresh_in_background(state: &AppState, user: &User, player: &DotaPlayer) -> bool {
    let stale_after_seconds = state.config.dota.stale_after_seconds;
    if stale_after_seconds <= 0 {
        // Turned off: the explicit endpoint is the only path to the provider.
        return false;
    }

    // A player who has never synced has nothing stored to be fresh.
    let stale_after = Duration::seconds(stale_after_seconds);
    let fresh_enough = player
        .last_synced_at
        .is_some_and(|at| Utc::now().signed_duration_since(at) < stale_after);

    if fresh_enough || players::cooldown_remaining_for(state, player).is_some() {
        return false;
    }

    // Claimed before the task starts, so a burst of requests produces one sync.
    if !state.sync_in_flight.claim(player.id) {
        return false;
    }

    let state = state.clone();
    let user_id = user.id;
    let player = player.clone();

    tokio::spawn(async move {
        let result = sync::sync_player(
            &state.db,
            state.dota.as_ref(),
            user_id,
            &player,
            state.config.dota.sync_match_limit,
            state.config.roles.analysis_match_limit,
        )
        .await;

        // Never fatal and never surfaced: the response this was spawned from is
        // long gone, and the player's stored history is unharmed either way.
        if let Err(e) = result {
            tracing::warn!(error = %e, "background match sync failed");
        }

        state.sync_in_flight.release(player.id);
    });

    true
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
    mode: ModeFilter,
) -> AppResult<FilterOptions> {
    let heroes =
        repositories::r#match::hero_facets(&state.db, dota_player_id, window, mode).await?;
    let roles =
        repositories::r#match::role_facets(&state.db, dota_player_id, window, mode).await?;

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
        (status = 402, description = "Trial expired and no active subscription", body = crate::error::ErrorBody),
        (status = 401, description = "No session cookie, or it has expired", body = crate::error::ErrorBody),
        (status = 500, description = "Database or internal failure", body = crate::error::ErrorBody),
    )
)]
pub async fn get(
    State(state): State<AppState>,
    EntitledUser(user): EntitledUser,
    AppPath(id): AppPath<Uuid>,
) -> AppResult<Json<MatchResponse>> {
    let player = load_linked_player(&state, &user).await?;

    let match_ = repositories::r#match::find_owned(&state.db, id, player.id)
        .await?
        .ok_or_else(|| AppError::NotFound("Match not found.".into()))?;

    let context = RowContext::load(&state, player.id).await?;

    Ok(Json(MatchResponse {
        match_: MatchView::of(match_, &context, &state.config.calibration),
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
        (status = 402, description = "Trial expired and no active subscription", body = crate::error::ErrorBody),
        (status = 401, description = "No session cookie, or it has expired", body = crate::error::ErrorBody),
        (status = 500, description = "Database or internal failure", body = crate::error::ErrorBody),
    )
)]
pub async fn comparison(
    State(state): State<AppState>,
    EntitledUser(user): EntitledUser,
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

    // The personal baseline: this hero, this position, eligible games, never
    // this match. The same scope the coach's match analysis reads, so "your
    // average" is one number on the whole page.
    let baseline = MatchScope::baseline(
        match_.hero_id,
        CoachableRole::from_stored(&match_.role),
        match_.id,
        state.config.roles.analysis_match_limit,
    );
    let averages =
        repositories::metrics::hero_averages(&state.db, player.id, match_.hero_id, &baseline)
            .await?;
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

    let lane = repositories::metrics::laning_averages(&state.db, player.id, &baseline).await?;
    let laning = crate::domain::match_comparison::laning(
        match_.last_hits_at_10,
        match_.gold_at_10,
        lane.last_hits_at_10,
        lane.lh_sample,
        lane.gold_at_10,
        lane.gold_sample,
    );

    let item_timings = crate::services::item_timing::for_match(
        state.item_timing.as_ref(),
        match_.hero_id,
        match_.bkb_seconds,
        match_.blink_seconds,
        match_.midas_seconds,
    )
    .await;

    Ok(Json(MatchComparison {
        item_timings,
        laning,
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
    fn the_default_mode_is_ranked_and_all_pick() {
        assert_eq!(ListMode::parse(None).unwrap(), ListMode::Eligible);
        assert_eq!(ListMode::parse(Some("")).unwrap(), ListMode::Eligible);
        assert_eq!(ListMode::parse(Some(" eligible ")).unwrap(), ListMode::Eligible);
    }

    #[test]
    fn ranked_turbo_and_all_are_requested_by_name() {
        assert_eq!(ListMode::parse(Some("ranked")).unwrap(), ListMode::Ranked);
        assert_eq!(ListMode::parse(Some("turbo")).unwrap(), ListMode::Turbo);
        assert_eq!(ListMode::parse(Some("all")).unwrap(), ListMode::All);
    }

    /// A value the server does not understand is rejected, never quietly
    /// replaced with a population the caller did not ask for.
    #[test]
    fn an_unknown_mode_is_rejected() {
        for bad in ["competitive", "Turbo", "all_pick", "unranked"] {
            assert!(ListMode::parse(Some(bad)).is_err(), "{bad}");
        }
    }

    #[test]
    fn each_mode_reads_its_own_population() {
        // The default is the coaching window, so counts match Coach and Heroes.
        let (window, filter) = ListMode::Eligible.population(100);
        assert!(!window.is_career());
        assert_eq!(filter, ModeFilter::All);

        let (window, filter) = ListMode::Turbo.population(100);
        assert!(window.is_career());
        assert_eq!(filter, ModeFilter::Turbo);

        let (window, filter) = ListMode::All.population(100);
        assert!(window.is_career());
        assert_eq!(filter, ModeFilter::All);
    }

    #[test]
    fn every_mode_slug_round_trips() {
        for mode in [ModeFilter::All, ModeFilter::Ranked, ModeFilter::Turbo] {
            assert_eq!(ModeFilter::parse(mode.slug()), Some(mode));
        }
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

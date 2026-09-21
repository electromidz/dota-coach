//! Peer comparison for the signed-in player.
//!
//! Benchmarks are computed against the player's most-played hero by default,
//! because that is where they have enough matches for a comparison to mean
//! anything. `?hero_id=` picks a different one.

use std::collections::HashMap;

use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::api::extract::{AppPath, AppQuery, CurrentUser};
use crate::domain::benchmark::{
    BenchmarkContext, BenchmarkContextInfo, BenchmarkMetric, BenchmarkResult, Confidence,
    PopulationScope, ResolvedBracket, Segment, UnavailableSegment,
};
use crate::domain::player::DotaPlayer;
use crate::domain::role::CoachableRole;
use crate::domain::scope::MatchScope;
use crate::domain::user::User;
use crate::error::{AppError, AppResult};
use crate::repositories;
use crate::repositories::metrics::HeroAverages;
use crate::services::benchmarks::{self, BenchmarkError, PlayerValues};
use crate::state::AppState;
use utoipa::ToSchema;

#[derive(Deserialize)]
pub struct BenchmarkQuery {
    pub hero_id: Option<i32>,
    /// Role slug. Omit to follow the coaching profile, `all` to compare across
    /// every role.
    pub role: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct BenchmarkResponse {
    pub hero_id: i32,
    pub hero_name: String,
    /// Matches on this hero **inside the scope** — the sample every percentile
    /// below rests on.
    pub sample: i64,
    pub results: Vec<BenchmarkResult>,
    /// Dimensions the provider could actually segment on, repeated at the top
    /// level so a client can caveat the whole page at once.
    pub segmented_by: Vec<Segment>,
    /// What was asked for, what was delivered, and what each side of the
    /// comparison actually covers.
    pub context: BenchmarkContextInfo,
    /// Set when the whole comparison is unavailable rather than any one metric.
    pub note: Option<String>,
}

/// The dimensions the product asks to compare on. The spec's four.
const REQUESTED_SEGMENTS: [Segment; 4] = Segment::ALL;

/// Why each dimension the provider could not segment on is missing.
///
/// Worded as statements about the data source rather than apologies: a reader
/// deciding how much to trust a percentile needs to know what it is a
/// percentile *of*.
///
/// Rank is the one that varies per request, so it takes the resolved bracket:
/// "we asked for Immortal and the provider publishes nothing there" and "you
/// are unranked, so there was no bracket to ask for" are different facts, and
/// neither is the old blanket "this provider cannot do rank".
pub(crate) fn unavailability_reason(segment: Segment, bracket: ResolvedBracket) -> String {
    match segment {
        Segment::Hero => String::new(),
        Segment::Role => {
            "The benchmark provider publishes one distribution per hero and does not segment it \
             by position. Your own figures below are restricted to the selected role; the peer \
             values are not."
                .into()
        }
        Segment::RankBracket => match bracket.requested {
            Some(asked) => format!(
                "The provider publishes no distribution for this hero in {}, so these peer \
                 values cover every bracket rather than yours.",
                asked.label(),
            ),
            None => "Your rank is unknown, so there was no bracket to compare against and these \
                     peer values cover every rank."
                .into(),
        },
        Segment::Patch => {
            "The benchmark provider does not state which patch its distribution covers.".into()
        }
    }
}

/// Describe both sides of the comparison.
pub(crate) fn population_scope(
    scope: &MatchScope,
    role: Option<CoachableRole>,
    hero: &str,
    bracket: ResolvedBracket,
) -> PopulationScope {
    let player = match (role, scope.limit) {
        (Some(role), Some(limit)) => format!(
            "Your last {limit} eligible Ranked and public All Pick matches on {hero} as {}.",
            role.label(),
        ),
        (Some(role), None) => format!(
            "Your eligible Ranked and public All Pick matches on {hero} as {}.",
            role.label(),
        ),
        (None, Some(limit)) => format!(
            "Your last {limit} eligible Ranked and public All Pick matches on {hero}, any role.",
        ),
        (None, None) => {
            format!("Your eligible Ranked and public All Pick matches on {hero}, any role.")
        }
    };

    let peers = match bracket.used {
        Some(used) => format!(
            "Public matches on {hero} in the {} bracket. The provider does not publish which \
             game modes or patches that distribution covers.",
            used.label(),
        ),
        None => format!(
            "Public matches on {hero} across every rank. The provider does not publish which \
             game modes or patches that distribution covers."
        ),
    };

    PopulationScope {
        player,
        peers,
        // Rank lines up now, which is the dimension that moved these numbers
        // most — but game mode and patch still do not, and a partially matched
        // population is not a matched one.
        comparable: false,
        note: "Rank aside, the two populations are described differently and are not known to \
               match. Read these percentiles as a close placement, not a measurement.",
    }
}

/// `GET /api/benchmark`
#[utoipa::path(
    get, path = "/api/benchmark", tag = "benchmark",
    summary = "Every metric against peers",
    description = "Your own figures are averaged over the eligible Ranked and public All Pick matches in the selected role. The peer distribution is segmented by hero only — the provider ignores rank and position, verified against the live API — so `context.unavailable` names every dimension the comparison could not honour, and `context.population` describes what each side actually covers. Percentiles are omitted, not estimated, when the sample is too small.",
    security(("session" = [])),
    params(
        ("hero_id" = Option<i32>, Query,
            description = "Dota hero id. Omit to benchmark the player's most-played hero in scope.",
            example = 26, minimum = 1),
        ("role" = Option<String>, Query,
            description = "Restrict your own figures to one role. Omit to follow the coaching \
profile, or pass `all` for every role. The peer distribution is hero-segmented either way — see \
`context.unavailable`.",
            example = "carry"),
    ),
    responses(
        (status = 200, description = "Player values beside peer medians, with the context each side covers", body = BenchmarkResponse),
        (status = 502, description = "The benchmark provider is unavailable", body = crate::error::ErrorBody),
        (status = 409, description = "No Dota account linked to this user yet", body = crate::error::ErrorBody),
        (status = 401, description = "No session cookie, or it has expired", body = crate::error::ErrorBody),
        (status = 500, description = "Database or internal failure", body = crate::error::ErrorBody),
    )
)]
pub async fn overview(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    AppQuery(query): AppQuery<BenchmarkQuery>,
) -> AppResult<Json<BenchmarkResponse>> {
    let player = load_linked_player(&state, &user).await?;
    let (scope, role) = resolve_scope(&state, &player, query.role.as_deref()).await?;
    build(&state, &player, query.hero_id, None, &scope, role)
        .await
        .map(Json)
}

/// Which matches of the player's own the comparison reads.
///
/// Defaults to the role they are being coached on, because that is the
/// comparison they came for: a support player's Phantom Assassin percentile is
/// a fact about a different game than the one they are trying to improve at.
/// `?role=all` opts out, and an explicit slug overrides both.
async fn resolve_scope(
    state: &AppState,
    player: &DotaPlayer,
    requested: Option<&str>,
) -> AppResult<(MatchScope, Option<CoachableRole>)> {
    let window = state.config.roles.analysis_match_limit;

    let role = match requested.map(str::trim) {
        // Explicit opt-out: every role, still competitive-only.
        Some("all") => return Ok((MatchScope::competitive(window), None)),
        Some(slug) if !slug.is_empty() => Some(CoachableRole::parse(slug).ok_or_else(|| {
            AppError::BadRequest(format!(
                "'{slug}' is not a role. Use one of: {}, or 'all'.",
                CoachableRole::ALL
                    .iter()
                    .map(|r| r.slug())
                    .collect::<Vec<_>>()
                    .join(", "),
            ))
        })?),
        // No preference stated: follow the coaching profile if there is one.
        _ => repositories::coaching_profile::find(&state.db, player.id)
            .await?
            .map(|profile| profile.selected_role),
    };

    Ok(match role {
        Some(role) => (MatchScope::for_role(role, window), Some(role)),
        None => (MatchScope::competitive(window), None),
    })
}

/// `GET /api/benchmark/:metric` — the same comparison, narrowed to one metric.
#[utoipa::path(
    get, path = "/api/benchmark/{metric}", tag = "benchmark",
    summary = "One metric against peers",
    security(("session" = [])),
    params(
        ("metric" = String, Path,
            description = "Metric slug. One of `gold_per_min`, `xp_per_min`, `last_hits_per_min`, \
`kills_per_min`, `deaths_per_min`, `assists_per_min`, `hero_damage_per_min`, `tower_damage`.",
            example = "gold_per_min"),
        ("hero_id" = Option<i32>, Query,
            description = "Dota hero id. Omit to benchmark the player's most-played hero in scope.",
            example = 26, minimum = 1),
        ("role" = Option<String>, Query,
            description = "Restrict your own figures to one role. Omit to follow the coaching \
profile, or pass `all` for every role.",
            example = "carry"),
    ),
    responses(
        (status = 200, description = "That metric only", body = BenchmarkResponse),
        (status = 404, description = "No such metric slug", body = crate::error::ErrorBody),
        (status = 502, description = "The benchmark provider is unavailable", body = crate::error::ErrorBody),
        (status = 409, description = "No Dota account linked to this user yet", body = crate::error::ErrorBody),
        (status = 401, description = "No session cookie, or it has expired", body = crate::error::ErrorBody),
        (status = 500, description = "Database or internal failure", body = crate::error::ErrorBody),
    )
)]
pub async fn metric(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    AppPath(slug): AppPath<String>,
    AppQuery(query): AppQuery<BenchmarkQuery>,
) -> AppResult<Json<BenchmarkResponse>> {
    let wanted = BenchmarkMetric::parse(&slug)
        .ok_or_else(|| AppError::BadRequest(format!("Unknown metric '{slug}'.")))?;

    let player = load_linked_player(&state, &user).await?;
    let (scope, role) = resolve_scope(&state, &player, query.role.as_deref()).await?;
    build(&state, &player, query.hero_id, Some(wanted), &scope, role)
        .await
        .map(Json)
}

/// The comparison itself, shared with the coaching layer so an insight and the
/// benchmark page can never disagree about a percentile.
/// `scope` decides which of the player's matches their own figures are averaged
/// over. The peer distribution is whatever the provider offers and is described
/// honestly by `segmented_by`; this parameter is about our side of the
/// comparison, and getting it wrong means benchmarking a player against a
/// percentile their average does not belong to.
pub(crate) async fn build(
    state: &AppState,
    player: &DotaPlayer,
    hero_id: Option<i32>,
    only: Option<BenchmarkMetric>,
    scope: &MatchScope,
    role: Option<CoachableRole>,
) -> AppResult<BenchmarkResponse> {
    // Default to the hero with the most matches: the only one likely to clear
    // the sample floor.
    let heroes = repositories::metrics::hero_stats_scoped(&state.db, player.id, scope, 1).await?;
    let (hero_id, hero_name) = match hero_id {
        Some(id) => {
            let name = heroes
                .iter()
                .find(|h| h.hero_id == id)
                .map(|h| h.hero_name.clone())
                .unwrap_or_else(|| format!("Hero {id}"));
            (id, name)
        }
        None => match heroes.first() {
            Some(h) => (h.hero_id, h.hero_name.clone()),
            None => {
                let note = match role {
                    Some(role) => format!(
                        "No eligible {} matches yet, so there is nothing to compare.",
                        role.label(),
                    ),
                    None => "Sync some matches first — there is nothing to compare yet.".into(),
                };

                return Ok(BenchmarkResponse {
                    hero_id: 0,
                    hero_name: String::new(),
                    sample: 0,
                    results: Vec::new(),
                    segmented_by: Vec::new(),
                    context: context_info(
                        0,
                        "",
                        role,
                        player,
                        scope,
                        &[],
                        ResolvedBracket::requested_for(player.rank_tier),
                    ),
                    note: Some(note),
                });
            }
        },
    };

    let averages =
        repositories::metrics::hero_averages(&state.db, player.id, hero_id, scope).await?;
    let values = PlayerValues {
        values: player_values(&averages),
        sample: averages.sample,
    };

    // What we *ask* the provider for. It honours hero and rank bracket; role
    // and patch are a statement of intent a future provider can fill in, and
    // the response reports which dimensions actually came back.
    let context = BenchmarkContext {
        hero_id,
        role: role.map(|r| r.slug().to_string()),
        rank_tier: player.rank_tier,
        patch: None,
    };

    // A provider outage degrades the page rather than failing it: the player's
    // own numbers are local and still worth showing.
    let distribution = match state.benchmarks.get_distribution(&context).await {
        Ok(d) => d,
        Err(e) => {
            let note = match e {
                BenchmarkError::RateLimited => {
                    "The benchmark provider is rate limiting us. Try again shortly."
                }
                BenchmarkError::NotFound => "No benchmark data exists for this hero yet.",
                _ => "Benchmarks are unavailable right now.",
            };
            tracing::warn!(error = %e, hero_id, "benchmark distribution unavailable");

            return Ok(BenchmarkResponse {
                context: context_info(
                    hero_id,
                    &hero_name,
                    role,
                    player,
                    scope,
                    &[],
                    // Nothing came back, so nothing was resolved. Reporting the
                    // requested bracket keeps the caveat about *which* peers
                    // are missing accurate.
                    ResolvedBracket::requested_for(player.rank_tier),
                ),
                hero_id,
                hero_name,
                sample: averages.sample,
                results: bare_results(&values, only),
                segmented_by: Vec::new(),
                note: Some(note.to_string()),
            });
        }
    };

    let segmented_by = distribution.segmented_by.clone();
    let bracket = distribution.bracket;
    let mut results = benchmarks::compare(&values, &distribution);
    if let Some(wanted) = only {
        results.retain(|r| r.metric == wanted);
    }

    Ok(BenchmarkResponse {
        context: context_info(
            hero_id,
            &hero_name,
            role,
            player,
            scope,
            &segmented_by,
            bracket,
        ),
        hero_id,
        hero_name,
        sample: averages.sample,
        results,
        segmented_by,
        note: None,
    })
}

/// What was asked for, what arrived, and what each side covers.
///
/// Built on every path — including the two failure paths above — because a
/// response that omits it when the provider is down is a response whose caveats
/// disappear exactly when they matter most.
pub(crate) fn context_info(
    hero_id: i32,
    hero_name: &str,
    role: Option<CoachableRole>,
    player: &DotaPlayer,
    scope: &MatchScope,
    segmented_by: &[Segment],
    bracket: ResolvedBracket,
) -> BenchmarkContextInfo {
    let unavailable = REQUESTED_SEGMENTS
        .into_iter()
        .filter(|segment| !segmented_by.contains(segment))
        .map(|segment| UnavailableSegment {
            segment,
            label: segment.label(),
            reason: unavailability_reason(segment, bracket),
        })
        .collect();

    BenchmarkContextInfo {
        hero_id,
        hero_name: hero_name.to_string(),
        role,
        role_label: role.map(CoachableRole::label),
        rank_tier: player.rank_tier,
        bracket,
        requested: REQUESTED_SEGMENTS.to_vec(),
        segmented_by: segmented_by.to_vec(),
        unavailable,
        population: population_scope(scope, role, hero_name, bracket),
    }
}

/// The player's figures with no distribution to place them in.
fn bare_results(values: &PlayerValues, only: Option<BenchmarkMetric>) -> Vec<BenchmarkResult> {
    BenchmarkMetric::ALL
        .into_iter()
        .filter(|m| only.is_none_or(|wanted| *m == wanted))
        .filter_map(|metric| {
            let player_value = *values.values.get(&metric)?;
            Some(BenchmarkResult {
                metric,
                label: metric.label(),
                higher_is_better: metric.higher_is_better(),
                player_value,
                player_sample: values.sample,
                peer_median: None,
                top_20_value: None,
                percentile: None,
                gap_to_top_20: None,
                peer_sample_size: None,
                confidence: Confidence::Insufficient,
                segmented_by: Vec::new(),
                note: None,
            })
        })
        .collect()
}

/// Map the SQL averages onto metric keys.
///
/// A thin alias over [`BenchmarkFigures::values`], which a single match uses
/// too — the mapping lives beside the columns it reads so a unit mismatch has
/// only one place to hide.
pub(crate) fn player_values(a: &HeroAverages) -> HashMap<BenchmarkMetric, f32> {
    a.figures.values()
}

async fn load_linked_player(state: &AppState, user: &User) -> AppResult<DotaPlayer> {
    repositories::dota_player::find_by_user_id(&state.db, user.id)
        .await?
        .ok_or(AppError::DotaAccountNotLinked)
}

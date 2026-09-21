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
    PopulationScope, ResolvedBracket, Segment, TargetComparison, TargetMetric, UnavailableSegment,
};
use crate::domain::hero::RankBracket;
use crate::domain::player::DotaPlayer;
use crate::domain::role::CoachableRole;
use crate::domain::scope::MatchScope;
use crate::domain::user::User;
use crate::error::{AppError, AppResult};
use crate::repositories;
use crate::repositories::metrics::HeroAverages;
use crate::services::benchmarks::{self, percentile, BenchmarkError, PlayerValues};
use crate::state::AppState;
use utoipa::ToSchema;

#[derive(Deserialize)]
pub struct BenchmarkQuery {
    pub hero_id: Option<i32>,
    /// Role slug. Omit to follow the coaching profile, `all` to compare across
    /// every role.
    pub role: Option<String>,
    /// Rank bracket to aim at. Omit for the next one up, `none` for no target.
    pub bracket: Option<String>,
}

/// Which bracket a request wants held up beside the player's own.
///
/// Three states rather than an `Option`, because "I did not say" and "I said
/// no" want different answers: the first gets the progression the page exists
/// to show, the second gets the plain single-bracket comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetChoice {
    /// Nothing asked for: the server picks the next bracket up.
    Default,
    /// Explicitly none — just my own rank.
    None,
    Named(RankBracket),
}

impl BenchmarkQuery {
    /// The bracket this request wants to be measured *against*.
    ///
    /// An unrecognised slug is rejected rather than ignored: silently serving
    /// the next bracket up to someone who asked for Divine would put a number
    /// on screen under the wrong heading.
    fn target(&self) -> AppResult<TargetChoice> {
        match self.bracket.as_deref().map(str::trim) {
            None | Some("") => Ok(TargetChoice::Default),
            Some("none") => Ok(TargetChoice::None),
            Some(slug) => RankBracket::parse(slug)
                .map(TargetChoice::Named)
                .ok_or_else(|| {
                    AppError::BadRequest(format!(
                        "'{slug}' is not a rank bracket. Use one of: {}, or 'none'.",
                        RankBracket::ALL
                            .iter()
                            .map(|b| b.slug())
                            .collect::<Vec<_>>()
                            .join(", "),
                    ))
                }),
        }
    }
}

/// The bracket a player is climbing towards: the next one above their own.
///
/// `None` for an Immortal player, who has nothing above them, and for an
/// unranked one, whose own bracket is not known — an honest absence in both
/// cases rather than a guess at where they belong.
fn default_target(rank_tier: Option<i32>) -> Option<RankBracket> {
    let own = rank_tier.and_then(RankBracket::from_rank_tier)?;
    RankBracket::from_index(own.index() + 1)
}

/// A bracket the peer distribution can be asked for.
///
/// Served rather than hardcoded in the client for the same reason roles and
/// heroes are: there is one list of Dota ranks in this system, it lives in
/// `domain::hero`, and a second copy in TypeScript would be a second thing to
/// keep in step.
#[derive(Serialize, ToSchema)]
pub struct BracketOption {
    pub value: &'static str,
    pub label: &'static str,
    /// True for the bracket the player's own rank falls in, so a client can
    /// mark it without re-deriving `rank_tier / 10`.
    pub is_player_rank: bool,
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
    /// comparison actually covers. Always describes the player's **own**
    /// bracket — `target` is what they are aiming at.
    pub context: BenchmarkContextInfo,
    /// The bracket being aimed at, when there is one and the provider has data
    /// for it. Never a substitute for `results`.
    pub target: Option<TargetComparison>,
    /// Every bracket that can be aimed at, in rank order.
    pub brackets: Vec<BracketOption>,
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
        ("bracket" = Option<String>, Query,
            description = "The rank bracket to hold up *beside* your own — `herald` … `immortal`, \
or `none` for no target. Omitted means the next bracket up, which is what the page shows by \
default. It never replaces your own bracket: `results` and every percentile in them always \
describe the peers you actually play against, and the target arrives separately in `target`. \
Where the provider publishes no distribution for that hero in that bracket, `target` is `null` \
rather than the all-ranks numbers under a bracket's name.",
            example = "ancient"),
    ),
    responses(
        (status = 200, description = "Player values beside peer medians, with the context each side covers", body = BenchmarkResponse),
        (status = 400, description = "An unknown role or rank bracket", body = crate::error::ErrorBody),
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
    build(
        &state,
        &player,
        query.hero_id,
        None,
        &scope,
        role,
        query.target()?,
    )
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
        ("bracket" = Option<String>, Query,
            description = "The rank bracket to hold up beside your own — `herald` … `immortal`, \
or `none`. Omitted means the next bracket up. `results` always describe your own bracket; the \
target arrives in `target`, narrowed to this same metric.",
            example = "ancient"),
    ),
    responses(
        (status = 200, description = "That metric only", body = BenchmarkResponse),
        (status = 400, description = "An unknown role or rank bracket", body = crate::error::ErrorBody),
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
    build(
        &state,
        &player,
        query.hero_id,
        Some(wanted),
        &scope,
        role,
        query.target()?,
    )
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
/// `target` is the bracket held up *beside* the player's own, never instead of
/// it: `results` always describe their own bracket, so a percentile does not
/// move because the reader got curious about Divine. [`TargetChoice::None`] is
/// the coaching default — an insight is about where a player stands, not where
/// they would like to.
pub(crate) async fn build(
    state: &AppState,
    player: &DotaPlayer,
    hero_id: Option<i32>,
    only: Option<BenchmarkMetric>,
    scope: &MatchScope,
    role: Option<CoachableRole>,
    target: TargetChoice,
) -> AppResult<BenchmarkResponse> {
    let wanted_target = match target {
        TargetChoice::None => None,
        TargetChoice::Default => default_target(player.rank_tier),
        TargetChoice::Named(bracket) => Some(bracket),
    };
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
                    // Nothing of the player's to compare, so nothing to aim at.
                    target: None,
                    brackets: bracket_options(player.rank_tier),
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
    // The player's own bracket, resolved from their rank exactly as it always
    // was. The target below is a *second* lookup; it never displaces this one,
    // which is what keeps `results` and every percentile in them a statement
    // about where the player actually stands.
    let context = BenchmarkContext {
        hero_id,
        role: role.map(|r| r.slug().to_string()),
        rank_tier: player.rank_tier,
        bracket: None,
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
                // The provider is down for this hero in every bracket, not just
                // the player's. There is no target to show either.
                target: None,
                brackets: bracket_options(player.rank_tier),
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

    let target = match wanted_target {
        // Already the player's own bracket. A second identical column would be
        // noise dressed as a comparison.
        Some(aim) if bracket.used == Some(aim) => None,
        Some(aim) => target_comparison(state, &context, &values, aim, only).await,
        None => None,
    };

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
        target,
        brackets: bracket_options(player.rank_tier),
        note: None,
    })
}

/// The bracket the player is aiming at, measured with the same arithmetic.
///
/// Returns `None` rather than an error on three paths, because a target is an
/// extra and must never cost the comparison the player came for:
///
///   - the provider is unavailable for that bracket;
///   - it publishes nothing there, so the lookup fell back to all ranks. An
///     all-ranks median displayed under the heading "Ancient" would be exactly
///     the fabrication the rest of this engine refuses, and a fallback is not
///     an answer to "what does Ancient look like";
///   - it has no median for any metric, leaving nothing to be short of.
///
/// Every figure comes from [`benchmarks::compare`] — the same function that
/// produced `results` — so the two columns cannot disagree about a percentile.
async fn target_comparison(
    state: &AppState,
    context: &BenchmarkContext,
    values: &PlayerValues,
    aim: RankBracket,
    only: Option<BenchmarkMetric>,
) -> Option<TargetComparison> {
    let context = BenchmarkContext {
        bracket: Some(aim),
        ..context.clone()
    };

    let distribution = match state.benchmarks.get_distribution(&context).await {
        Ok(distribution) => distribution,
        Err(e) => {
            tracing::debug!(error = %e, bracket = aim.slug(), "no target distribution");
            return None;
        }
    };

    if distribution.bracket.fell_back {
        return None;
    }

    let metrics: Vec<TargetMetric> = benchmarks::compare(values, &distribution)
        .into_iter()
        .filter(|result| only.is_none_or(|wanted| result.metric == wanted))
        .map(|result| {
            // The same signed, direction-aware distance the top-20% gap uses,
            // pointed at the median instead. Positive is always work to do.
            let gap = result.peer_median.map(|median| {
                percentile::gap_to_top(result.player_value, median, result.higher_is_better)
            });

            TargetMetric {
                metric: result.metric,
                label: result.label,
                higher_is_better: result.higher_is_better,
                peer_median: result.peer_median,
                top_20_value: result.top_20_value,
                percentile: result.percentile,
                gap_to_median: gap,
                cleared: gap.is_some_and(|gap| gap <= 0.0),
            }
        })
        .collect();

    let compared = metrics.iter().filter(|m| m.gap_to_median.is_some()).count();
    if compared == 0 {
        return None;
    }

    Some(TargetComparison {
        bracket: distribution.bracket,
        label: aim.label(),
        metrics_cleared: metrics.iter().filter(|m| m.cleared).count() as i64,
        metrics_compared: compared as i64,
        metrics,
    })
}

/// Every bracket, in rank order, with the player's own marked.
fn bracket_options(rank_tier: Option<i32>) -> Vec<BracketOption> {
    let own = rank_tier.and_then(RankBracket::from_rank_tier);

    RankBracket::ALL
        .into_iter()
        .map(|bracket| BracketOption {
            value: bracket.slug(),
            label: bracket.label(),
            is_player_rank: own == Some(bracket),
        })
        .collect()
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a query with only the bracket set, the way a client would.
    fn query(bracket: Option<&str>) -> BenchmarkQuery {
        BenchmarkQuery {
            hero_id: None,
            role: None,
            bracket: bracket.map(str::to_string),
        }
    }

    #[test]
    fn the_default_target_is_the_next_bracket_up() {
        // rank_tier is medal * 10 + stars, so 55 is Legend 5.
        assert_eq!(default_target(Some(55)), Some(RankBracket::Ancient));
        assert_eq!(default_target(Some(11)), Some(RankBracket::Guardian));
        assert_eq!(default_target(Some(71)), Some(RankBracket::Immortal));
    }

    #[test]
    fn nobody_is_given_a_target_that_does_not_exist() {
        // Immortal has nothing above it.
        assert_eq!(default_target(Some(80)), None);
        // And an unranked player has no rung to climb from. Defaulting them to
        // Herald would invent a starting point the provider never reported.
        assert_eq!(default_target(None), None);
        assert_eq!(default_target(Some(0)), None);
    }

    #[test]
    fn an_absent_bracket_means_the_default_and_none_means_none() {
        // The distinction the whole three-state exists for: saying nothing gets
        // the progression, saying "none" opts out of it.
        assert_eq!(query(None).target().unwrap(), TargetChoice::Default);
        assert_eq!(query(Some("")).target().unwrap(), TargetChoice::Default);
        assert_eq!(query(Some("none")).target().unwrap(), TargetChoice::None);
        assert_eq!(
            query(Some("ancient")).target().unwrap(),
            TargetChoice::Named(RankBracket::Ancient),
        );
    }

    #[test]
    fn an_unusable_bracket_is_rejected_rather_than_defaulted() {
        for slug in ["titan", "all", "9", "ancien"] {
            assert!(
                query(Some(slug)).target().is_err(),
                "'{slug}' must not quietly become the default target",
            );
        }
    }

    #[test]
    fn every_bracket_is_offered_and_exactly_one_is_the_players_own() {
        let options = bracket_options(Some(55));

        assert_eq!(options.len(), RankBracket::ALL.len());
        let own: Vec<&str> = options
            .iter()
            .filter(|o| o.is_player_rank)
            .map(|o| o.value)
            .collect();
        assert_eq!(own, vec!["legend"]);

        // An unranked player's own bracket is not guessed at.
        assert!(bracket_options(None).iter().all(|o| !o.is_player_rank));
    }
}

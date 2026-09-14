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
    BenchmarkContext, BenchmarkMetric, BenchmarkResult, Confidence, Segment,
};
use crate::domain::player::DotaPlayer;
use crate::domain::user::User;
use crate::error::{AppError, AppResult};
use crate::repositories;
use crate::repositories::metrics::HeroAverages;
use crate::services::benchmarks::{self, BenchmarkError, PlayerValues};
use crate::state::AppState;

#[derive(Deserialize)]
pub struct BenchmarkQuery {
    pub hero_id: Option<i32>,
}

#[derive(Serialize)]
pub struct BenchmarkResponse {
    pub hero_id: i32,
    pub hero_name: String,
    /// Matches on this hero — the sample every percentile below rests on.
    pub sample: i64,
    pub results: Vec<BenchmarkResult>,
    /// Dimensions the provider could actually segment on, repeated at the top
    /// level so a client can caveat the whole page at once.
    pub segmented_by: Vec<Segment>,
    /// Set when the whole comparison is unavailable rather than any one metric.
    pub note: Option<String>,
}

/// `GET /api/benchmark`
pub async fn overview(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    AppQuery(query): AppQuery<BenchmarkQuery>,
) -> AppResult<Json<BenchmarkResponse>> {
    let player = load_linked_player(&state, &user).await?;
    build(&state, &player, query.hero_id, None).await.map(Json)
}

/// `GET /api/benchmark/:metric` — the same comparison, narrowed to one metric.
pub async fn metric(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    AppPath(slug): AppPath<String>,
    AppQuery(query): AppQuery<BenchmarkQuery>,
) -> AppResult<Json<BenchmarkResponse>> {
    let wanted = BenchmarkMetric::parse(&slug)
        .ok_or_else(|| AppError::BadRequest(format!("Unknown metric '{slug}'.")))?;

    let player = load_linked_player(&state, &user).await?;
    build(&state, &player, query.hero_id, Some(wanted))
        .await
        .map(Json)
}

/// The comparison itself, shared with the coaching layer so an insight and the
/// benchmark page can never disagree about a percentile.
pub(crate) async fn build(
    state: &AppState,
    player: &DotaPlayer,
    hero_id: Option<i32>,
    only: Option<BenchmarkMetric>,
) -> AppResult<BenchmarkResponse> {
    // Default to the hero with the most matches: the only one likely to clear
    // the sample floor.
    let heroes = repositories::metrics::hero_stats(&state.db, player.id, 1).await?;
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
                return Ok(BenchmarkResponse {
                    hero_id: 0,
                    hero_name: String::new(),
                    sample: 0,
                    results: Vec::new(),
                    segmented_by: Vec::new(),
                    note: Some("Sync some matches first — there is nothing to compare yet.".into()),
                })
            }
        },
    };

    let averages = repositories::metrics::hero_averages(&state.db, player.id, hero_id).await?;
    let values = PlayerValues {
        values: player_values(&averages),
        sample: averages.sample,
    };

    let context = BenchmarkContext {
        hero_id,
        role: None,
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
    let mut results = benchmarks::compare(&values, &distribution);
    if let Some(wanted) = only {
        results.retain(|r| r.metric == wanted);
    }

    Ok(BenchmarkResponse {
        hero_id,
        hero_name,
        sample: averages.sample,
        results,
        segmented_by,
        note: None,
    })
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

/// Map the SQL averages onto metric keys, dropping anything the player has no
/// data for rather than sending a zero.
///
/// Shared with Hero Intelligence, which needs the same player-side values to
/// derive a per-hero percentile; duplicating the mapping would be a second
/// place for a unit mismatch to hide.
pub(crate) fn player_values(a: &HeroAverages) -> HashMap<BenchmarkMetric, f32> {
    let pairs = [
        (BenchmarkMetric::GoldPerMin, a.gold_per_min),
        (BenchmarkMetric::XpPerMin, a.xp_per_min),
        (BenchmarkMetric::LastHitsPerMin, a.last_hits_per_min),
        (BenchmarkMetric::KillsPerMin, a.kills_per_min),
        (BenchmarkMetric::DeathsPerMin, a.deaths_per_min),
        (BenchmarkMetric::AssistsPerMin, a.assists_per_min),
        (BenchmarkMetric::HeroDamagePerMin, a.hero_damage_per_min),
        (BenchmarkMetric::TowerDamage, a.tower_damage),
    ];

    pairs
        .into_iter()
        .filter_map(|(metric, value)| value.map(|v| (metric, v as f32)))
        .collect()
}

async fn load_linked_player(state: &AppState, user: &User) -> AppResult<DotaPlayer> {
    repositories::dota_player::find_by_user_id(&state.db, user.id)
        .await?
        .ok_or(AppError::DotaAccountNotLinked)
}

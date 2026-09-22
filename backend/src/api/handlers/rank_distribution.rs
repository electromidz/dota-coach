//! Where the player's measured numbers sit against every rank bracket.
//!
//! # Why this is its own route
//!
//! `/api/calibration` is deliberately provider-free: it reads stored matches
//! and stored rank readings, so it answers while OpenDota is down. This one
//! cannot — a placement against Archon's peers requires Archon's peer
//! distribution, which only the provider has. Keeping them apart means a
//! benchmark outage costs this panel and nothing else, rather than taking the
//! player's own rank and trajectory down with it.
//!
//! # What the numbers mean
//!
//! A **measurement**, not a prediction. "Archon 62nd" says the player's own
//! figures beat 62% of Archon players on the metrics that could be compared.
//! It does not say they have a 62% chance of calibrating Archon — that would
//! need a model fitted against real calibration outcomes, and Valve publishes
//! none. `closest` names the bracket whose peers they most resemble, which is
//! the bracket they are merely average in.
//!
//! Peer distributions are per-hero, so this describes the player on one hero —
//! their most-played — and says which one rather than implying a career-wide
//! verdict.

use axum::extract::State;
use axum::Json;

use crate::api::extract::CurrentUser;
use crate::api::handlers::benchmark::player_values;
use crate::api::handlers::stats::load_linked_player;
use crate::domain::benchmark::BenchmarkContext;
use crate::domain::calibration::{BracketFit, RankDistribution};
use crate::domain::hero::RankBracket;
use crate::domain::scope::MatchScope;
use crate::error::AppResult;
use crate::repositories;
use crate::repositories::r#match::MatchFilter;
use crate::services::benchmarks::{self, PlayerValues};
use crate::services::calibration;
use crate::state::AppState;

#[utoipa::path(
    get, path = "/api/calibration/brackets", tag = "players",
    summary = "Where the player's numbers sit against every rank bracket",
    description = "For each medal, the player's own figures compared against that bracket's real peer distribution — a measurement, not a prediction of where they will calibrate. Drawn from their most-played hero, because peer distributions are per-hero. A bracket the provider has no data for reports a null percentile rather than a default, and a provider outage empties this panel without affecting /api/calibration.",
    security(("session" = [])),
    responses(
        (status = 200, description = "One placement per bracket, Herald first", body = RankDistribution),
        (status = 409, description = "No Dota account linked to this user yet", body = crate::error::ErrorBody),
        (status = 401, description = "No session cookie, or it has expired", body = crate::error::ErrorBody),
        (status = 500, description = "Database or internal failure", body = crate::error::ErrorBody),
    )
)]
pub async fn get(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> AppResult<Json<RankDistribution>> {
    let player = load_linked_player(&state, &user).await?;
    let scope = MatchScope::competitive(state.config.roles.analysis_match_limit);

    // The most-played hero: the only one likely to clear the sample floor, and
    // the same default `/api/benchmark` uses so the two screens describe the
    // same hero unless the player asks otherwise.
    let heroes = repositories::metrics::hero_stats_scoped(&state.db, player.id, &scope, 1).await?;
    let Some(top) = heroes.first() else {
        return Ok(Json(RankDistribution {
            hero_id: 0,
            hero_name: String::new(),
            sample: 0,
            fits: Vec::new(),
            closest: None,
            resemblance: Vec::new(),
            own_bracket_metrics: Vec::new(),
            consistency: None,
            note: Some(
                "Sync some matches first — there is nothing to place against a bracket yet.".into(),
            ),
        }));
    };

    let averages =
        repositories::metrics::hero_averages(&state.db, player.id, top.hero_id, &scope).await?;
    let values = PlayerValues {
        values: player_values(&averages),
        sample: averages.sample,
    };

    let mut fits: Vec<BracketFit> = Vec::with_capacity(RankBracket::ALL.len());
    let mut outages = 0;
    // Kept from the loop rather than fetched again: the player's own bracket
    // is already one of the eight, and asking twice would double a provider
    // call to learn something we just computed.
    let mut own_bracket_metrics = Vec::new();

    for bracket in RankBracket::ALL {
        // One lookup per bracket. Each is cached in `benchmark_snapshots` for
        // `BENCHMARK_TTL_HOURS` and shared across every user, so the eight
        // calls are paid once a day by whoever asks first, not per request.
        let context = BenchmarkContext {
            hero_id: top.hero_id,
            role: None,
            rank_tier: player.rank_tier,
            bracket: Some(bracket),
            patch: None,
        };

        let (percentile, metrics_used, sample_size) =
            match state.benchmarks.get_distribution(&context).await {
                Ok(distribution) => {
                    // A provider that answered with a *different* bracket has
                    // not answered this question. Counting a fallback cohort
                    // as this bracket's peers is how a placement against
                    // Archon quietly becomes a placement against everyone.
                    if distribution.bracket.used != Some(bracket) {
                        (None, 0, distribution.sample_size)
                    } else {
                        let results = benchmarks::compare(&values, &distribution);
                        let (percentile, used) = calibration::bracket_placement(&results);

                        if player
                            .rank_tier
                            .and_then(RankBracket::from_rank_tier)
                            .is_some_and(|own| own == bracket)
                        {
                            own_bracket_metrics = results.clone();
                        }

                        (percentile, used, distribution.sample_size)
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        hero_id = top.hero_id,
                        bracket = bracket.slug(),
                        "bracket distribution unavailable"
                    );
                    outages += 1;
                    (None, 0, None)
                }
            };

        fits.push(BracketFit {
            bracket,
            label: bracket.label(),
            percentile,
            metrics_used,
            sample_size,
            is_player_bracket: player
                .rank_tier
                .and_then(RankBracket::from_rank_tier)
                .is_some_and(|own| own == bracket),
        });
    }

    let closest = calibration::closest_bracket(&fits);
    let resemblance = calibration::resemblance(&fits);

    // Local arithmetic over stored matches, so it survives a provider outage
    // that empties every bar above it.
    let window = MatchScope::competitive(state.config.roles.analysis_match_limit);
    let recent = repositories::r#match::list_by_player_scoped(
        &state.db,
        player.id,
        &window,
        &MatchFilter::default(),
        state.config.roles.analysis_match_limit,
        0,
    )
    .await?;
    let consistency = calibration::consistency(&recent);
    let placed = fits.iter().filter(|f| f.percentile.is_some()).count();

    // Said plainly rather than left to be inferred from a short chart: a
    // panel with three bars because five lookups failed looks exactly like a
    // panel with three bars because the player is unusual.
    let note = if placed == 0 && outages > 0 {
        Some("Benchmarks are unavailable right now, so there is nothing to place against.".into())
    } else if placed == 0 {
        Some(format!(
            "Not enough {} matches yet to place against any bracket.",
            top.hero_name
        ))
    } else if outages > 0 {
        Some(format!(
            "{outages} of {} brackets could not be fetched and are shown without a placement.",
            RankBracket::ALL.len()
        ))
    } else {
        None
    };

    Ok(Json(RankDistribution {
        hero_id: top.hero_id,
        hero_name: top.hero_name.clone(),
        sample: averages.sample,
        fits,
        closest,
        resemblance,
        own_bracket_metrics,
        consistency,
        note,
    }))
}

//! Rank calibration: where the player actually stands, and how sure of it we
//! are.
//!
//! Free, like `/api/stats` and `/api/benchmark`. This is arithmetic over
//! matches the player already synced — no model call, nothing that costs money
//! to answer — and a trial-expired account being unable to see its own medal
//! would be a strange thing to sell.
//!
//! The population is **ranked matches only**, narrower than the competitive
//! one the dashboard uses. See `services::calibration` for why.
//!
//! # What is measured and what is modeled
//!
//! `established_rank` and every `estimated: false` trajectory point are real:
//! they come from `rank_snapshots` rows written when the sync read the
//! player's profile. Everything flagged `estimated` is a disclosed model,
//! because Valve stopped publishing per-match MMR and no public source can
//! recover it. The `methodology` block travels with every response so a client
//! never has to hardcode the formula it discloses.

use axum::extract::State;
use axum::Json;
use serde::Serialize;
use utoipa::ToSchema;

use crate::api::extract::CurrentUser;
use crate::api::handlers::stats::load_linked_player;
use crate::domain::calibration::{
    estimate_mmr, rank_label, EstablishedRank, Methodology, Momentum, RankConfidence,
    RolePreference, Streak, TrajectoryPoint,
};
use crate::error::{AppError, AppResult};
use crate::repositories;
use crate::repositories::r#match::MatchFilter;
use crate::services::calibration::{self, CALIBRATION_VERSION};
use crate::state::AppState;

#[derive(Serialize, ToSchema)]
pub struct CalibrationResponse {
    pub established_rank: EstablishedRank,
    pub confidence: RankConfidence,
    /// Oldest first. Every point carries an explicit `estimated` flag; a
    /// client that renders the two alike is presenting a guess as Valve's
    /// number.
    pub trajectory: Vec<TrajectoryPoint>,
    pub streak: Streak,
    /// Modeled MMR movement across the most recent ranked matches, oldest
    /// first. Relative to zero — never an absolute rating.
    pub momentum: Momentum,
    /// Most-played role first. Empty when no ranked match is in the window.
    pub role_preference: Vec<RolePreference>,
    pub methodology: Methodology,
    pub calibration_version: i32,
}

#[utoipa::path(
    get, path = "/api/calibration", tag = "players",
    summary = "Established rank, rank confidence and rank trajectory",
    description = "Deterministic, computed from synced matches and the daily rank readings taken during sync. Ranked lobbies only — Turbo, unranked public and tournament games do not move a medal and are excluded. `established_rank` and every trajectory point with `estimated: false` are measured; points flagged `estimated: true` are a disclosed model, because Valve does not publish per-match MMR. The `methodology` block reports the model actually in force. No model call is involved.",
    security(("session" = [])),
    responses(
        (status = 200, description = "Rank, confidence, trajectory, streak and role split", body = CalibrationResponse),
        (status = 409, description = "No Dota account linked, or no matches synced yet", body = crate::error::ErrorBody),
        (status = 401, description = "No session cookie, or it has expired", body = crate::error::ErrorBody),
        (status = 500, description = "Database or internal failure", body = crate::error::ErrorBody),
    )
)]
pub async fn get(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> AppResult<Json<CalibrationResponse>> {
    let player = load_linked_player(&state, &user).await?;
    let config = &state.config.calibration;

    let now = chrono::Utc::now();
    // The confidence decay window bounds everything on this screen: a match
    // older than it cannot affect the count, and a rank reading older than it
    // describes a player who has since been recalibrated.
    let since = now - chrono::Duration::days(config.decay_days);

    let matches = repositories::r#match::list_since(&state.db, player.id, since).await?;

    // An empty window is not the same thing as an empty history. A player who
    // stopped queueing a year ago has matches, they are just all older than
    // the decay window — and the honest answer for them is zero confidence and
    // a short chart, not "sync your matches". Only a genuinely empty history
    // is a precondition failure, and the extra count is only paid to tell the
    // two apart.
    if matches.is_empty()
        && repositories::r#match::count_by_player(&state.db, player.id, &MatchFilter::default())
            .await?
            == 0
    {
        return Err(AppError::PreconditionUnmet(
            "Sync your matches first — there is no history to calibrate against yet.".into(),
        ));
    }

    let snapshots =
        repositories::rank_snapshots::list_for_player(&state.db, player.dota_account_id, since)
            .await?;

    // The newest reading that actually reported a rank. Preferred over
    // `dota_players.rank_tier` so the headline medal and the last real point
    // on the chart are the same fact rather than two reads of it.
    let latest = snapshots.iter().rev().find(|s| s.rank_tier.is_some());

    let rank_tier = latest
        .and_then(|s| s.rank_tier)
        .map(i32::from)
        .or(player.rank_tier);

    let established_rank = EstablishedRank {
        rank_tier,
        label: rank_tier.and_then(rank_label),
        leaderboard_rank: latest.and_then(|s| s.leaderboard_rank),
        mmr: rank_tier.and_then(estimate_mmr),
    };

    Ok(Json(CalibrationResponse {
        established_rank,
        // `matches_counted` on this is the ranked sample behind every other
        // figure here, so a thin trajectory reads as "not enough games yet"
        // rather than as a flat one.
        confidence: calibration::rank_confidence(&matches, config, now),
        trajectory: calibration::trajectory(&snapshots, &matches, config),
        streak: calibration::streak(&matches),
        momentum: calibration::momentum(&matches, config),
        role_preference: calibration::role_preference(&matches),
        methodology: calibration::methodology(config),
        calibration_version: CALIBRATION_VERSION,
    }))
}

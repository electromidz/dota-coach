//! The overall player analysis.
//!
//! Every number here is computed in SQL over stored metrics — the client does
//! no arithmetic, and neither does the LLM.
//!
//! The population is **the competitive one**: the latest eligible Ranked and
//! public All Pick matches, and nothing else. That is a change from this
//! endpoint's original career-wide meaning, and it is deliberate: "how am I
//! performing" is a question about the ladder, and a win rate that quietly
//! included Turbo answered a different one.
//!
//! The career history has not gone anywhere — `/api/matches` still lists every
//! stored game — and the `eligibility` block below accounts for the difference
//! match for match, so the two can never appear to contradict each other
//! without saying why.

use axum::extract::State;
use axum::Json;
use serde::Serialize;

use crate::api::extract::CurrentUser;
use crate::domain::eligibility::{self, EligibilitySummary};
use crate::domain::metrics::{HeroStats, PlayerStats};
use crate::domain::player::DotaPlayer;
use crate::domain::role::RoleAnalysis;
use crate::domain::scope::{MatchScope, SampleConfidence};
use crate::domain::user::User;
use crate::error::{AppError, AppResult};
use crate::repositories;
use crate::services::metrics::METRICS_VERSION;
use crate::services::roles;
use crate::state::AppState;
use utoipa::ToSchema;

/// Heroes returned by `/api/stats`. The full list lives on the heroes page.
const TOP_HEROES: i64 = 8;

/// What the numbers in this response were computed over.
///
/// Present on every response so a client never has to assume, and so the
/// dashboard can say which games it read in the same words every time.
#[derive(Serialize, ToSchema)]
pub struct AnalysisScopeInfo {
    /// Stable identifier for the population, for a client that branches on it.
    pub population: &'static str,
    /// One sentence naming the population, for a client that renders it.
    pub description: &'static str,
    /// Eligible matches actually read, after filtering and after the window.
    pub analyzed_matches: i64,
    /// The ceiling on that number — the window, not the sample.
    pub window_limit: i64,
    pub confidence: SampleConfidence,
    pub confidence_label: &'static str,
    pub confidence_caveat: &'static str,
}

#[derive(Serialize, ToSchema)]
pub struct StatsResponse {
    pub scope: AnalysisScopeInfo,
    /// Every stored match accounted for: how many were eligible, and what the
    /// rest were.
    pub eligibility: EligibilitySummary,
    pub overall: PlayerStats,
    pub heroes: Vec<HeroStats>,
    /// Per-role performance, the unclassified count, and the advisory pick.
    ///
    /// Replaces the old `roles` array. Two role rollups in one payload would be
    /// two answers to the same question, computed differently — which is the
    /// failure the single scope exists to prevent.
    pub role_analysis: RoleAnalysis,
    /// Which formula set produced these numbers.
    pub metrics_version: i32,
}

/// `GET /api/stats`
#[utoipa::path(
    get, path = "/api/stats", tag = "stats",
    summary = "Overall performance across eligible competitive matches",
    description = "Computed in SQL over stored metrics for the latest Ranked and public All Pick matches only — Turbo, Ability Draft, custom and event games are excluded before anything is averaged. The `eligibility` block accounts for every stored match that did not make it in. No model is involved and no arithmetic happens on the client.",
    security(("session" = [])),
    responses(
        (status = 200, description = "Overall, per-hero and per-role aggregates over the competitive window", body = StatsResponse),
        (status = 409, description = "No Dota account linked to this user yet", body = crate::error::ErrorBody),
        (status = 401, description = "No session cookie, or it has expired", body = crate::error::ErrorBody),
        (status = 500, description = "Database or internal failure", body = crate::error::ErrorBody),
    )
)]
pub async fn get(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> AppResult<Json<StatsResponse>> {
    let player = load_linked_player(&state, &user).await?;
    let scope = MatchScope::competitive(state.config.roles.analysis_match_limit);

    let overall = repositories::metrics::player_stats_scoped(&state.db, player.id, &scope).await?;
    let heroes =
        repositories::metrics::hero_stats_scoped(&state.db, player.id, &scope, TOP_HEROES).await?;
    let eligibility =
        eligibility::summarize(&repositories::metrics::mode_counts(&state.db, player.id).await?);

    let (scope_info, role_analysis) = competitive_roles(&state, &player).await?;

    Ok(Json(StatsResponse {
        scope: scope_info,
        eligibility,
        overall,
        heroes,
        role_analysis,
        metrics_version: METRICS_VERSION,
    }))
}

/// Role performance over the competitive window, with the scope it describes.
///
/// Shared with the coaching role-selection endpoint so the dashboard and the
/// "which role do you want to improve" screen can never disagree about how a
/// role is performing — there is one computation, read twice.
pub(crate) async fn competitive_roles(
    state: &AppState,
    player: &DotaPlayer,
) -> AppResult<(AnalysisScopeInfo, RoleAnalysis)> {
    let window = state.config.roles.analysis_match_limit;
    let scope = MatchScope::competitive(window);

    let totals = repositories::metrics::role_totals(&state.db, player.id, &scope).await?;
    let analysis = roles::analyze(&totals, state.config.roles.score_weights);

    Ok((scope_info(window, analysis.analyzed_matches), analysis))
}

/// The population description that travels with every competitive response.
pub(crate) fn scope_info(window_limit: i64, analyzed_matches: i64) -> AnalysisScopeInfo {
    let confidence = SampleConfidence::for_matches(analyzed_matches);

    AnalysisScopeInfo {
        population: "ranked_public_all_pick",
        description:
            "Ranked and public All Pick matches only. Turbo and every other mode are excluded.",
        analyzed_matches,
        window_limit,
        confidence,
        confidence_label: confidence.label(),
        confidence_caveat: confidence.caveat(),
    }
}

pub(crate) async fn load_linked_player(state: &AppState, user: &User) -> AppResult<DotaPlayer> {
    repositories::dota_player::find_by_user_id(&state.db, user.id)
        .await?
        .ok_or(AppError::DotaAccountNotLinked)
}

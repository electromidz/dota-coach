//! The AI coach.
//!
//! Reading and generating are separate verbs on purpose. `GET` shows the
//! measured evidence and whatever analysis was last produced, costs nothing,
//! and cannot fail because a provider is down. `POST` is the only thing that
//! spends a model call, and it is the only thing that is rate limited.
//!
//! Generation is synchronous. That is a deliberate limit rather than an
//! oversight: one user-triggered call, bounded by `LLM_TIMEOUT_SECONDS` and by
//! the router's own timeout, needs no queue. Background or scheduled analysis
//! would — and that is where a queue belongs, not here.

use axum::extract::State;
use axum::Json;
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::api::extract::{AppJson, AppPath, CurrentUser, EntitledUser};
use crate::api::handlers::{benchmark, heroes, stats};
use crate::domain::coaching::{AnalysisScope, CoachingAnalysis, Evidence};
use crate::domain::coaching_profile::CoachingProfile;
use crate::domain::coaching_session::CoachingSession;
use crate::domain::event::EventType;
use crate::domain::metrics::{HeroStats, PlayerStats};
use crate::domain::player::DotaPlayer;
use crate::domain::player_model::{PatternStatus, PlayerModel, RecurringPattern};
use crate::domain::r#match::Match;
use crate::domain::role::CoachableRole;
use crate::domain::scope::{MatchScope, SampleConfidence};
use crate::domain::training::{PreliminaryFocus, ProgressSeries, TrainingFocus};
use crate::domain::user::User;
use crate::error::{AppError, AppResult};
use crate::repositories;
use crate::repositories::coaching::NewAnalysis;
use crate::services::cache;
use crate::services::coaching::{
    self,
    evidence::{EvidenceInputs, EvidenceScope},
    CoachingError,
};
use crate::services::coaching_session;
use crate::services::events;
use crate::services::llm::LlmError;
use crate::services::player_model::{self, patterns, ModelInputs};
use crate::services::progress as progress_engine;
use crate::services::training::{self, SelectionInputs};
use crate::state::AppState;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
pub struct CoachResponse {
    /// The last analysis for this scope, or `null` when none has been
    /// generated yet.
    pub analysis: Option<CoachingAnalysis>,
    /// What the coach can currently see. Always present, always measured, and
    /// the only place numbers come from.
    pub evidence: Vec<Evidence>,
    /// False when no model is configured on this deployment.
    pub llm_available: bool,
    /// True when the stored analysis was produced from different evidence than
    /// the evidence above — new matches have landed since it was written.
    pub stale: bool,
    /// Whether this response came from storage rather than a fresh call.
    pub cached: bool,
    /// Recurring patterns currently detected. Deterministic, and present
    /// whether or not a model has ever run.
    ///
    /// Detected inside the same scope as the evidence, so a pattern here is a
    /// statement about the role being coached.
    pub patterns: Vec<RecurringPattern>,
    /// The role everything in this response is about.
    pub role: Option<CoachableRole>,
    pub role_label: Option<&'static str>,
    pub note: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct TrainingFocusResponse {
    /// The one thing to work on, or `null` when nothing clears the bar.
    pub focus: Option<TrainingFocus>,
    /// The weakest thing that *was* measured, when no focus clears the bar.
    ///
    /// Only ever set alongside `focus: null`, and never a substitute for one: it
    /// carries no target and no progress, and its `confidence` is whatever the
    /// benchmark engine assigned — frequently `low`, sometimes `insufficient`,
    /// in which case `percentile` is `null` because none was ever claimed.
    pub preliminary: Option<PreliminaryFocus>,
    /// The focus measure over time, oldest bucket first.
    pub progress: Option<ProgressSeries>,
    /// What would be next, so "why this one" has a comparison.
    pub next_up: Vec<TrainingFocus>,
    /// What the player has worked on before, newest first.
    pub history: Vec<TrainingFocus>,
    pub note: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct PlayerModelResponse {
    pub model: PlayerModel,
    /// Detectors that could not report, with how many matches they could
    /// actually check. Shown rather than hidden: silence from a detector would
    /// otherwise read as a clean bill of health.
    pub unmeasurable: Vec<UnmeasuredDetector>,
    /// The evidence floor every pattern had to clear.
    pub thresholds: PatternThresholds,
}

#[derive(Serialize, ToSchema)]
pub struct UnmeasuredDetector {
    pub id: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    pub measured: i64,
    pub required: i64,
}

#[derive(Serialize, ToSchema)]
pub struct PatternThresholds {
    pub min_measured: i64,
    pub min_occurrences: i64,
    pub min_rate: f32,
}

#[derive(Serialize, ToSchema)]
pub struct RoleSelectionResponse {
    /// The population the role figures were measured over.
    pub scope: crate::api::handlers::stats::AnalysisScopeInfo,
    /// Per-role performance and the advisory pick.
    pub analysis: crate::domain::role::RoleAnalysis,
    /// The player's current choice, or `null` when they have not made one.
    pub profile: Option<CoachingProfile>,
    /// Every role that can be chosen, whether or not it has matches behind it.
    ///
    /// Sent explicitly so the client offers the same five options to a player
    /// with no history as to one with a thousand games: the choice is the
    /// player's, and a role they have never played is a legitimate thing to
    /// want to get better at.
    pub selectable_roles: Vec<SelectableRole>,
}

#[derive(Serialize, ToSchema)]
pub struct SelectableRole {
    pub role: CoachableRole,
    pub label: &'static str,
    pub position: u8,
}

#[derive(Deserialize, ToSchema)]
pub struct SelectRoleRequest {
    /// Role slug — `carry`, `mid`, `offlane`, `soft_support`, `hard_support`.
    pub role: String,
}

/// `GET /api/coach/roles`
///
/// What the "which role do you want to improve?" screen needs: how each role is
/// actually performing, what the system would advise, and what the player has
/// already chosen. Deterministic — no model call.
#[utoipa::path(
    get, path = "/api/coach/roles", tag = "coaching",
    summary = "Role performance, the advisory pick, and the current choice",
    description = "Measured over the latest eligible Ranked and public All Pick matches. The recommendation is advice: any of the five roles may be selected, including one with no matches behind it.",
    security(("session" = [])),
    responses(
        (status = 200, description = "Role performance, recommendation and stored profile", body = RoleSelectionResponse),
        (status = 409, description = "No Dota account linked to this user yet", body = crate::error::ErrorBody),
        (status = 401, description = "No session cookie, or it has expired", body = crate::error::ErrorBody),
        (status = 500, description = "Database or internal failure", body = crate::error::ErrorBody),
    )
)]
pub async fn roles(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> AppResult<Json<RoleSelectionResponse>> {
    let player = load_linked_player(&state, &user).await?;
    let (scope, analysis) = stats::competitive_roles(&state, &player).await?;
    let profile = repositories::coaching_profile::find(&state.db, player.id).await?;

    Ok(Json(RoleSelectionResponse {
        scope,
        analysis,
        profile,
        selectable_roles: selectable_roles(),
    }))
}

/// `POST /api/coach/role`
///
/// The player's choice, which is final. The recommendation that was on offer is
/// stored beside it — not to second-guess the choice, but so the profile can
/// later say what was advised and what was picked.
#[utoipa::path(
    post, path = "/api/coach/role", tag = "coaching",
    summary = "Choose the role to be coached on",
    description = "Any of the five roles is accepted, including one the system did not recommend and one the player has never played. The selection becomes the scope of every subsequent piece of coaching.",
    security(("session" = [])),
    request_body = SelectRoleRequest,
    responses(
        (status = 200, description = "The stored profile and the analysis behind it", body = RoleSelectionResponse),
        (status = 400, description = "Not one of the five coachable roles", body = crate::error::ErrorBody),
        (status = 409, description = "No Dota account linked to this user yet", body = crate::error::ErrorBody),
        (status = 401, description = "No session cookie, or it has expired", body = crate::error::ErrorBody),
        (status = 500, description = "Database or internal failure", body = crate::error::ErrorBody),
    )
)]
pub async fn select_role(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    AppJson(body): AppJson<SelectRoleRequest>,
) -> AppResult<Json<RoleSelectionResponse>> {
    let role = CoachableRole::parse(&body.role).ok_or_else(|| {
        AppError::BadRequest(format!(
            "'{}' is not a role that can be coached. Choose one of: {}.",
            body.role,
            CoachableRole::ALL
                .iter()
                .map(|r| r.slug())
                .collect::<Vec<_>>()
                .join(", "),
        ))
    })?;

    let player = load_linked_player(&state, &user).await?;
    let (scope, analysis) = stats::competitive_roles(&state, &player).await?;

    // Stored as context, never as a constraint: the write below uses `role`,
    // which is what the player asked for.
    let recommended = analysis.recommendation.as_ref().map(|r| r.role);
    let profile = repositories::coaching_profile::upsert(
        &state.db,
        player.id,
        role,
        recommended,
        analysis.analyzed_matches,
    )
    .await?;

    Ok(Json(RoleSelectionResponse {
        scope,
        analysis,
        profile: Some(profile),
        selectable_roles: selectable_roles(),
    }))
}

fn selectable_roles() -> Vec<SelectableRole> {
    CoachableRole::ALL
        .into_iter()
        .map(|role| SelectableRole {
            role,
            label: role.label(),
            position: role.position(),
        })
        .collect()
}

/// The population one player's coaching reads.
///
/// Built once per request and threaded through everything below it, so there is
/// no path where one part of an answer is scoped and another is not.
#[derive(Debug, Clone)]
pub(crate) struct CoachingScope {
    /// The role the player chose. Not the recommended one.
    pub role: CoachableRole,
    /// Eligible matches in that role, newest first, capped at the window.
    pub matches: MatchScope,
}

/// The player's coaching scope, or a refusal.
///
/// Coaching without a chosen role would mean choosing one for them, and the
/// only honest defaults are both wrong: every role mixes evidence the product
/// exists to keep apart, and the recommended role silently overrides a decision
/// that belongs to the player. So this is a precondition, not a fallback.
pub(crate) async fn require_scope(
    state: &AppState,
    player: &DotaPlayer,
) -> AppResult<CoachingScope> {
    let window = state.config.roles.analysis_match_limit;

    let Some(profile) = repositories::coaching_profile::find(&state.db, player.id).await? else {
        // Two different reasons to have no role, and they need different
        // answers: a player with no eligible matches cannot meaningfully
        // choose one yet, and telling them to pick would be a dead end.
        let eligible = repositories::metrics::player_stats_scoped(
            &state.db,
            player.id,
            &MatchScope::competitive(window),
        )
        .await?
        .matches;

        return Err(AppError::PreconditionUnmet(if eligible == 0 {
            "Sync some Ranked or public All Pick matches first — there is nothing to coach on yet."
                .into()
        } else {
            "Choose the role you want to improve first — coaching is scoped to one role.".into()
        }));
    };

    Ok(CoachingScope {
        role: profile.selected_role,
        matches: MatchScope::for_role(profile.selected_role, window),
    })
}

/// `GET /api/coach`
///
/// Never calls the model, never fails on a provider outage.
#[utoipa::path(
    get, path = "/api/coach", tag = "coaching",
    summary = "The stored analysis",
    description = "Reads only. Always answers, including on a deployment with no model configured at all — the deterministic evidence is the part that matters and it is computed in Rust.",
    security(("session" = [])),
    responses(
        (status = 200, description = "Last generated analysis, or evidence alone if none exists", body = CoachResponse),
        (status = 409, description = "No Dota account linked to this user yet", body = crate::error::ErrorBody),
        (status = 401, description = "No session cookie, or it has expired", body = crate::error::ErrorBody),
        (status = 500, description = "Database or internal failure", body = crate::error::ErrorBody),
    )
)]
pub async fn get(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> AppResult<Json<CoachResponse>> {
    let player = load_linked_player(&state, &user).await?;
    let scope = require_scope(&state, &player).await?;
    let (evidence, patterns) = role_evidence(&state, &user, &player, &scope).await?;
    let stored =
        repositories::coaching::latest(&state.db, player.id, None, Some(scope.role)).await?;

    Ok(Json(respond(
        &state,
        stored,
        evidence,
        patterns,
        Some(scope.role),
        None,
        true,
    )))
}

/// `GET /api/coach/player-model`
///
/// The long-term model: what the backend believes about this player and how
/// much that belief is worth. Deterministic throughout — no model call, and
/// nothing here depends on one having ever been made.
#[utoipa::path(
    get, path = "/api/coach/player-model", tag = "coaching",
    summary = "The long-term player model",
    description = "Traits, role affinity and recurring patterns accumulated across matches. Deterministic throughout — reading it never calls a model.",
    security(("session" = [])),
    responses(
        (status = 200, description = "Strengths, weaknesses and recurring patterns", body = PlayerModelResponse),
        (status = 409, description = "No Dota account linked to this user yet", body = crate::error::ErrorBody),
        (status = 401, description = "No session cookie, or it has expired", body = crate::error::ErrorBody),
        (status = 500, description = "Database or internal failure", body = crate::error::ErrorBody),
    )
)]
pub async fn player_model(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> AppResult<Json<PlayerModelResponse>> {
    let player = load_linked_player(&state, &user).await?;

    // Detect and persist first: `first_detected_at` and the resolved set both
    // come back from storage, and both need this run to have happened.
    let window = state.config.roles.analysis_match_limit;
    let refreshed = player_model::refresh(&state.db, player.id, window).await?;
    let stored = repositories::player_model::list_patterns(&state.db, player.id).await?;

    let active = hydrate_patterns(refreshed.patterns, &stored);
    let resolved: Vec<RecurringPattern> = stored
        .into_iter()
        .filter(|p| p.status == PatternStatus::Resolved)
        .collect();

    let benchmark = benchmark::build(
        &state,
        &player,
        None,
        None,
        &MatchScope::competitive(window),
        None,
        // Coaching always compares a player against their own bracket; the
        // selectable one is a question the Benchmark page asks, not this one.
        None,
    )
    .await?;
    let heroes = heroes::build(
        &state,
        &user,
        Some(state.config.heroes.recommendation_limit),
        &MatchScope::career(),
        None,
    )
    .await?;
    // Role affinity describes competitive play: "you mostly play support" is a
    // claim about the ladder, and Turbo games would pad it with a population
    // nothing else in the model reads.
    let roles = repositories::metrics::role_stats_scoped(
        &state.db,
        player.id,
        &MatchScope::competitive(window),
    )
    .await?;

    let model = player_model::build(
        &ModelInputs {
            matches: &refreshed.history,
            benchmarks: &benchmark.results,
            benchmark_hero: (!benchmark.hero_name.is_empty())
                .then_some(benchmark.hero_name.as_str()),
            pool: &heroes.pool,
            roles: &roles,
        },
        active,
        resolved,
        Utc::now(),
    );

    Ok(Json(PlayerModelResponse {
        unmeasurable: unmeasured(&refreshed.history),
        model,
        thresholds: PatternThresholds {
            min_measured: patterns::MIN_MEASURED,
            min_occurrences: patterns::MIN_OCCURRENCES,
            min_rate: patterns::MIN_RATE,
        },
    }))
}

/// Which detectors stayed silent for want of data, and how short they fell.
fn unmeasured(history: &[crate::domain::player_model::AnalyzedMatch]) -> Vec<UnmeasuredDetector> {
    let short = patterns::unmeasurable(history);

    patterns::DETECTORS
        .iter()
        .filter_map(|detector| {
            let measured = *short.get(detector.id)?;
            Some(UnmeasuredDetector {
                id: detector.id,
                label: detector.label,
                description: detector.description,
                measured,
                required: patterns::MIN_MEASURED,
            })
        })
        .collect()
}

/// Attach the stored first-sighting to a freshly detected pattern.
///
/// Detection knows the numbers; storage knows the history. Neither alone can
/// say "you have been doing this since March".
fn hydrate_patterns(
    detected: Vec<RecurringPattern>,
    stored: &[RecurringPattern],
) -> Vec<RecurringPattern> {
    detected
        .into_iter()
        .map(|mut pattern| {
            pattern.first_detected_at = stored
                .iter()
                .find(|s| s.id == pattern.id)
                .and_then(|s| s.first_detected_at);
            pattern
        })
        .collect()
}

/// `GET /api/coach/training-focus`
///
/// One focus, its progress, and the runners-up. Deterministic: no model call,
/// and the selection is reproducible from the same history.
#[utoipa::path(
    get, path = "/api/coach/training-focus", tag = "coaching",
    summary = "The current training focus",
    description = "One focus at a time by design. Chosen from benchmark gap, historical pattern, impact and confidence — not simply the lowest statistic.",
    security(("session" = [])),
    responses(
        (status = 200, description = "The active focus and progress against it", body = TrainingFocusResponse),
        (status = 409, description = "No Dota account linked to this user yet", body = crate::error::ErrorBody),
        (status = 401, description = "No session cookie, or it has expired", body = crate::error::ErrorBody),
        (status = 500, description = "Database or internal failure", body = crate::error::ErrorBody),
    )
)]
pub async fn training_focus(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> AppResult<Json<TrainingFocusResponse>> {
    let player = load_linked_player(&state, &user).await?;
    let scope = require_scope(&state, &player).await?;

    // Role-scoped throughout: the focus is "the one thing to work on" in the
    // role being coached, and a goal derived from another role's matches would
    // be advice about a game the player is not currently playing.
    let refreshed = player_model::analyze_scope(&state.db, player.id, &scope.matches).await?;
    let benchmark = role_benchmark(&state, &player, &scope).await?;

    let inputs = SelectionInputs {
        history: &refreshed.history,
        benchmarks: &benchmark.results,
        patterns: &refreshed.patterns,
        now: Utc::now(),
    };

    let focus = training::ensure(
        &state.db,
        player.id,
        Some(scope.role),
        &inputs,
        state.config.training.focus_weights,
    )
    .await?;

    let progress = focus.as_ref().map(|f| {
        training::series(
            f.measure,
            f.pattern_id.as_deref(),
            &refreshed.history,
            Some(f.target_value),
        )
    });

    // The runners-up, minus whatever is currently being worked on.
    let next_up: Vec<TrainingFocus> = training::rank(&inputs, state.config.training.focus_weights)
        .into_iter()
        .filter(|candidate| focus.as_ref().is_none_or(|f| f.key != candidate.key))
        .take(3)
        .collect();

    let history = repositories::training::history(
        &state.db,
        player.id,
        Some(scope.role),
        state.config.training.history_limit,
    )
    .await?;

    // Only consulted when nothing cleared the bar, and it changes no part of
    // the decision above — it reports what the benchmark engine already
    // measured so that "keep playing" is not the whole answer.
    let preliminary = focus
        .is_none()
        .then(|| training::preliminary(&benchmark.results))
        .flatten();

    Ok(Json(TrainingFocusResponse {
        // Suppressed when there is a preliminary reading: the card says its own
        // caveat, and a second "nothing stands out" beside it would contradict
        // the thing it sits next to.
        note: (focus.is_none() && preliminary.is_none()).then(|| {
            if refreshed.history.is_empty() {
                format!(
                    "No eligible {} matches yet — there is nothing to train on in this role.",
                    scope.role.label(),
                )
            } else {
                "Nothing stands out as a training focus right now. Keep playing and check back."
                    .to_string()
            }
        }),
        focus,
        preliminary,
        progress,
        next_up,
        history,
    }))
}

/// `POST /api/coach/analyze`
///
/// Premium: generating is the one thing in this API that costs money per call,
/// so it is the one thing the trial gates. Everything measured — stats,
/// benchmarks, patterns, the stored analysis — keeps answering after the trial
/// ends.
#[utoipa::path(
    post, path = "/api/coach/analyze", tag = "coaching",
    summary = "Generate a new analysis",
    description = "The only route that calls a model, and the only rate-limited verb in the API. Slow by nature: a reasoning model spends most of a call thinking, so this endpoint carries a longer timeout than the rest of the API. Every insight the model returns is verified against the evidence it was given and discarded if it cites anything that does not exist.",
    security(("session" = [])),
    responses(
        (status = 200, description = "Freshly generated, validated analysis", body = CoachResponse),
        (status = 429, description = "Inside the cooldown, or over the daily limit", body = crate::error::ErrorBody),
        (status = 502, description = "The model failed, timed out, or returned an unusable answer", body = crate::error::ErrorBody),
        (status = 503, description = "No coaching model is configured on this server", body = crate::error::ErrorBody),
        (status = 409, description = "No Dota account linked to this user yet", body = crate::error::ErrorBody),
        (status = 401, description = "No session cookie, or it has expired", body = crate::error::ErrorBody),
        (status = 500, description = "Database or internal failure", body = crate::error::ErrorBody),
    )
)]
pub async fn analyze(
    State(state): State<AppState>,
    EntitledUser(user): EntitledUser,
) -> AppResult<Json<CoachResponse>> {
    let player = load_linked_player(&state, &user).await?;
    let scope = require_scope(&state, &player).await?;

    // One gather, two renderings: the prose the model is shown, and the
    // numbers the session records. Fetching twice would let the snapshot and
    // the advice written about it describe different windows.
    let inputs = role_inputs(&state, &user, &player, &scope).await?;
    let patterns = inputs.patterns.clone();

    // A threshold of one, not ten: an analysis has to bind to a snapshot
    // describing exactly the games it read, so any new match earns a session
    // before the model sees anything. A failure here must not cost the player
    // the analysis they are paying for.
    let session = match checkpoint_from(&state, &player, &scope, &inputs, 1).await {
        Ok(session) => session,
        Err(e) => {
            tracing::warn!(error = %e, "coaching session skipped before generation");
            None
        }
    };
    // With nothing new, the newest existing session is the one this analysis
    // describes.
    let session_id = match session {
        Some(session) => Some(session.id),
        None => repositories::coaching_session::latest(&state.db, player.id, scope.role)
            .await?
            .map(|s| s.id),
    };

    // Read *after* the checkpoint, so a session written a moment ago is the
    // current half of the comparison rather than being compared against.
    let progress = progress_context(&state, &player, scope.role).await?;
    let evidence = inputs.evidence(
        &scope,
        state.config.coach.recent_matches as usize,
        &progress,
    );

    run(
        &state,
        &player,
        Generation {
            scope: AnalysisScope::Role,
            role: Some(scope.role),
            match_id: None,
            evidence,
            patterns,
            session_id,
        },
    )
    .await
}

/// `POST /api/matches/:id/analyze`
///
/// Premium, for the same reason as `analyze`: it spends a model call.
#[utoipa::path(
    post, path = "/api/matches/{id}/analyze", tag = "coaching",
    summary = "Generate an analysis for one match",
    description = "Calls a model, and is rate limited and timed out on the same terms as `/api/coach/analyze`.",
    security(("session" = [])),
    params(
        ("id" = Uuid, Path,
            description = "Internal match id — the `id` from `/api/matches`, not the Dota match id.",
            example = "3fa85f64-5717-4562-b3fc-2c963f66afa6"),
    ),
    responses(
        (status = 200, description = "Freshly generated, validated analysis", body = CoachResponse),
        (status = 404, description = "No such match, or it belongs to another player", body = crate::error::ErrorBody),
        (status = 429, description = "Inside the cooldown, or over the daily limit", body = crate::error::ErrorBody),
        (status = 502, description = "The model failed, timed out, or returned an unusable answer", body = crate::error::ErrorBody),
        (status = 503, description = "No coaching model is configured on this server", body = crate::error::ErrorBody),
        (status = 409, description = "No Dota account linked to this user yet", body = crate::error::ErrorBody),
        (status = 401, description = "No session cookie, or it has expired", body = crate::error::ErrorBody),
        (status = 500, description = "Database or internal failure", body = crate::error::ErrorBody),
    )
)]
pub async fn analyze_match(
    State(state): State<AppState>,
    EntitledUser(user): EntitledUser,
    AppPath(id): AppPath<Uuid>,
) -> AppResult<Json<CoachResponse>> {
    let player = load_linked_player(&state, &user).await?;
    let match_ = load_owned_match(&state, &player, id).await?;
    let (evidence, patterns) = match_evidence(&state, &user, &player, &match_).await?;

    run(
        &state,
        &player,
        Generation {
            scope: AnalysisScope::Match,
            role: CoachableRole::from_stored(&match_.role),
            match_id: Some(id),
            evidence,
            patterns,
            // A single game is not a coaching session, so nothing to bind.
            session_id: None,
        },
    )
    .await
}

/// `GET /api/matches/:id/analysis`
///
/// The stored analysis for one match, if there is one. Separate from the POST
/// so opening a match page never spends a model call.
#[utoipa::path(
    get, path = "/api/matches/{id}/analysis", tag = "coaching",
    summary = "Stored analysis for one match",
    security(("session" = [])),
    params(
        ("id" = Uuid, Path,
            description = "Internal match id — the `id` from `/api/matches`, not the Dota match id.",
            example = "3fa85f64-5717-4562-b3fc-2c963f66afa6"),
    ),
    responses(
        (status = 200, description = "Stored analysis, or evidence alone if none exists", body = CoachResponse),
        (status = 404, description = "No such match, or it belongs to another player", body = crate::error::ErrorBody),
        (status = 409, description = "No Dota account linked to this user yet", body = crate::error::ErrorBody),
        (status = 401, description = "No session cookie, or it has expired", body = crate::error::ErrorBody),
        (status = 500, description = "Database or internal failure", body = crate::error::ErrorBody),
    )
)]
pub async fn match_analysis(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    AppPath(id): AppPath<Uuid>,
) -> AppResult<Json<CoachResponse>> {
    let player = load_linked_player(&state, &user).await?;
    let match_ = load_owned_match(&state, &player, id).await?;
    let role = CoachableRole::from_stored(&match_.role);

    let (evidence, patterns) = match_evidence(&state, &user, &player, &match_).await?;
    let stored = repositories::coaching::latest(&state.db, player.id, Some(id), role).await?;

    Ok(Json(respond(
        &state, stored, evidence, patterns, role, None, true,
    )))
}

/// One question to put to the model, and where its answer belongs.
struct Generation {
    scope: AnalysisScope,
    role: Option<CoachableRole>,
    match_id: Option<Uuid>,
    evidence: Vec<Evidence>,
    patterns: Vec<RecurringPattern>,
    /// The session this analysis interprets, when there is one. A match
    /// analysis has none: a single game is not a coaching session.
    session_id: Option<Uuid>,
}

/// Generate, verify and store — or hand back the answer to the same question.
async fn run(
    state: &AppState,
    player: &DotaPlayer,
    request: Generation,
) -> AppResult<Json<CoachResponse>> {
    let Generation {
        scope,
        role,
        match_id,
        evidence,
        patterns,
        session_id,
    } = request;

    if evidence.is_empty() {
        return Err(AppError::PreconditionUnmet(
            "There is nothing to analyse yet — sync some matches first.".into(),
        ));
    }
    if !state.llm.is_configured() {
        return Err(AppError::FeatureUnavailable(
            LlmError::NotConfigured.user_note().to_string(),
        ));
    }

    // Recorded once the preconditions pass, whether this call ends up
    // generating or serving a cached answer — either way the player invoked
    // the paid feature.
    let scope_slug = match scope {
        AnalysisScope::Player => "player",
        AnalysisScope::Match => "match_analysis",
        AnalysisScope::Role => "role_analysis",
    };
    events::track(
        &state.db,
        player.user_id,
        EventType::FeatureUsed,
        serde_json::json!({ "scope": scope_slug }),
    )
    .await;

    // Keyed on the configured model rather than the one that answers: it is
    // what the *next* request would use, and a served-model change behind the
    // same configuration is not a different question.
    // The role is part of the question's identity, not an attribute of the
    // answer. Without it a Carry analysis and a Support one could share a cache
    // key, and the player would be served advice about a role they did not ask
    // about — silently, and looking exactly like a fresh answer.
    let hash = coaching::context_hash(&evidence, scope, role, &state.config.llm.model);

    if let Some(cached) =
        repositories::coaching::find_by_hash(&state.db, player.id, match_id, &hash).await?
    {
        // Same evidence, same answer. Costs nothing and spends no budget.
        return Ok(Json(respond(
            state,
            Some(cached),
            evidence,
            patterns,
            role,
            None,
            true,
        )));
    }

    enforce_limits(state, player).await?;

    let generated = coaching::generate(state.llm.as_ref(), scope, &evidence, &state.config.coach)
        .await
        .map_err(coaching_error)?;

    let analysis_id = repositories::coaching::insert(
        &state.db,
        &NewAnalysis {
            dota_player_id: player.id,
            match_id,
            scope,
            role,
            context_hash: &hash,
            model: &generated.model,
            summary: &generated.summary,
            evidence: &evidence,
            insights: &generated.insights,
            plan: &generated.plan,
        },
    )
    .await?;

    // Bind the model's reading to the snapshot it read. A session records the
    // *first* interpretation made of it: re-generating after a prompt-version
    // bump writes a new analysis row and leaves the session alone, which is
    // what immutability means here. `attach_analysis` answering false is that
    // case, not a failure.
    if let Some(session_id) = session_id {
        match repositories::coaching_session::attach_analysis(
            &state.db,
            session_id,
            player.id,
            analysis_id,
        )
        .await
        {
            Ok(true) => tracing::info!(session = %session_id, "analysis attached to session"),
            Ok(false) => {}
            // The analysis is stored and is what the caller asked for; losing
            // the link is not worth failing the request over.
            Err(e) => tracing::warn!(error = %e, "could not attach the analysis to its session"),
        }
    }

    let stored =
        repositories::coaching::find_by_hash(&state.db, player.id, match_id, &hash).await?;
    Ok(Json(respond(
        state, stored, evidence, patterns, role, None, false,
    )))
}

fn respond(
    state: &AppState,
    analysis: Option<CoachingAnalysis>,
    evidence: Vec<Evidence>,
    patterns: Vec<RecurringPattern>,
    role: Option<CoachableRole>,
    note: Option<String>,
    cached: bool,
) -> CoachResponse {
    let llm_available = state.llm.is_configured();

    // "Stale" compares the evidence behind the stored answer with the evidence
    // as it is now, so a player who has synced since their last analysis is
    // told the advice predates their recent games.
    let stale = analysis.as_ref().is_some_and(|a| {
        let stored: Vec<&str> = a.evidence.iter().map(|e| e.statement.as_str()).collect();
        let current: Vec<&str> = evidence.iter().map(|e| e.statement.as_str()).collect();
        stored != current
    });

    let note =
        note.or_else(|| (!llm_available).then(|| LlmError::NotConfigured.user_note().to_string()));

    CoachResponse {
        analysis,
        evidence,
        llm_available,
        stale,
        cached,
        patterns,
        role,
        role_label: role.map(CoachableRole::label),
        note,
    }
}

/// Per-user budget for the one operation that costs money per request.
async fn enforce_limits(state: &AppState, player: &DotaPlayer) -> AppResult<()> {
    let config = &state.config.coach;

    if config.cooldown_seconds > 0 {
        if let Some(last) = repositories::coaching::last_generated_at(&state.db, player.id).await? {
            let wait = Duration::seconds(config.cooldown_seconds) - (Utc::now() - last);
            if wait > Duration::zero() {
                return Err(AppError::TooManyRequests(format!(
                    "Give the coach a moment — try again in {} seconds.",
                    wait.num_seconds().max(1),
                )));
            }
        }
    }

    if config.daily_limit > 0 {
        let since = Utc::now() - Duration::hours(24);
        let used = repositories::coaching::count_since(&state.db, player.id, since).await?;
        if used >= config.daily_limit {
            return Err(AppError::TooManyRequests(format!(
                "You have used all {} analyses for today. They reset on a rolling 24-hour window.",
                config.daily_limit,
            )));
        }
    }

    Ok(())
}

/// The coaching evidence set, scoped to one role.
///
/// This function is the architectural boundary the whole phase rests on. Every
/// figure in the returned evidence comes from a query that was handed
/// `scope.matches`, so a Support match cannot reach a Carry analysis by any
/// path — not because the prompt asks the model to ignore it, but because it
/// was never fetched.
pub(crate) async fn role_evidence(
    state: &AppState,
    user: &User,
    player: &DotaPlayer,
    scope: &CoachingScope,
) -> AppResult<(Vec<Evidence>, Vec<RecurringPattern>)> {
    let key = evidence_cache_key(state, player, scope).await;

    let compute = || async {
        let inputs = role_inputs(state, user, player, scope).await?;
        let progress = progress_context(state, player, scope.role).await?;
        let evidence =
            inputs.evidence(scope, state.config.coach.recent_matches as usize, &progress);
        Ok(CachedEvidence {
            evidence,
            patterns: inputs.patterns,
        })
    };

    let cached = match key {
        Some(key) => cache::read_through(state.cache.as_ref(), &key, player.id, compute).await?,
        // No fingerprint means something the key depends on could not be read.
        // Computing without caching is the safe half of that: a value nobody
        // can describe is a value nobody should store.
        None => compute().await?,
    };

    Ok((cached.evidence, cached.patterns))
}

/// The cached half of a coaching read.
///
/// Both halves travel together because they are computed together and are
/// consistent only with each other: patterns detected in one window and
/// evidence built from another would let the page cite a habit its own
/// figures do not show.
#[derive(Serialize, Deserialize)]
struct CachedEvidence {
    evidence: Vec<Evidence>,
    patterns: Vec<RecurringPattern>,
}

/// A key that stops matching the moment anything behind the value moves.
///
/// This is the invalidation mechanism, and it is deliberately not eviction.
/// Forgetting to evict is how a coaching cache starts telling a player about
/// last week; a key that cannot match cannot be forgotten about.
///
/// The fingerprint covers every input the cached evidence depends on:
///
/// | Input | Why it is in the key |
/// |---|---|
/// | `last_synced_at` | New matches, and the metrics recomputed with them |
/// | `metrics_version` | A formula change makes every stored figure different |
/// | coaching profile `updated_at` | The role, and the window it implies |
/// | active focus id and `updated_at` | `focus.current` evidence |
/// | newest session id | The progress comparison |
/// | benchmark and hero-meta snapshot freshness | Peer and meta evidence |
///
/// `None` when the fingerprint itself could not be read, which is treated as
/// "do not cache" rather than "cache under a guess".
async fn evidence_cache_key(
    state: &AppState,
    player: &DotaPlayer,
    scope: &CoachingScope,
) -> Option<cache::CacheKey> {
    if !state.cache.is_enabled() {
        return None;
    }

    let fingerprint: String = sqlx::query_scalar::<_, Option<String>>(
        "SELECT concat_ws(
                    '|',
                    to_char(p.last_synced_at, 'YYYYMMDDHH24MISSUS'),
                    $2::text,
                    to_char(cp.updated_at, 'YYYYMMDDHH24MISSUS'),
                    tf.id::text,
                    to_char(tf.updated_at, 'YYYYMMDDHH24MISSUS'),
                    cs.id::text,
                    to_char(bs.newest, 'YYYYMMDDHH24MISSUS'),
                    to_char(hms.newest, 'YYYYMMDDHH24MISSUS')
                )
           FROM dota_players p
           LEFT JOIN coaching_profiles cp ON cp.dota_player_id = p.id
           LEFT JOIN training_focus tf
                  ON tf.dota_player_id = p.id
                 AND tf.status = 'active'
                 AND tf.role IS NOT DISTINCT FROM $3
           LEFT JOIN LATERAL (
                    SELECT id FROM coaching_sessions
                     WHERE dota_player_id = p.id AND role = $3
                     ORDER BY sequence DESC LIMIT 1
                ) cs ON TRUE
           LEFT JOIN LATERAL (
                    SELECT MAX(fetched_at) AS newest FROM benchmark_snapshots
                ) bs ON TRUE
           LEFT JOIN LATERAL (
                    SELECT MAX(fetched_at) AS newest FROM hero_meta_snapshots
                ) hms ON TRUE
          WHERE p.id = $1",
    )
    .bind(player.id)
    .bind(crate::services::metrics::METRICS_VERSION)
    .bind(scope.role.slug())
    .fetch_optional(&state.db)
    .await
    .unwrap_or_else(|e| {
        tracing::warn!(error = %e, "coaching cache fingerprint failed; computing uncached");
        None
    })
    // `fetch_optional` wraps the column's own nullability, so flatten before
    // using it: no row and a null fingerprint both mean "do not cache".
    .flatten()?;

    Some(cache::CacheKey::new(
        "coach-evidence",
        player.id,
        scope.role.slug(),
        &short_hash(&fingerprint),
    ))
}

/// A fingerprint short enough to read in a log line.
///
/// Hashed rather than concatenated because the raw string carries timestamps
/// and ids that have no business being a primary key, and because a key of
/// bounded length keeps the index small.
fn short_hash(input: &str) -> String {
    use sha2::{Digest, Sha256};

    let digest = Sha256::digest(input.as_bytes());
    digest[..16].iter().fold(String::new(), |mut acc, byte| {
        use std::fmt::Write;
        let _ = write!(acc, "{byte:02x}");
        acc
    })
}

/// Everything one role's coaching is computed from, fetched once.
///
/// Two things are rendered from this: *sentences* for a model to cite, and
/// *numbers* for a coaching session to store and a later session to be compared
/// against. They are the same facts in two shapes — prose cannot be compared,
/// and numbers cannot be read aloud — so fetching twice would mean a snapshot
/// and the advice written about it could disagree.
pub(crate) struct RoleInputs {
    pub stats: PlayerStats,
    /// The scoped match history the pattern detectors read.
    pub history: Vec<crate::domain::player_model::AnalyzedMatch>,
    pub patterns: Vec<RecurringPattern>,
    /// The whole scoped window, newest first — the exact matches every figure
    /// here was read from, and the set a session records.
    ///
    /// Fetched once and sliced for the recent-form evidence, rather than
    /// queried twice at two limits: two reads of a moving window can disagree.
    pub window: Vec<Match>,
    pub benchmark: benchmark::BenchmarkResponse,
    /// True when the benchmark provider answered at all. A provider that is
    /// *down* is different from one that has no data for this hero, and only
    /// the first should stop a session being written.
    pub benchmark_available: bool,
    pub heroes: heroes::Built,
    /// The role's own rollup, used for the stored performance score.
    pub hero_stats: Vec<HeroStats>,
    pub focus: Option<TrainingFocus>,
}

impl RoleInputs {
    /// The ids of every match behind these figures, newest first.
    pub fn window_ids(&self) -> Vec<Uuid> {
        self.window.iter().map(|m| m.id).collect()
    }

    /// The prose rendering, for the model.
    ///
    /// `progress` is passed in rather than gathered here because its value
    /// depends on *when* it is read: a generation checkpoints first, so the
    /// session it just wrote is part of the comparison the model is shown.
    fn evidence(
        &self,
        scope: &CoachingScope,
        recent: usize,
        progress: &ProgressContext,
    ) -> Vec<Evidence> {
        let recent = &self.window[..recent.min(self.window.len())];

        coaching::evidence::build(&EvidenceInputs {
            scope: EvidenceScope {
                role: Some(scope.role),
                matches: self.stats.matches,
                confidence: SampleConfidence::for_matches(self.stats.matches),
            },
            stats: &self.stats,
            recent,
            benchmarks: &self.benchmark.results,
            benchmark_hero: (!self.benchmark.hero_name.is_empty())
                .then_some(self.benchmark.hero_name.as_str()),
            pool: &self.heroes.pool,
            recommendations: &self.heroes.recommendations,
            patterns: &self.patterns,
            progress: progress.comparison.as_ref(),
            has_session: progress.has_session,
            focus: self.focus.as_ref(),
            focus_match: None,
            focus_metrics: None,
        })
    }
}

/// Fetch everything one role's coaching reads.
///
/// This is the architectural boundary: every query below is handed
/// `scope.matches`, so a Support match cannot reach a Carry analysis by any
/// path — not because the prompt asks the model to ignore it, but because it
/// was never fetched.
pub(crate) async fn role_inputs(
    state: &AppState,
    user: &User,
    player: &DotaPlayer,
    scope: &CoachingScope,
) -> AppResult<RoleInputs> {
    let stats =
        repositories::metrics::player_stats_scoped(&state.db, player.id, &scope.matches).await?;

    // Patterns are what let an insight be about a habit rather than an
    // average, so they are detected before the evidence is assembled — and
    // detected inside the scope, so "you keep doing this" means "in this role".
    let refreshed = player_model::analyze_scope(&state.db, player.id, &scope.matches).await?;
    let patterns = refreshed.patterns;

    // The analysis window itself: no display filter, newest first, exactly the
    // games the scope defines.
    let window = repositories::r#match::list_by_player_scoped(
        &state.db,
        player.id,
        &scope.matches,
        &repositories::r#match::MatchFilter::default(),
        state.config.roles.analysis_match_limit,
        0,
    )
    .await?;

    // Both of these degrade internally rather than failing: a benchmark or
    // meta outage removes evidence, it does not remove the coach.
    let benchmark = role_benchmark(state, player, scope).await?;
    // `build` reports an outage as a note rather than an error, so that note is
    // the only signal a caller has that the peer half is missing for a reason
    // other than "no data".
    let benchmark_available = benchmark.note.is_none();

    let heroes = heroes::build(
        state,
        user,
        Some(state.config.heroes.recommendation_limit),
        &scope.matches,
        Some(scope.role),
    )
    .await?;

    // The role's most-played heroes, as plain rollups. Cheaper than
    // `heroes::build` and the shape a session stores.
    let hero_stats = repositories::metrics::hero_stats_scoped(
        &state.db,
        player.id,
        &scope.matches,
        HERO_SNAPSHOT_LIMIT,
    )
    .await?;

    let focus = active_focus(
        state,
        player,
        Some(scope.role),
        &refreshed.history,
        &benchmark.results,
        &patterns,
    )
    .await?;

    Ok(RoleInputs {
        stats,
        history: refreshed.history,
        patterns,
        window,
        benchmark,
        benchmark_available,
        heroes,
        hero_stats,
        focus,
    })
}

/// How many of the role's heroes a session records.
const HERO_SNAPSHOT_LIMIT: i64 = 5;

/// What the coach knows about this player's history with the role.
///
/// Carries the absence as well as the comparison, because they are different
/// absences: a player with no sessions and a player with exactly one both have
/// no comparison, and only the second has been measured before.
pub(crate) struct ProgressContext {
    pub comparison: Option<crate::domain::progress::SessionProgress>,
    pub has_session: bool,
}

/// Compare the two newest sessions for a role, if there are two.
///
/// Read at the point of use rather than cached on [`RoleInputs`]: a generation
/// checkpoints *before* building its evidence, so the session it just wrote
/// has to be part of the comparison the model is shown.
pub(crate) async fn progress_context(
    state: &AppState,
    player: &DotaPlayer,
    role: CoachableRole,
) -> AppResult<ProgressContext> {
    // Two is all a comparison needs. The trend across ten is a different
    // question, answered by `/api/coach/progress`.
    let sessions = repositories::coaching_session::list(&state.db, player.id, role, 2, 0).await?;

    Ok(ProgressContext {
        has_session: !sessions.is_empty(),
        comparison: match sessions.as_slice() {
            [current, previous] => Some(progress_engine::compare(previous, current)),
            _ => None,
        },
    })
}

/// The coaching scope, or `None` when the player has not chosen a role.
///
/// The quiet counterpart to [`require_scope`], for callers that are not serving
/// a coaching request and must not turn "no role yet" into an error. A sync is
/// the case that matters: a player who has never opened the coach still syncs
/// matches, and that must keep working.
pub(crate) async fn scope_for_checkpoint(
    state: &AppState,
    player: &DotaPlayer,
) -> AppResult<Option<CoachingScope>> {
    let Some(profile) = repositories::coaching_profile::find(&state.db, player.id).await? else {
        return Ok(None);
    };

    Ok(Some(CoachingScope {
        role: profile.selected_role,
        matches: MatchScope::for_role(
            profile.selected_role,
            state.config.roles.analysis_match_limit,
        ),
    }))
}

/// Record where the player stands, if enough has changed to be worth recording.
///
/// `min_new` is the number of matches the last session did not read that
/// justifies a new one. Two callers want different answers:
///
///   * A **sync** passes [`MIN_NEW_MATCHES`] — background checkpointing, where
///     the threshold is what stops history filling with near-identical
///     snapshots that make every later comparison read as noise.
///   * A **generation** passes 1, because an analysis must bind to a snapshot
///     describing exactly the games it read.
///
/// `Ok(None)` means nothing was new enough to record. That is the common case
/// and is not an error.
///
/// A session is only written when it can be written **completely**. If the
/// benchmark provider is down, this writes nothing and lets the next
/// checkpoint pick it up: a snapshot permanently missing its peer half would
/// poison every comparison made against it, and unlike a page, a stored
/// session cannot be re-rendered later.
pub(crate) async fn checkpoint(
    state: &AppState,
    user: &User,
    player: &DotaPlayer,
    scope: &CoachingScope,
    min_new: usize,
) -> AppResult<Option<CoachingSession>> {
    let inputs = role_inputs(state, user, player, scope).await?;
    checkpoint_from(state, player, scope, &inputs, min_new).await
}

/// The same, from a gather the caller already did.
///
/// Generation needs both renderings of one fetch — the prose for the model and
/// the numbers for the session — and fetching twice would let the snapshot and
/// the advice written about it describe different windows.
pub(crate) async fn checkpoint_from(
    state: &AppState,
    player: &DotaPlayer,
    scope: &CoachingScope,
    inputs: &RoleInputs,
    min_new: usize,
) -> AppResult<Option<CoachingSession>> {
    let latest = repositories::coaching_session::latest(&state.db, player.id, scope.role).await?;

    if !due(latest.as_ref(), &inputs.window_ids(), min_new) {
        return Ok(None);
    }

    if !inputs.benchmark_available {
        tracing::warn!(
            role = scope.role.slug(),
            "coaching session skipped: the benchmark provider is unavailable"
        );
        return Ok(None);
    }

    // The role's own row out of the ladder-wide analysis. `None` when the role
    // has too few matches to score, which is a session with no performance
    // rather than no session.
    let (_, analysis) = stats::competitive_roles(state, player).await?;
    let role_performance = analysis.roles.iter().find(|r| r.role == scope.role);

    let model = player_model::build(
        &ModelInputs {
            matches: &inputs.history,
            benchmarks: &inputs.benchmark.results,
            benchmark_hero: (!inputs.benchmark.hero_name.is_empty())
                .then_some(inputs.benchmark.hero_name.as_str()),
            pool: &inputs.heroes.pool,
            roles: &[],
        },
        inputs.patterns.clone(),
        // Resolved patterns come from storage and are player-wide, not
        // role-scoped. A role snapshot claims nothing about them.
        Vec::new(),
        Utc::now(),
    );

    let draft = coaching_session::build(&coaching_session::SessionInputs {
        role: scope.role,
        matches: &inputs.window,
        role_performance,
        stats: &inputs.stats,
        model: &model,
        benchmarks: &inputs.benchmark.results,
        heroes: &inputs.hero_stats,
        focus: inputs.focus.as_ref(),
    });

    let stored = repositories::coaching_session::insert(&state.db, player.id, &draft).await?;

    tracing::info!(
        session = %stored.id,
        sequence = stored.sequence,
        role = scope.role.slug(),
        matches = stored.analyzed_match_count,
        "coaching session recorded"
    );

    Ok(Some(stored))
}

/// Whether the window has moved enough since the last session.
///
/// The first session is due as soon as there is anything to measure.
fn due(latest: Option<&CoachingSession>, window_ids: &[Uuid], min_new: usize) -> bool {
    match latest {
        None => !window_ids.is_empty(),
        Some(latest) => coaching_session::new_match_count(latest, window_ids) >= min_new,
    }
}

/// The peer comparison for the role being coached.
///
/// The hero is picked from the role's own most-played, not the player's: a
/// Phantom Assassin benchmark is not a fact about their hard support games.
/// The comparison itself is still hero-segmented — that is all the provider
/// offers, and it says so on every result.
async fn role_benchmark(
    state: &AppState,
    player: &DotaPlayer,
    scope: &CoachingScope,
) -> AppResult<benchmark::BenchmarkResponse> {
    let top = repositories::metrics::hero_stats_scoped(&state.db, player.id, &scope.matches, 1)
        .await?
        .into_iter()
        .next()
        .map(|hero| hero.hero_id);

    // The scope is passed either way: the player's own averages have to come
    // from the role's matches, or the percentile would describe a figure the
    // coaching set does not contain.
    benchmark::build(
        state,
        player,
        top,
        None,
        &scope.matches,
        Some(scope.role),
        None,
    )
    .await
}

/// One match, read against the same career evidence.
///
/// The career half is what makes a single game coachable: "six deaths" is a
/// fact, "six deaths against your average of 3.2" is a lesson.
async fn match_evidence(
    state: &AppState,
    user: &User,
    player: &DotaPlayer,
    match_: &Match,
) -> AppResult<(Vec<Evidence>, Vec<RecurringPattern>)> {
    // A match is read against the player's record **in the role that match was
    // played in**, not against their chosen coaching role and not against their
    // career. "Six deaths against your average" is only a lesson if the average
    // is from games like this one.
    //
    // A match whose role the estimator could not attribute falls back to the
    // whole competitive window, which is the honest comparison available.
    let role = CoachableRole::from_stored(&match_.role);
    let window = state.config.roles.analysis_match_limit;
    let match_scope = match role {
        Some(role) => MatchScope::for_role(role, window),
        None => MatchScope::competitive(window),
    };

    let stats =
        repositories::metrics::player_stats_scoped(&state.db, player.id, &match_scope).await?;
    let refreshed = player_model::analyze_scope(&state.db, player.id, &match_scope).await?;
    let detected = refreshed.patterns;
    let metrics = repositories::metrics::for_match(&state.db, match_.id).await?;
    let benchmark = benchmark::build(
        state,
        player,
        Some(match_.hero_id),
        None,
        &match_scope,
        role,
        None,
    )
    .await?;
    let heroes = heroes::build(
        state,
        user,
        Some(state.config.heroes.recommendation_limit),
        &match_scope,
        role,
    )
    .await?;
    let focus = active_focus(
        state,
        player,
        role,
        &refreshed.history,
        &benchmark.results,
        &detected,
    )
    .await?;

    let evidence = coaching::evidence::build(&EvidenceInputs {
        scope: EvidenceScope {
            role,
            matches: stats.matches,
            confidence: SampleConfidence::for_matches(stats.matches),
        },
        stats: &stats,
        recent: &[],
        benchmarks: &benchmark.results,
        benchmark_hero: (!benchmark.hero_name.is_empty()).then_some(benchmark.hero_name.as_str()),
        pool: &heroes.pool,
        recommendations: &[],
        // A single match read against the player's habits, not just their
        // averages: "you did it again" is the coachable observation.
        patterns: &detected,
        // A single match is not a coaching session, so it is never described
        // as progress against one.
        progress: None,
        has_session: false,
        focus: focus.as_ref(),
        focus_match: Some(match_),
        focus_metrics: metrics.as_ref(),
    });

    Ok((evidence, detected))
}

/// Translate a coaching failure into the HTTP answer it deserves.
///
/// A failed generation is an error rather than a quiet empty response: the
/// caller asked for something specific and did not get it. Reading coaching
/// still works throughout.
fn coaching_error(error: CoachingError) -> AppError {
    match error {
        CoachingError::NoEvidence => AppError::PreconditionUnmet(error.user_note()),
        CoachingError::Unusable(ref detail) => {
            tracing::warn!(detail, "discarded an unverifiable coaching answer");
            AppError::Upstream("the coaching model".into())
        }
        CoachingError::Llm(LlmError::NotConfigured) => {
            AppError::FeatureUnavailable(LlmError::NotConfigured.user_note().to_string())
        }
        CoachingError::Llm(LlmError::RateLimited) => {
            AppError::TooManyRequests(LlmError::RateLimited.user_note().to_string())
        }
        CoachingError::Llm(e) => {
            tracing::warn!(error = %e, "coaching model unavailable");
            AppError::Upstream("the coaching model".into())
        }
    }
}

/// The active focus with its live figures, read-only.
///
/// Deliberately does not *select* one: reading the coach must not have the
/// side effect of committing a player to a training goal. Selection happens on
/// `GET /api/coach/training-focus`, which is the endpoint that means it.
async fn active_focus(
    state: &AppState,
    player: &DotaPlayer,
    role: Option<CoachableRole>,
    history: &[crate::domain::player_model::AnalyzedMatch],
    benchmarks: &[crate::domain::benchmark::BenchmarkResult],
    patterns: &[RecurringPattern],
) -> AppResult<Option<TrainingFocus>> {
    let Some(stored) = repositories::training::active(&state.db, player.id, role).await? else {
        return Ok(None);
    };

    Ok(Some(training::hydrate(
        stored,
        &SelectionInputs {
            history,
            benchmarks,
            patterns,
            now: Utc::now(),
        },
    )))
}

async fn load_owned_match(state: &AppState, player: &DotaPlayer, id: Uuid) -> AppResult<Match> {
    repositories::r#match::find_owned(&state.db, id, player.id)
        .await?
        .ok_or_else(|| AppError::NotFound("Match not found.".into()))
}

async fn load_linked_player(state: &AppState, user: &User) -> AppResult<DotaPlayer> {
    repositories::dota_player::find_by_user_id(&state.db, user.id)
        .await?
        .ok_or(AppError::DotaAccountNotLinked)
}

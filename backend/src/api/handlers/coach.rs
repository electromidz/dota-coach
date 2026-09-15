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
use serde::Serialize;
use uuid::Uuid;

use crate::api::extract::{AppPath, CurrentUser, EntitledUser};
use crate::api::handlers::{benchmark, heroes};
use crate::domain::coaching::{AnalysisScope, CoachingAnalysis, Evidence};
use crate::domain::player::DotaPlayer;
use crate::domain::player_model::{PatternStatus, PlayerModel, RecurringPattern};
use crate::domain::r#match::Match;
use crate::domain::training::{ProgressSeries, TrainingFocus};
use crate::domain::user::User;
use crate::error::{AppError, AppResult};
use crate::repositories;
use crate::repositories::coaching::NewAnalysis;
use crate::services::coaching::{self, evidence::EvidenceInputs, CoachingError};
use crate::services::llm::LlmError;
use crate::services::player_model::{self, patterns, ModelInputs};
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
    pub patterns: Vec<RecurringPattern>,
    pub note: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct TrainingFocusResponse {
    /// The one thing to work on, or `null` when nothing clears the bar.
    pub focus: Option<TrainingFocus>,
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
    let (evidence, patterns) = player_evidence(&state, &user, &player).await?;
    let stored = repositories::coaching::latest(&state.db, player.id, None).await?;

    Ok(Json(respond(
        &state, stored, evidence, patterns, None, true,
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
    let refreshed = player_model::refresh(&state.db, player.id).await?;
    let stored = repositories::player_model::list_patterns(&state.db, player.id).await?;

    let active = hydrate_patterns(refreshed.patterns, &stored);
    let resolved: Vec<RecurringPattern> = stored
        .into_iter()
        .filter(|p| p.status == PatternStatus::Resolved)
        .collect();

    let benchmark = benchmark::build(&state, &player, None, None).await?;
    let heroes = heroes::build(
        &state,
        &user,
        Some(state.config.heroes.recommendation_limit),
    )
    .await?;
    let roles = repositories::metrics::role_stats(&state.db, player.id).await?;

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

    let refreshed = player_model::refresh(&state.db, player.id).await?;
    let benchmark = benchmark::build(&state, &player, None, None).await?;

    let inputs = SelectionInputs {
        history: &refreshed.history,
        benchmarks: &benchmark.results,
        patterns: &refreshed.patterns,
        now: Utc::now(),
    };

    let focus = training::ensure(
        &state.db,
        player.id,
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

    let history =
        repositories::training::history(&state.db, player.id, state.config.training.history_limit)
            .await?;

    Ok(Json(TrainingFocusResponse {
        note: focus.is_none().then(|| {
            if refreshed.history.is_empty() {
                "Sync some matches first — there is nothing to train on yet.".to_string()
            } else {
                "Nothing stands out as a training focus right now. Keep playing and check back."
                    .to_string()
            }
        }),
        focus,
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
    let (evidence, patterns) = player_evidence(&state, &user, &player).await?;

    run(
        &state,
        &player,
        AnalysisScope::Player,
        None,
        evidence,
        patterns,
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
        AnalysisScope::Match,
        Some(id),
        evidence,
        patterns,
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

    let (evidence, patterns) = match_evidence(&state, &user, &player, &match_).await?;
    let stored = repositories::coaching::latest(&state.db, player.id, Some(id)).await?;

    Ok(Json(respond(
        &state, stored, evidence, patterns, None, true,
    )))
}

/// Generate, verify and store — or hand back the answer to the same question.
async fn run(
    state: &AppState,
    player: &DotaPlayer,
    scope: AnalysisScope,
    match_id: Option<Uuid>,
    evidence: Vec<Evidence>,
    patterns: Vec<RecurringPattern>,
) -> AppResult<Json<CoachResponse>> {
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

    // Keyed on the configured model rather than the one that answers: it is
    // what the *next* request would use, and a served-model change behind the
    // same configuration is not a different question.
    let hash = coaching::context_hash(&evidence, scope, &state.config.llm.model);

    if let Some(cached) =
        repositories::coaching::find_by_hash(&state.db, player.id, match_id, &hash).await?
    {
        // Same evidence, same answer. Costs nothing and spends no budget.
        return Ok(Json(respond(
            state,
            Some(cached),
            evidence,
            patterns,
            None,
            true,
        )));
    }

    enforce_limits(state, player).await?;

    let generated = coaching::generate(state.llm.as_ref(), scope, &evidence, &state.config.coach)
        .await
        .map_err(coaching_error)?;

    repositories::coaching::insert(
        &state.db,
        &NewAnalysis {
            dota_player_id: player.id,
            match_id,
            scope,
            context_hash: &hash,
            model: &generated.model,
            summary: &generated.summary,
            evidence: &evidence,
            insights: &generated.insights,
        },
    )
    .await?;

    let stored =
        repositories::coaching::find_by_hash(&state.db, player.id, match_id, &hash).await?;
    Ok(Json(respond(
        state, stored, evidence, patterns, None, false,
    )))
}

fn respond(
    state: &AppState,
    analysis: Option<CoachingAnalysis>,
    evidence: Vec<Evidence>,
    patterns: Vec<RecurringPattern>,
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

/// Career evidence: stats, form, peer comparison and repertoire.
async fn player_evidence(
    state: &AppState,
    user: &User,
    player: &DotaPlayer,
) -> AppResult<(Vec<Evidence>, Vec<RecurringPattern>)> {
    let stats = repositories::metrics::player_stats(&state.db, player.id).await?;
    // Patterns are what let an insight be about a habit rather than an
    // average, so they are detected before the evidence is assembled.
    let refreshed = player_model::refresh(&state.db, player.id).await?;
    let detected = refreshed.patterns;
    let recent = repositories::r#match::list_by_player(
        &state.db,
        player.id,
        state.config.coach.recent_matches,
        0,
    )
    .await?;

    // Both of these degrade internally rather than failing: a benchmark or
    // meta outage removes evidence, it does not remove the coach.
    let benchmark = benchmark::build(state, player, None, None).await?;
    let heroes = heroes::build(state, user, Some(state.config.heroes.recommendation_limit)).await?;
    let focus = active_focus(
        state,
        player,
        &refreshed.history,
        &benchmark.results,
        &detected,
    )
    .await?;

    let evidence = coaching::evidence::build(&EvidenceInputs {
        stats: &stats,
        recent: &recent,
        benchmarks: &benchmark.results,
        benchmark_hero: (!benchmark.hero_name.is_empty()).then_some(benchmark.hero_name.as_str()),
        pool: &heroes.pool,
        recommendations: &heroes.recommendations,
        patterns: &detected,
        focus: focus.as_ref(),
        focus_match: None,
        focus_metrics: None,
    });

    Ok((evidence, detected))
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
    let stats = repositories::metrics::player_stats(&state.db, player.id).await?;
    let refreshed = player_model::refresh(&state.db, player.id).await?;
    let detected = refreshed.patterns;
    let metrics = repositories::metrics::for_match(&state.db, match_.id).await?;
    let benchmark = benchmark::build(state, player, Some(match_.hero_id), None).await?;
    let heroes = heroes::build(state, user, Some(state.config.heroes.recommendation_limit)).await?;
    let focus = active_focus(
        state,
        player,
        &refreshed.history,
        &benchmark.results,
        &detected,
    )
    .await?;

    let evidence = coaching::evidence::build(&EvidenceInputs {
        stats: &stats,
        recent: &[],
        benchmarks: &benchmark.results,
        benchmark_hero: (!benchmark.hero_name.is_empty()).then_some(benchmark.hero_name.as_str()),
        pool: &heroes.pool,
        recommendations: &[],
        // A single match read against the player's habits, not just their
        // averages: "you did it again" is the coachable observation.
        patterns: &detected,
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
    history: &[crate::domain::player_model::AnalyzedMatch],
    benchmarks: &[crate::domain::benchmark::BenchmarkResult],
    patterns: &[RecurringPattern],
) -> AppResult<Option<TrainingFocus>> {
    let Some(stored) = repositories::training::active(&state.db, player.id).await? else {
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

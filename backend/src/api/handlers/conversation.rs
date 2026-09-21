//! Conversational coaching.
//!
//! Reading a conversation is free and never calls a model. Asking a question
//! is the third thing in this service that spends one, and it is gated and
//! limited like the other two.
//!
//! The analysis is **not** re-run per message. A question reads the same
//! prepared evidence `GET /api/coach` does, through the same cache, so asking
//! five follow-ups costs one context build rather than five. That is the whole
//! reason the cache in Phase 5 exists.

use axum::extract::State;
use axum::Json;
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::api::extract::{AppQuery, CurrentUser, EntitledUser};
use crate::api::handlers::coach;
use crate::domain::conversation::{Conversation, Message, HISTORY_TURNS, MAX_QUESTION_CHARS};
use crate::domain::event::EventType;
use crate::domain::player::DotaPlayer;
use crate::domain::role::CoachableRole;
use crate::domain::user::User;
use crate::error::{AppError, AppResult};
use crate::repositories;
use crate::services::coaching::chat;
use crate::services::events;
use crate::services::llm::LlmError;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct ConversationQuery {
    /// Role slug. Omit to read the role currently being coached.
    pub role: Option<String>,
}

#[derive(Deserialize, ToSchema)]
pub struct AskRequest {
    /// The player's question, in their own words.
    pub question: String,
}

#[derive(Serialize, ToSchema)]
pub struct ConversationResponse {
    pub role: CoachableRole,
    pub role_label: &'static str,
    /// Oldest first. Empty before the first question.
    pub messages: Vec<Message>,
    /// False when no model is configured on this deployment — the transcript
    /// is still readable, there is just nothing to ask.
    pub llm_available: bool,
    pub note: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct AskResponse {
    pub role: CoachableRole,
    pub role_label: &'static str,
    /// The reply just given.
    pub message: Message,
}

/// `GET /api/coach/conversation`
#[utoipa::path(
    get, path = "/api/coach/conversation", tag = "coaching",
    summary = "The coaching conversation for one role",
    description = "Reads only, and never calls a model. Chat history is conversation context, \
not player truth — no figure is ever read back out of a message, and a reply's `evidence` ids \
point at the backend statements its numbers came from.",
    security(("session" = [])),
    params(
        ("role" = Option<String>, Query,
            description = "Role slug. Omit to read the role currently being coached.",
            example = "carry"),
    ),
    responses(
        (status = 200, description = "The transcript, oldest first", body = ConversationResponse),
        (status = 400, description = "Unknown role slug", body = crate::error::ErrorBody),
        (status = 409, description = "No Dota account linked, or no role chosen yet", body = crate::error::ErrorBody),
        (status = 401, description = "No session cookie, or it has expired", body = crate::error::ErrorBody),
        (status = 500, description = "Database or internal failure", body = crate::error::ErrorBody),
    )
)]
pub async fn get(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    AppQuery(query): AppQuery<ConversationQuery>,
) -> AppResult<Json<ConversationResponse>> {
    let player = load_linked_player(&state, &user).await?;
    let role = resolve_role(&state, &player, query.role.as_deref()).await?;

    let stored: Option<Conversation> =
        repositories::conversation::find(&state.db, player.id, role, 200).await?;

    let llm_available = state.llm.is_configured();

    Ok(Json(ConversationResponse {
        role,
        role_label: role.label(),
        messages: stored.map(|c| c.messages).unwrap_or_default(),
        llm_available,
        note: (!llm_available).then(|| LlmError::NotConfigured.user_note().to_string()),
    }))
}

/// `POST /api/coach/conversation`
#[utoipa::path(
    post, path = "/api/coach/conversation", tag = "coaching",
    summary = "Ask the coach a question",
    description = "Premium: this is one of the three verbs that spends a model call.\n\n\
The answer is grounded in the same prepared evidence `GET /api/coach` serves, read through the \
coaching cache — asking five follow-up questions costs one context build, not five. The \
analysis is never re-run per message.\n\n\
Every figure in the reply is checked against that evidence before the reply is stored. An \
answer quoting a number the player's data does not contain is discarded rather than shown, and \
answers with `502`.\n\n\
The question is treated as data, not instructions. Earlier turns are replayed to the model \
role-tagged, so a message cannot forge a coach turn.",
    security(("session" = [])),
    request_body = AskRequest,
    responses(
        (status = 200, description = "The coach's reply, already stored", body = AskResponse),
        (status = 400, description = "Empty question, or one over the length limit", body = crate::error::ErrorBody),
        (status = 402, description = "Trial expired and no active subscription", body = crate::error::ErrorBody),
        (status = 409, description = "No role chosen, or nothing to coach on yet", body = crate::error::ErrorBody),
        (status = 429, description = "Daily question limit reached", body = crate::error::ErrorBody),
        (status = 502, description = "The answer could not be verified against the player's data", body = crate::error::ErrorBody),
        (status = 503, description = "No model is configured on this deployment", body = crate::error::ErrorBody),
        (status = 401, description = "No session cookie, or it has expired", body = crate::error::ErrorBody),
        (status = 500, description = "Database or internal failure", body = crate::error::ErrorBody),
    )
)]
pub async fn ask(
    State(state): State<AppState>,
    EntitledUser(user): EntitledUser,
    crate::api::extract::AppJson(body): crate::api::extract::AppJson<AskRequest>,
) -> AppResult<Json<AskResponse>> {
    let question = body.question.trim();
    if question.is_empty() {
        return Err(AppError::BadRequest("Ask something first.".into()));
    }
    if question.chars().count() > MAX_QUESTION_CHARS {
        return Err(AppError::BadRequest(format!(
            "That question is too long — keep it under {MAX_QUESTION_CHARS} characters."
        )));
    }

    let player = load_linked_player(&state, &user).await?;
    let scope = coach::require_scope(&state, &player).await?;

    if !state.llm.is_configured() {
        return Err(AppError::FeatureUnavailable(
            LlmError::NotConfigured.user_note().to_string(),
        ));
    }

    enforce_question_limit(&state, &player).await?;

    // The same evidence the coach page shows, through the same cache. This is
    // the line that keeps a conversation from re-analysing a hundred matches
    // per message.
    let (evidence, _) = coach::role_evidence(&state, &user, &player, &scope).await?;
    if evidence.is_empty() {
        return Err(AppError::PreconditionUnmet(
            "There is nothing to discuss yet — sync some matches first.".into(),
        ));
    }

    let conversation_id =
        repositories::conversation::ensure(&state.db, player.id, scope.role).await?;
    let history =
        repositories::conversation::recent(&state.db, conversation_id, HISTORY_TURNS).await?;

    events::track(
        &state.db,
        player.user_id,
        EventType::FeatureUsed,
        serde_json::json!({ "scope": "conversation" }),
    )
    .await;

    let reply = chat::answer(
        state.llm.as_ref(),
        question,
        &evidence,
        &history,
        state.config.coach.max_output_tokens,
        state.config.coach.temperature,
    )
    .await
    .map_err(chat_error)?;

    repositories::conversation::append_exchange(
        &state.db,
        conversation_id,
        question,
        &reply.text,
        &reply.evidence,
        &reply.model,
    )
    .await?;

    // Re-read so the response carries the stored ids and timestamps rather
    // than a hand-built copy that could drift from what was persisted.
    let stored = repositories::conversation::recent(&state.db, conversation_id, 1).await?;
    let message = stored.into_iter().next().ok_or_else(|| {
        AppError::Internal("the reply was stored but could not be read back".into())
    })?;

    Ok(Json(AskResponse {
        role: scope.role,
        role_label: scope.role.label(),
        message,
    }))
}

/// How many questions a player may ask in a rolling day.
///
/// Counted over *replies*, so a question whose answer failed verification does
/// not spend the budget — the same rule the analysis limiter follows.
///
/// There is deliberately no cooldown. A thirty-second gap between messages is
/// right for regenerating an analysis and wrong for a conversation; the daily
/// ceiling is what bounds cost here.
async fn enforce_question_limit(state: &AppState, player: &DotaPlayer) -> AppResult<()> {
    let limit = state.config.coach.chat_daily_limit;
    if limit <= 0 {
        return Ok(());
    }

    let since = Utc::now() - Duration::hours(24);
    let asked =
        repositories::conversation::count_replies_since(&state.db, player.id, since).await?;

    if asked >= limit {
        return Err(AppError::TooManyRequests(format!(
            "You have asked {asked} questions today. The limit is {limit}; it resets as \
             your earlier questions pass 24 hours old."
        )));
    }

    Ok(())
}

fn chat_error(error: chat::ChatError) -> AppError {
    let note = error.user_note();

    match error {
        chat::ChatError::Llm(LlmError::NotConfigured) => AppError::FeatureUnavailable(note),
        chat::ChatError::Llm(LlmError::RateLimited) => AppError::TooManyRequests(note),
        // A refused answer is an upstream problem, not the player's: they
        // asked a reasonable question and the model answered badly.
        chat::ChatError::Unverifiable(_) | chat::ChatError::Empty => AppError::Upstream(note),
        chat::ChatError::Llm(_) => AppError::Upstream(note),
    }
}

/// Which role's conversation. Mirrors the history endpoints.
async fn resolve_role(
    state: &AppState,
    player: &DotaPlayer,
    requested: Option<&str>,
) -> AppResult<CoachableRole> {
    if let Some(slug) = requested.map(str::trim).filter(|s| !s.is_empty()) {
        return CoachableRole::parse(slug).ok_or_else(|| {
            AppError::BadRequest(format!(
                "'{slug}' is not a role that can be coached. Choose one of: {}.",
                CoachableRole::ALL
                    .iter()
                    .map(|r| r.slug())
                    .collect::<Vec<_>>()
                    .join(", "),
            ))
        });
    }

    match coach::scope_for_checkpoint(state, player).await? {
        Some(scope) => Ok(scope.role),
        None => Err(AppError::PreconditionUnmet(
            "Choose the role you want to improve first, or ask for one by name with ?role=.".into(),
        )),
    }
}

async fn load_linked_player(state: &AppState, user: &User) -> AppResult<DotaPlayer> {
    repositories::dota_player::find_by_user_id(&state.db, user.id)
        .await?
        .ok_or(AppError::DotaAccountNotLinked)
}

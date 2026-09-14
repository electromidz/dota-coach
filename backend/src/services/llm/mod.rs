//! LLM provider abstraction.
//!
//! The boundary is deliberately narrow: text in, text out. Everything that
//! makes the coaching layer trustworthy — what the model is allowed to see,
//! what it is allowed to claim, and what happens to its answer — lives above
//! this trait, in `services::coaching`, where it can be tested without a
//! network or a key.
//!
//! Credentials never leave the server, and no provider type escapes this
//! module.

pub mod openai;

use async_trait::async_trait;

/// One generation request.
///
/// There is no conversation history and no tool surface: coaching is a single
/// structured question with a single structured answer, and a chat transcript
/// would only be a place for earlier model output to become an input.
#[derive(Debug, Clone)]
pub struct LlmRequest {
    pub system: String,
    pub user: String,
    pub max_output_tokens: u32,
    /// Low by default: this is interpretation of fixed numbers, not creative
    /// writing, and a high temperature mostly buys embellishment.
    pub temperature: f32,
    /// Ask the provider to constrain the answer to a JSON object. Advisory —
    /// the response is validated regardless of whether the provider honours it.
    pub json_only: bool,
}

#[derive(Debug, Clone)]
pub struct LlmCompletion {
    pub text: String,
    /// The model that actually answered, as the provider reported it. Stored
    /// alongside generated coaching so an answer can be attributed later.
    pub model: String,
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
}

#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    /// No API key. Distinct from an outage: nothing was attempted, and the
    /// fix is configuration rather than patience.
    #[error("llm provider is not configured")]
    NotConfigured,
    #[error("llm provider unavailable: {0}")]
    Unavailable(String),
    #[error("llm provider rate limited")]
    RateLimited,
    #[error("llm provider timed out")]
    Timeout,
    #[error("unexpected llm response: {0}")]
    InvalidResponse(String),
}

impl LlmError {
    /// A short, user-safe explanation. Coaching degrades to its deterministic
    /// half rather than failing, so this is shown as a note, not an error.
    pub fn user_note(&self) -> &'static str {
        match self {
            LlmError::NotConfigured => {
                "AI coaching is not configured on this server, so only the measured evidence is shown."
            }
            LlmError::RateLimited => {
                "The coaching model is rate limiting us. The measured evidence is unchanged."
            }
            LlmError::Timeout => "The coaching model took too long to answer. Try again shortly.",
            _ => "AI coaching is unavailable right now, so only the measured evidence is shown.",
        }
    }
}

#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Whether a call is even possible. Checked before the rate limiter, so an
    /// unconfigured server never consumes a user's daily budget.
    fn is_configured(&self) -> bool;

    async fn generate(&self, request: &LlmRequest) -> Result<LlmCompletion, LlmError>;
}

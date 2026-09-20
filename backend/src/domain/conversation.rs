//! Conversational coaching.
//!
//! The rule that shapes everything here: **chat history is conversation
//! context, structured data is player truth.** A message is a record of what
//! was said, never a source of what is true. No figure is ever read back out
//! of one — when the coach mentions a number for the second time it comes from
//! the evidence again, not from its own earlier sentence.
//!
//! That is why a reply stores the evidence ids it was grounded in. The same
//! provenance a stored insight carries: the text is the model's, every figure
//! in it traces to something the backend computed, and the check runs before
//! the reply is stored rather than after it is read.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::domain::role::CoachableRole;

/// Who said something.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Speaker {
    Player,
    Coach,
}

impl Speaker {
    pub fn slug(self) -> &'static str {
        match self {
            Speaker::Player => "player",
            Speaker::Coach => "coach",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "player" => Some(Speaker::Player),
            "coach" => Some(Speaker::Coach),
            _ => None,
        }
    }
}

/// One turn.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Message {
    pub id: Uuid,
    pub speaker: Speaker,
    pub content: String,
    /// Evidence ids the reply was grounded in. Empty for a player turn.
    pub evidence: Vec<String>,
    /// Which model answered. `None` for a player turn.
    pub model: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// The ongoing discussion about one role.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Conversation {
    pub id: Uuid,
    pub role: CoachableRole,
    pub role_label: &'static str,
    /// Oldest first — transcript order.
    pub messages: Vec<Message>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_message_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// The longest question the coach will read.
///
/// Not a token budget — a question longer than this is not a question. The
/// cap also bounds how much player-controlled text reaches the model at all,
/// which matters more here than anywhere else in the product: this is the one
/// place a player writes into a prompt.
pub const MAX_QUESTION_CHARS: usize = 1_000;

/// How many prior turns are replayed to the model.
///
/// Enough for a follow-up to make sense, bounded so a long-running
/// conversation cannot grow the prompt without limit. The *evidence* is
/// re-sent in full every turn regardless, because that is where facts come
/// from — an older turn scrolling out of the window loses the conversation,
/// never the data.
pub const HISTORY_TURNS: i64 = 10;

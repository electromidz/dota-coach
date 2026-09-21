//! Persistence for conversational coaching.

use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::conversation::{Conversation, Message, Speaker};
use crate::domain::role::CoachableRole;

/// The conversation for one role, creating it if this is the first question.
///
/// Upsert rather than find-then-insert: two questions sent at once would
/// otherwise both find nothing and both insert, and `UNIQUE` would fail the
/// second. `DO UPDATE` on a no-op column is the idiom that makes `RETURNING`
/// work on the conflict path.
pub async fn ensure(
    pool: &PgPool,
    dota_player_id: Uuid,
    role: CoachableRole,
) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO coaching_conversations (dota_player_id, role)
         VALUES ($1, $2)
         ON CONFLICT (dota_player_id, role) DO UPDATE
             SET dota_player_id = EXCLUDED.dota_player_id
         RETURNING id",
    )
    .bind(dota_player_id)
    .bind(role.slug())
    .fetch_one(pool)
    .await
}

/// The stored conversation for one role, if there is one.
///
/// Scoped by player, so a conversation id is never enough on its own.
pub async fn find(
    pool: &PgPool,
    dota_player_id: Uuid,
    role: CoachableRole,
    limit: i64,
) -> Result<Option<Conversation>, sqlx::Error> {
    let Some(row) = sqlx::query_as::<_, ConversationRow>(
        "SELECT id, role, created_at, last_message_at
           FROM coaching_conversations
          WHERE dota_player_id = $1 AND role = $2",
    )
    .bind(dota_player_id)
    .bind(role.slug())
    .fetch_optional(pool)
    .await?
    else {
        return Ok(None);
    };

    let messages = recent(pool, row.id, limit).await?;

    Ok(Some(Conversation {
        id: row.id,
        role,
        role_label: role.label(),
        messages,
        created_at: row.created_at,
        last_message_at: row.last_message_at,
    }))
}

/// The newest `limit` turns, returned oldest first.
///
/// Newest-first in SQL so the limit takes the *recent* end, then reversed so
/// the caller reads a transcript. Taking the oldest N would replay the start
/// of a long conversation and drop what was just said.
///
/// Ordered by `seq`, not `created_at`: a question and its reply share a
/// transaction and therefore share `now()`, so the timestamp cannot separate
/// them.
pub async fn recent(
    pool: &PgPool,
    conversation_id: Uuid,
    limit: i64,
) -> Result<Vec<Message>, sqlx::Error> {
    let mut rows: Vec<MessageRow> = sqlx::query_as(
        "SELECT id, speaker, content, evidence_refs, model, created_at
           FROM coaching_messages
          WHERE conversation_id = $1
          ORDER BY seq DESC
          LIMIT $2",
    )
    .bind(conversation_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    rows.reverse();
    Ok(rows
        .into_iter()
        .filter_map(MessageRow::into_domain)
        .collect())
}

/// Store a question and its reply together.
///
/// One transaction, because a stored question with no answer is a conversation
/// the player cannot continue from, and a stored answer with no question is a
/// transcript that makes no sense.
pub async fn append_exchange(
    pool: &PgPool,
    conversation_id: Uuid,
    question: &str,
    reply: &str,
    evidence: &[String],
    model: &str,
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;

    sqlx::query(
        "INSERT INTO coaching_messages (conversation_id, speaker, content)
         VALUES ($1, 'player', $2)",
    )
    .bind(conversation_id)
    .bind(question)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        "INSERT INTO coaching_messages
             (conversation_id, speaker, content, evidence_refs, model)
         VALUES ($1, 'coach', $2, $3, $4)",
    )
    .bind(conversation_id)
    .bind(reply)
    .bind(evidence)
    .bind(model)
    .execute(&mut *tx)
    .await?;

    sqlx::query("UPDATE coaching_conversations SET last_message_at = now() WHERE id = $1")
        .bind(conversation_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await
}

/// How many replies this player has been given since `since`.
///
/// Counts *coach* turns, so a question that failed validation and produced no
/// answer does not spend the player's budget — the same rule the analysis
/// limiter follows by counting stored analyses rather than attempts.
pub async fn count_replies_since(
    pool: &PgPool,
    dota_player_id: Uuid,
    since: chrono::DateTime<chrono::Utc>,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT COUNT(*)
           FROM coaching_messages m
           JOIN coaching_conversations c ON c.id = m.conversation_id
          WHERE c.dota_player_id = $1
            AND m.speaker = 'coach'
            AND m.created_at >= $2",
    )
    .bind(dota_player_id)
    .bind(since)
    .fetch_one(pool)
    .await
}

#[derive(sqlx::FromRow)]
struct ConversationRow {
    id: Uuid,
    #[allow(dead_code)]
    role: String,
    created_at: chrono::DateTime<chrono::Utc>,
    last_message_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(sqlx::FromRow)]
struct MessageRow {
    id: Uuid,
    speaker: String,
    content: String,
    evidence_refs: Vec<String>,
    model: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
}

impl MessageRow {
    /// A row whose speaker no longer parses is dropped rather than guessed at:
    /// attributing the coach's words to the player, or the reverse, is worse
    /// than a gap in a transcript.
    fn into_domain(self) -> Option<Message> {
        Some(Message {
            id: self.id,
            speaker: Speaker::parse(&self.speaker)?,
            content: self.content,
            evidence: self.evidence_refs,
            model: self.model,
            created_at: self.created_at,
        })
    }
}

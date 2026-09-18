use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::event::EventRecord;

/// Append one event. Never updated, never deleted except by the user's own
/// account being deleted (`ON DELETE CASCADE`).
pub async fn insert(
    pool: &PgPool,
    user_id: Uuid,
    event_type: &str,
    metadata: serde_json::Value,
) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO events (user_id, type, metadata) VALUES ($1, $2, $3)")
        .bind(user_id)
        .bind(event_type)
        .bind(metadata)
        .execute(pool)
        .await?;

    Ok(())
}

/// One account's timeline, newest first — the admin user-detail page.
pub async fn list_for_user(
    pool: &PgPool,
    user_id: Uuid,
    limit: i64,
) -> Result<Vec<EventRecord>, sqlx::Error> {
    sqlx::query_as::<_, EventRecord>(
        "SELECT id, type, metadata, created_at
           FROM events
          WHERE user_id = $1
          ORDER BY created_at DESC
          LIMIT $2",
    )
    .bind(user_id)
    .bind(limit)
    .fetch_all(pool)
    .await
}

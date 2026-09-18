use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::domain::user::User;

pub async fn create(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    token_hash: &str,
    expires_at: DateTime<Utc>,
) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO sessions (user_id, token_hash, expires_at)
         VALUES ($1, $2, $3)
         RETURNING id",
    )
    .bind(user_id)
    .bind(token_hash)
    .bind(expires_at)
    .fetch_one(&mut **tx)
    .await
}

/// Resolve a session token hash to its owner, in one query.
///
/// Expired rows never match, so a stale cookie reads exactly like no cookie.
pub async fn find_user_by_token_hash(
    pool: &PgPool,
    token_hash: &str,
) -> Result<Option<User>, sqlx::Error> {
    sqlx::query_as::<_, User>(
        "SELECT u.id, u.steam_id, u.persona_name, u.avatar_url, u.profile_url,
                u.last_login_at, u.is_admin, u.status, u.created_at, u.updated_at
           FROM sessions s
           JOIN users u ON u.id = s.user_id
          WHERE s.token_hash = $1
            AND s.expires_at > now()",
    )
    .bind(token_hash)
    .fetch_optional(pool)
    .await
}

/// Log out. Deleting by hash means an unknown token is simply a no-op.
pub async fn delete_by_token_hash(pool: &PgPool, token_hash: &str) -> Result<u64, sqlx::Error> {
    let result = sqlx::query("DELETE FROM sessions WHERE token_hash = $1")
        .bind(token_hash)
        .execute(pool)
        .await?;

    Ok(result.rows_affected())
}

/// Drop expired rows. Called at startup; sessions are also filtered on read, so
/// this is housekeeping rather than a security control.
pub async fn delete_expired(pool: &PgPool) -> Result<u64, sqlx::Error> {
    let result = sqlx::query("DELETE FROM sessions WHERE expires_at <= now()")
        .execute(pool)
        .await?;

    Ok(result.rows_affected())
}

//! Storage for the coaching cache.
//!
//! Every function here is allowed to fail without consequence. The caller
//! treats an error as a miss, because the thing being cached is always
//! recomputable and a cache that can break a page is worse than no cache.

use sqlx::PgPool;
use uuid::Uuid;

/// Read an entry, if it exists and is still inside the TTL.
///
/// Freshness is a SQL predicate rather than a check in Rust, so a stale row
/// simply does not match — the same shape `benchmark_snapshots` uses. There is
/// no window where one connection reads a row another is replacing and treats
/// it as current.
pub async fn get(
    pool: &PgPool,
    key: &str,
    ttl_minutes: i64,
) -> Result<Option<serde_json::Value>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT payload
           FROM coaching_cache
          WHERE key = $1
            AND fetched_at > now() - make_interval(mins => $2::int)",
    )
    .bind(key)
    .bind(ttl_minutes as i32)
    .fetch_optional(pool)
    .await
}

/// Write an entry and drop the player's older ones.
///
/// The prune is what keeps this table at roughly one row per player per role
/// rather than one per sync: a fingerprint that moved leaves an entry nothing
/// will ever look up again, and the write that replaced it is the natural
/// moment to notice.
///
/// Both statements share a transaction so a crash between them cannot leave
/// the new entry missing and the old ones gone.
pub async fn put(
    pool: &PgPool,
    key: &str,
    dota_player_id: Uuid,
    payload: &serde_json::Value,
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;

    sqlx::query(
        "INSERT INTO coaching_cache (key, dota_player_id, payload, fetched_at)
         VALUES ($1, $2, $3, now())
         ON CONFLICT (key) DO UPDATE
             SET payload = EXCLUDED.payload,
                 fetched_at = now()",
    )
    .bind(key)
    .bind(dota_player_id)
    .bind(payload)
    .execute(&mut *tx)
    .await?;

    sqlx::query("DELETE FROM coaching_cache WHERE dota_player_id = $1 AND key <> $2")
        .bind(dota_player_id)
        .bind(key)
        .execute(&mut *tx)
        .await?;

    tx.commit().await
}

/// Drop every entry for one player.
///
/// Not needed for correctness — a key that no longer matches is already
/// unreachable — but it exists for the cases where correctness is not the
/// point: a support request, or a deliberate "recompute everything for this
/// account".
pub async fn purge(pool: &PgPool, dota_player_id: Uuid) -> Result<u64, sqlx::Error> {
    let deleted = sqlx::query("DELETE FROM coaching_cache WHERE dota_player_id = $1")
        .bind(dota_player_id)
        .execute(pool)
        .await?
        .rows_affected();

    Ok(deleted)
}

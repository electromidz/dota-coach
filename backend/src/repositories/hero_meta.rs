use sqlx::PgPool;

/// Return a cached hero meta payload, but only while it is still fresh.
///
/// Freshness is a SQL predicate rather than a Rust comparison for the same
/// reason it is on `benchmark_snapshots`: a stale row simply does not match,
/// so there is no window where one connection treats a row another is about to
/// replace as current.
pub async fn fresh_snapshot(
    pool: &PgPool,
    provider: &str,
    context_key: &str,
    ttl_hours: i64,
) -> Result<Option<serde_json::Value>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT payload
           FROM hero_meta_snapshots
          WHERE provider = $1
            AND context_key = $2
            AND fetched_at > now() - make_interval(hours => $3::int)",
    )
    .bind(provider)
    .bind(context_key)
    .bind(ttl_hours as i32)
    .fetch_optional(pool)
    .await
}

/// Store or refresh the snapshot for a context.
pub async fn store_snapshot(
    pool: &PgPool,
    provider: &str,
    context_key: &str,
    payload: &serde_json::Value,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO hero_meta_snapshots (provider, context_key, payload, fetched_at)
         VALUES ($1, $2, $3, now())
         ON CONFLICT (provider, context_key) DO UPDATE
             SET payload = EXCLUDED.payload,
                 fetched_at = now()",
    )
    .bind(provider)
    .bind(context_key)
    .bind(payload)
    .execute(pool)
    .await?;

    Ok(())
}

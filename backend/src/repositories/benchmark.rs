use sqlx::PgPool;

/// Return a cached distribution, but only while it is still fresh.
///
/// Freshness is checked in SQL rather than in Rust so a stale row simply does
/// not match — there is no window where one connection reads a row another is
/// about to replace and treats it as current.
pub async fn fresh_snapshot(
    pool: &PgPool,
    provider: &str,
    hero_id: i32,
    ttl_hours: i64,
) -> Result<Option<serde_json::Value>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT payload
           FROM benchmark_snapshots
          WHERE provider = $1
            AND hero_id = $2
            AND fetched_at > now() - make_interval(hours => $3::int)",
    )
    .bind(provider)
    .bind(hero_id)
    .bind(ttl_hours as i32)
    .fetch_optional(pool)
    .await
}

/// Store or refresh the snapshot for a hero.
pub async fn store_snapshot(
    pool: &PgPool,
    provider: &str,
    hero_id: i32,
    payload: &serde_json::Value,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO benchmark_snapshots (provider, hero_id, payload, fetched_at)
         VALUES ($1, $2, $3, now())
         ON CONFLICT (provider, hero_id) DO UPDATE
             SET payload = EXCLUDED.payload,
                 fetched_at = now()",
    )
    .bind(provider)
    .bind(hero_id)
    .bind(payload)
    .execute(pool)
    .await?;

    Ok(())
}

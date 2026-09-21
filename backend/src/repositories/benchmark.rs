use sqlx::PgPool;

/// The cache key's "all ranks" bracket.
///
/// A sentinel rather than a `NULL` because the unique constraint has to hold
/// for this row too, and `NULL` never conflicts with anything.
pub const ALL_RANKS: i16 = 0;

/// Return a cached distribution, but only while it is still fresh.
///
/// Freshness is checked in SQL rather than in Rust so a stale row simply does
/// not match — there is no window where one connection reads a row another is
/// about to replace and treats it as current.
///
/// `bracket` is part of the key, not a filter: the Divine distribution for a
/// hero and the all-ranks one are different documents, and serving either in
/// place of the other would quietly change what a percentile is a percentile
/// *of*.
pub async fn fresh_snapshot(
    pool: &PgPool,
    provider: &str,
    hero_id: i32,
    bracket: i16,
    ttl_hours: i64,
) -> Result<Option<serde_json::Value>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT payload
           FROM benchmark_snapshots
          WHERE provider = $1
            AND hero_id = $2
            AND bracket = $3
            AND fetched_at > now() - make_interval(hours => $4::int)",
    )
    .bind(provider)
    .bind(hero_id)
    .bind(bracket)
    .bind(ttl_hours as i32)
    .fetch_optional(pool)
    .await
}

/// Store or refresh the snapshot for a hero in one bracket.
pub async fn store_snapshot(
    pool: &PgPool,
    provider: &str,
    hero_id: i32,
    bracket: i16,
    payload: &serde_json::Value,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO benchmark_snapshots (provider, hero_id, bracket, payload, fetched_at)
         VALUES ($1, $2, $3, $4, now())
         ON CONFLICT (provider, hero_id, bracket) DO UPDATE
             SET payload = EXCLUDED.payload,
                 fetched_at = now()",
    )
    .bind(provider)
    .bind(hero_id)
    .bind(bracket)
    .bind(payload)
    .execute(pool)
    .await?;

    Ok(())
}

//! Storage for provider match-timeline snapshots.
//!
//! A cache, so every function here is allowed to fail without consequence: the
//! caller treats an error as a miss and asks the provider again. See
//! `migrations/0023_match_fact_snapshots.sql` for why this is a separate table
//! from `coaching_cache`.

use sqlx::PgPool;

/// One stored snapshot, with the flag that decides whether it may be reused.
pub struct Snapshot {
    pub parsed: bool,
    pub payload: serde_json::Value,
}

/// Return the stored snapshot for a match and player, if it is still reusable.
///
/// Two TTLs rather than one, because the two cases are different facts:
///
///   * A **parsed** match is immutable. `parsed_ttl_hours` only bounds how long
///     a normalization change takes to reach an already-cached match.
///   * An **unparsed** match is a statement about now. Valve parses replays
///     after the fact, so `unparsed_ttl_hours` is how long the answer "no
///     timeline available" is allowed to stand before it is re-checked.
///
/// Freshness is a SQL predicate rather than a Rust comparison, the same as every
/// other snapshot table here: a stale row simply does not match, so there is no
/// window in which one connection treats a row another is replacing as current.
pub async fn fresh_snapshot(
    pool: &PgPool,
    provider: &str,
    match_id: i64,
    account_id: i64,
    parsed_ttl_hours: i64,
    unparsed_ttl_hours: i64,
) -> Result<Option<Snapshot>, sqlx::Error> {
    let row: Option<(bool, serde_json::Value)> = sqlx::query_as(
        "SELECT parsed, payload
           FROM match_fact_snapshots
          WHERE provider = $1
            AND match_id = $2
            AND account_id = $3
            AND fetched_at > now() - make_interval(
                    hours => CASE WHEN parsed THEN $4::int ELSE $5::int END
                )",
    )
    .bind(provider)
    .bind(match_id)
    .bind(account_id)
    .bind(parsed_ttl_hours as i32)
    .bind(unparsed_ttl_hours as i32)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|(parsed, payload)| Snapshot { parsed, payload }))
}

/// Store or refresh the snapshot for one match and player.
pub async fn store_snapshot(
    pool: &PgPool,
    provider: &str,
    match_id: i64,
    account_id: i64,
    parsed: bool,
    payload: &serde_json::Value,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO match_fact_snapshots
             (provider, match_id, account_id, parsed, payload, fetched_at)
         VALUES ($1, $2, $3, $4, $5, now())
         ON CONFLICT (provider, match_id, account_id) DO UPDATE
             SET parsed = EXCLUDED.parsed,
                 payload = EXCLUDED.payload,
                 fetched_at = now()",
    )
    .bind(provider)
    .bind(match_id)
    .bind(account_id)
    .bind(parsed)
    .bind(payload)
    .execute(pool)
    .await?;

    Ok(())
}

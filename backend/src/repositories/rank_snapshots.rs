//! Rank history reads and writes.
//!
//! Keyed by `dota_account_id`, not the `dota_players.id` UUID every other
//! child table uses. A rank history is a fact about the Dota account rather
//! than about this application's row for it, and the account id is what the
//! sync already holds when the provider's profile comes back.
//!
//! The practical consequence, for whatever reads this next: joining snapshots
//! to `matches` (keyed by `dota_player_id UUID`) needs a hop through
//! `dota_players`. That is an indexed unique lookup, but it is not a direct
//! join, and code that assumes otherwise will not compile against this.

use chrono::{DateTime, Utc};
use sqlx::PgPool;

use crate::domain::calibration::RankSnapshot;

/// The expression `rank_snapshots_player_day_key` is built on.
///
/// Repeated verbatim in the conflict target because Postgres infers the index
/// by matching the expression, and a near-miss does not fall back to anything
/// — it raises "no unique or exclusion constraint matching the ON CONFLICT
/// specification" at runtime. A macro rather than a `const` because sqlx only
/// accepts `&'static str` queries, and `concat!` keeps the composed SQL a
/// compile-time literal (same reason `dota_player.rs` spells its columns this
/// way).
macro_rules! day_key {
    () => {
        "(dota_account_id, ((captured_at AT TIME ZONE 'UTC')::date))"
    };
}

/// Record where this account stands today, replacing today's reading if one
/// already exists.
///
/// At most one row per account per UTC day: a player syncing six times in an
/// afternoon should move the day's point, not stack six identical ones. The
/// last sync of the day wins, which is the reading closest to where they
/// finished playing.
///
/// A `None` rank is written, not skipped. A private profile genuinely reports
/// no medal, and storing that is what keeps a later trajectory honest about
/// the gap instead of drawing through it.
pub async fn insert_snapshot(
    pool: &PgPool,
    dota_account_id: i64,
    rank_tier: Option<i32>,
    leaderboard_rank: Option<i32>,
) -> Result<(), sqlx::Error> {
    sqlx::query(concat!(
        "INSERT INTO rank_snapshots (dota_account_id, rank_tier, leaderboard_rank)
         VALUES ($1, $2, $3)
         ON CONFLICT ",
        day_key!(),
        " DO UPDATE SET rank_tier = EXCLUDED.rank_tier,
                        leaderboard_rank = EXCLUDED.leaderboard_rank,
                        captured_at = now()"
    ))
    .bind(dota_account_id)
    // The column is SMALLINT; the rest of the codebase speaks i32 for a rank
    // tier, so the narrowing happens here rather than spreading a second
    // integer width through the domain. Rank tiers max out at 80, so nothing
    // real is ever truncated.
    .bind(rank_tier.map(|t| t as i16))
    .bind(leaderboard_rank)
    .execute(pool)
    .await?;

    Ok(())
}

/// This account's rank history, oldest first, from `since` onward.
///
/// Ascending because every consumer is drawing or walking a timeline, and a
/// caller that wants the latest reading wants `last()`, not a re-sort.
pub async fn list_for_player(
    pool: &PgPool,
    dota_account_id: i64,
    since: DateTime<Utc>,
) -> Result<Vec<RankSnapshot>, sqlx::Error> {
    sqlx::query_as::<_, RankSnapshot>(
        "SELECT rank_tier, leaderboard_rank, captured_at
           FROM rank_snapshots
          WHERE dota_account_id = $1
            AND captured_at >= $2
          ORDER BY captured_at ASC",
    )
    .bind(dota_account_id)
    .bind(since)
    .fetch_all(pool)
    .await
}

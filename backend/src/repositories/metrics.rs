use sqlx::{AssertSqlSafe, PgPool};
use uuid::Uuid;

use crate::domain::eligibility::ModeCount;
use crate::domain::metrics::{HeroStats, MatchMetrics, PlayerStats, RoleStats};
use crate::domain::role::RoleTotals;
use crate::domain::scope::MatchScope;

/// Store (or replace) the derived metrics for a match.
///
/// Recomputation overwrites rather than appends: there is exactly one current
/// answer per match, stamped with the formula version that produced it.
pub async fn upsert(pool: &PgPool, m: &MatchMetrics) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO match_metrics (
             match_id, metrics_version, kda, kills_per_10, deaths_per_10, assists_per_10,
             last_hits_per_min, hero_damage_per_min, tower_damage_per_min,
             kill_participation, gold_advantage_at_10, computed_at
         )
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, now())
         ON CONFLICT (match_id) DO UPDATE SET
             metrics_version      = EXCLUDED.metrics_version,
             kda                  = EXCLUDED.kda,
             kills_per_10         = EXCLUDED.kills_per_10,
             deaths_per_10        = EXCLUDED.deaths_per_10,
             assists_per_10       = EXCLUDED.assists_per_10,
             last_hits_per_min    = EXCLUDED.last_hits_per_min,
             hero_damage_per_min  = EXCLUDED.hero_damage_per_min,
             tower_damage_per_min = EXCLUDED.tower_damage_per_min,
             kill_participation   = EXCLUDED.kill_participation,
             gold_advantage_at_10 = EXCLUDED.gold_advantage_at_10,
             computed_at          = now()",
    )
    .bind(m.match_id)
    .bind(m.metrics_version)
    .bind(m.kda)
    .bind(m.kills_per_10)
    .bind(m.deaths_per_10)
    .bind(m.assists_per_10)
    .bind(m.last_hits_per_min)
    .bind(m.hero_damage_per_min)
    .bind(m.tower_damage_per_min)
    .bind(m.kill_participation)
    .bind(m.gold_advantage_at_10)
    .execute(pool)
    .await?;

    Ok(())
}

/// The stored metrics for one match, if they have been computed.
///
/// Returned as an option rather than defaulted: a match whose metrics have not
/// been computed yet is a different thing from one whose every metric is zero.
pub async fn for_match(pool: &PgPool, match_id: Uuid) -> Result<Option<MatchMetrics>, sqlx::Error> {
    sqlx::query_as::<_, MatchMetrics>(
        "SELECT match_id, metrics_version, kda, kills_per_10, deaths_per_10, assists_per_10,
                last_hits_per_min, hero_damage_per_min, tower_damage_per_min,
                kill_participation, gold_advantage_at_10
           FROM match_metrics
          WHERE match_id = $1",
    )
    .bind(match_id)
    .fetch_optional(pool)
    .await
}

/// Aggregate across every stored match for a player.
///
/// The career reading: every mode, every role, no window. Kept for the pages
/// that legitimately show a player their whole history.
pub async fn player_stats(pool: &PgPool, dota_player_id: Uuid) -> Result<PlayerStats, sqlx::Error> {
    player_stats_scoped(pool, dota_player_id, &MatchScope::career()).await
}

/// The same aggregate over one [`MatchScope`].
///
/// One query body for every population: the scope decides which matches reach
/// it, and there is no second copy of the arithmetic that could disagree.
///
/// Averages that depend on an optional input are averaged over the rows that
/// actually have it, and the row count comes back alongside, so a caller can
/// tell a solid figure from one built on two games.
pub async fn player_stats_scoped(
    pool: &PgPool,
    dota_player_id: Uuid,
    scope: &MatchScope,
) -> Result<PlayerStats, sqlx::Error> {
    /// The aggregate row, straight from SQL. Averages arrive as `float8`
    /// because Postgres widens them; they are narrowed once, below.
    #[derive(sqlx::FromRow)]
    struct Row {
        matches: i64,
        wins: i64,
        avg_kda: Option<f64>,
        avg_gpm: Option<f64>,
        avg_xpm: Option<f64>,
        avg_last_hits: Option<f64>,
        avg_deaths_per_10: Option<f64>,
        avg_kills_per_10: Option<f64>,
        avg_hero_damage: Option<f64>,
        avg_kill_participation: Option<f64>,
        kp_sample: i64,
        parsed: i64,
    }

    let row = sqlx::query_as::<_, Row>(AssertSqlSafe(format!(
        "{cte}
         SELECT
             COUNT(*)                                        AS matches,
             COUNT(*) FILTER (WHERE m.won)                   AS wins,
             AVG(mm.kda)::float8                             AS avg_kda,
             AVG(m.gpm)::float8                              AS avg_gpm,
             AVG(m.xpm)::float8                              AS avg_xpm,
             AVG(m.last_hits)::float8                        AS avg_last_hits,
             AVG(mm.deaths_per_10)::float8                   AS avg_deaths_per_10,
             AVG(mm.kills_per_10)::float8                    AS avg_kills_per_10,
             AVG(m.hero_damage)::float8                      AS avg_hero_damage,
             AVG(mm.kill_participation)::float8              AS avg_kill_participation,
             COUNT(mm.kill_participation)                    AS kp_sample,
             COUNT(*) FILTER (WHERE m.replay_parsed)         AS parsed
           FROM matches m
           JOIN match_metrics mm ON mm.match_id = m.id
           {join}
          WHERE m.dota_player_id = $1",
        cte = scope.cte(),
        join = scope.join(),
    )))
    .bind(dota_player_id)
    .fetch_one(pool)
    .await?;

    let narrow = |v: Option<f64>| v.map(|x| x as f32);

    Ok(PlayerStats {
        matches: row.matches,
        wins: row.wins,
        losses: row.matches - row.wins,
        // No matches means no win rate, not a 0% one.
        win_rate: (row.matches > 0).then(|| row.wins as f32 / row.matches as f32),
        avg_kda: narrow(row.avg_kda),
        avg_gpm: narrow(row.avg_gpm),
        avg_xpm: narrow(row.avg_xpm),
        avg_last_hits: narrow(row.avg_last_hits),
        avg_deaths_per_10: narrow(row.avg_deaths_per_10),
        avg_kills_per_10: narrow(row.avg_kills_per_10),
        avg_hero_damage: narrow(row.avg_hero_damage),
        avg_kill_participation: narrow(row.avg_kill_participation),
        kill_participation_sample: row.kp_sample,
        parsed_matches: row.parsed,
    })
}

pub async fn hero_stats(
    pool: &PgPool,
    dota_player_id: Uuid,
    limit: i64,
) -> Result<Vec<HeroStats>, sqlx::Error> {
    hero_stats_scoped(pool, dota_player_id, &MatchScope::career(), limit).await
}

/// Per-hero rollup over one [`MatchScope`].
///
/// With a role scope this is "your heroes *in that role*" — a Phantom Assassin
/// safe-lane record does not follow the player into their support coaching.
pub async fn hero_stats_scoped(
    pool: &PgPool,
    dota_player_id: Uuid,
    scope: &MatchScope,
    limit: i64,
) -> Result<Vec<HeroStats>, sqlx::Error> {
    sqlx::query_as::<_, HeroStats>(AssertSqlSafe(format!(
        "{cte}
         SELECT
             m.hero_id,
             MIN(m.hero_name)                                           AS hero_name,
             COUNT(*)                                                   AS matches,
             COUNT(*) FILTER (WHERE m.won)                              AS wins,
             (COUNT(*) FILTER (WHERE m.won))::real / COUNT(*)::real     AS win_rate,
             AVG(mm.kda)::real                                          AS avg_kda,
             AVG(m.gpm)::real                                           AS avg_gpm,
             MAX(m.started_at)                                          AS last_played_at
           FROM matches m
           JOIN match_metrics mm ON mm.match_id = m.id
           {join}
          WHERE m.dota_player_id = $1
          GROUP BY m.hero_id
          ORDER BY matches DESC, last_played_at DESC
          LIMIT $2",
        cte = scope.cte(),
        join = scope.join(),
    )))
    .bind(dota_player_id)
    .bind(limit)
    .fetch_all(pool)
    .await
}

pub async fn role_stats(
    pool: &PgPool,
    dota_player_id: Uuid,
) -> Result<Vec<RoleStats>, sqlx::Error> {
    role_stats_scoped(pool, dota_player_id, &MatchScope::career()).await
}

/// Per-role rollup over one [`MatchScope`], grouped by the estimator's own
/// labels.
pub async fn role_stats_scoped(
    pool: &PgPool,
    dota_player_id: Uuid,
    scope: &MatchScope,
) -> Result<Vec<RoleStats>, sqlx::Error> {
    sqlx::query_as::<_, RoleStats>(AssertSqlSafe(format!(
        "{cte}
         SELECT
             m.role,
             COUNT(*)                                                   AS matches,
             COUNT(*) FILTER (WHERE m.won)                              AS wins,
             (COUNT(*) FILTER (WHERE m.won))::real / COUNT(*)::real     AS win_rate,
             AVG(mm.kda)::real                                          AS avg_kda,
             AVG(m.gpm)::real                                           AS avg_gpm
           FROM matches m
           JOIN match_metrics mm ON mm.match_id = m.id
           {join}
          WHERE m.dota_player_id = $1
          GROUP BY m.role
          ORDER BY matches DESC, m.role",
        cte = scope.cte(),
        join = scope.join(),
    )))
    .bind(dota_player_id)
    .fetch_all(pool)
    .await
}

/// Per-role **sums** over one scope, for the role performance score.
///
/// Sums rather than averages because `services::roles` merges several stored
/// labels into one coachable role, and merging averages needs the totals back
/// anyway. Grouping stays on the raw label: the label-to-role mapping lives in
/// `domain::role` and is applied in Rust, so there is one copy of it rather
/// than one here and one there.
pub async fn role_totals(
    pool: &PgPool,
    dota_player_id: Uuid,
    scope: &MatchScope,
) -> Result<Vec<RoleTotals>, sqlx::Error> {
    sqlx::query_as::<_, RoleTotals>(AssertSqlSafe(format!(
        "{cte}
         SELECT
             m.role,
             COUNT(*)                                     AS matches,
             COUNT(*) FILTER (WHERE m.won)                AS wins,
             SUM(mm.kda)::float8                          AS kda_sum,
             SUM(m.gpm)::float8                           AS gpm_sum,
             SUM(m.xpm)::float8                           AS xpm_sum,
             SUM(mm.last_hits_per_min)::float8            AS last_hits_per_min_sum,
             SUM(mm.deaths_per_10)::float8                AS deaths_per_10_sum,
             -- Summed and counted over the rows that carry team totals, which
             -- is a smaller set than the role's matches.
             SUM(mm.kill_participation)::float8           AS kill_participation_sum,
             COUNT(mm.kill_participation)                 AS kill_participation_matches
           FROM matches m
           JOIN match_metrics mm ON mm.match_id = m.id
           {join}
          WHERE m.dota_player_id = $1
          GROUP BY m.role
          ORDER BY matches DESC, m.role",
        cte = scope.cte(),
        join = scope.join(),
    )))
    .bind(dota_player_id)
    .fetch_all(pool)
    .await
}

/// Every stored match grouped by the two fields eligibility reads.
///
/// Unscoped on purpose: this is the input to "what did the filter exclude, and
/// why", which is a question about the whole stored history.
pub async fn mode_counts(
    pool: &PgPool,
    dota_player_id: Uuid,
) -> Result<Vec<ModeCount>, sqlx::Error> {
    sqlx::query_as::<_, ModeCount>(
        "SELECT m.game_mode, m.lobby_type, COUNT(*) AS matches
           FROM matches m
          WHERE m.dota_player_id = $1
          GROUP BY m.game_mode, m.lobby_type",
    )
    .bind(dota_player_id)
    .fetch_all(pool)
    .await
}

/// Match ids in one scope, newest first.
///
/// The coaching dataset, enumerated. A coaching profile records exactly which
/// matches its advice rests on, and a test can assert that no ineligible or
/// wrong-role match is among them.
pub async fn scoped_match_ids(
    pool: &PgPool,
    dota_player_id: Uuid,
    scope: &MatchScope,
) -> Result<Vec<Uuid>, sqlx::Error> {
    sqlx::query_scalar(AssertSqlSafe(format!(
        "{cte}
         SELECT m.id
           FROM matches m
           {join}
          WHERE m.dota_player_id = $1
          ORDER BY m.started_at DESC",
        cte = scope.cte(),
        join = scope.join(),
    )))
    .bind(dota_player_id)
    .fetch_all(pool)
    .await
}

/// Matches whose derived metrics are out of date.
///
/// Three ways that happens, and all three must be caught:
///   1. never computed,
///   2. computed by an older formula version,
///   3. the *inputs* changed since — a fact backfill rewriting the match row
///      leaves a metrics row that is the right version but the wrong answer.
///
/// (3) is why the timestamp comparison is here rather than a version check
/// alone; `matches.updated_at` is maintained by trigger, so any write to the
/// row invalidates what was derived from it.
pub async fn stale_match_ids(
    pool: &PgPool,
    dota_player_id: Uuid,
    current_version: i32,
) -> Result<Vec<Uuid>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT m.id
           FROM matches m
           LEFT JOIN match_metrics mm ON mm.match_id = m.id
          WHERE m.dota_player_id = $1
            AND (
                  mm.match_id IS NULL
               OR mm.metrics_version <> $2
               OR mm.computed_at < m.updated_at
            )",
    )
    .bind(dota_player_id)
    .bind(current_version)
    .fetch_all(pool)
    .await
}

/// The player's averaged, benchmark-comparable figures for one hero, inside a
/// scope.
///
/// The scope matters as much as the hero: a Phantom Assassin average that
/// quietly includes Turbo games, or safe-lane games when the player is being
/// coached on mid, is not a figure any percentile should be attached to.
///
/// Averaged in SQL over that hero's matches, in the same per-minute units the
/// provider's distribution uses, so the two sides are directly comparable
/// without the handler doing arithmetic.
#[derive(Debug, sqlx::FromRow)]
pub struct HeroAverages {
    pub sample: i64,
    pub gold_per_min: Option<f64>,
    pub xp_per_min: Option<f64>,
    pub last_hits_per_min: Option<f64>,
    pub kills_per_min: Option<f64>,
    pub deaths_per_min: Option<f64>,
    pub assists_per_min: Option<f64>,
    pub hero_damage_per_min: Option<f64>,
    pub tower_damage: Option<f64>,
}

pub async fn hero_averages(
    pool: &PgPool,
    dota_player_id: Uuid,
    hero_id: i32,
    scope: &MatchScope,
) -> Result<HeroAverages, sqlx::Error> {
    sqlx::query_as::<_, HeroAverages>(AssertSqlSafe(format!(
        "{cte}
         SELECT
             COUNT(*)                                  AS sample,
             AVG(m.gpm)::float8                        AS gold_per_min,
             AVG(m.xpm)::float8                        AS xp_per_min,
             AVG(mm.last_hits_per_min)::float8         AS last_hits_per_min,
             -- The metrics engine stores per-10; the provider speaks per-minute.
             (AVG(mm.kills_per_10) / 10.0)::float8     AS kills_per_min,
             (AVG(mm.deaths_per_10) / 10.0)::float8    AS deaths_per_min,
             (AVG(mm.assists_per_10) / 10.0)::float8   AS assists_per_min,
             AVG(mm.hero_damage_per_min)::float8       AS hero_damage_per_min,
             AVG(m.tower_damage)::float8               AS tower_damage
           FROM matches m
           JOIN match_metrics mm ON mm.match_id = m.id
           {join}
          WHERE m.dota_player_id = $1
            AND m.hero_id = $2",
        cte = scope.cte(),
        join = scope.join(),
    )))
    .bind(dota_player_id)
    .bind(hero_id)
    .fetch_one(pool)
    .await
}

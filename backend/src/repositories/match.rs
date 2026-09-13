use std::collections::HashSet;

use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::r#match::{Match, NewMatch};

/// A macro rather than a `const` because sqlx only accepts `&'static str`
/// queries; `concat!` keeps the composed SQL a compile-time literal.
macro_rules! columns {
    () => {
        "m.id, m.dota_player_id, m.match_id, m.hero_id, m.hero_name, m.role, m.lane_role, \
         m.won, m.duration_seconds, m.kills, m.deaths, m.assists, m.gpm, m.xpm, \
         m.last_hits, m.denies, m.net_worth, m.hero_damage, m.tower_damage, \
         m.hero_healing, m.game_mode, m.lobby_type, m.party_size, m.started_at, \
         m.detail_synced, m.team_kills, m.team_deaths, m.replay_parsed, \
         m.last_hits_at_10, m.last_hits_at_15, m.gold_at_10, m.gold_at_15, \
         m.xp_at_10, m.xp_at_15, m.bkb_seconds, m.blink_seconds, m.midas_seconds, \
         m.teamfight_participation, m.created_at, m.updated_at"
    };
}

/// Match ids already stored for this player. The sync planner diffs against
/// this so nothing is fetched or inserted twice.
pub async fn existing_match_ids(
    pool: &PgPool,
    dota_player_id: Uuid,
) -> Result<HashSet<i64>, sqlx::Error> {
    let ids: Vec<i64> =
        sqlx::query_scalar("SELECT match_id FROM matches WHERE dota_player_id = $1")
            .bind(dota_player_id)
            .fetch_all(pool)
            .await?;

    Ok(ids.into_iter().collect())
}

/// Insert matches, skipping any that already exist.
///
/// Returns how many rows were actually created. `UNIQUE (dota_player_id,
/// match_id)` does the deduplication, so concurrent syncs cannot produce
/// duplicates even if both pass the planner's check.
pub async fn insert_new(pool: &PgPool, matches: &[NewMatch]) -> Result<u64, sqlx::Error> {
    if matches.is_empty() {
        return Ok(0);
    }

    let mut tx = pool.begin().await?;
    let mut inserted = 0;

    for m in matches {
        let d = &m.data;
        let rows = sqlx::query(
            "INSERT INTO matches (
                 dota_player_id, match_id, hero_id, hero_name, role, lane_role, won,
                 duration_seconds, kills, deaths, assists, gpm, xpm, last_hits, denies,
                 net_worth, hero_damage, tower_damage, hero_healing, game_mode, lobby_type,
                 party_size, started_at, detail_synced, team_kills, team_deaths,
                 replay_parsed, last_hits_at_10, last_hits_at_15, gold_at_10, gold_at_15,
                 xp_at_10, xp_at_15, bkb_seconds, blink_seconds, midas_seconds,
                 teamfight_participation
             )
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16,
                     $17, $18, $19, $20, $21, $22, $23, $24, $25, $26, $27, $28, $29, $30,
                     $31, $32, $33, $34, $35, $36, $37)
             ON CONFLICT (dota_player_id, match_id) DO NOTHING",
        )
        .bind(m.dota_player_id)
        .bind(d.match_id)
        .bind(d.hero_id)
        .bind(&m.hero_name)
        .bind(&m.role)
        .bind(d.lane_role)
        .bind(d.won)
        .bind(d.duration_seconds)
        .bind(d.kills)
        .bind(d.deaths)
        .bind(d.assists)
        .bind(d.gpm)
        .bind(d.xpm)
        .bind(d.last_hits)
        .bind(d.denies)
        .bind(d.net_worth)
        .bind(d.hero_damage)
        .bind(d.tower_damage)
        .bind(d.hero_healing)
        .bind(d.game_mode)
        .bind(d.lobby_type)
        .bind(d.party_size)
        .bind(d.started_at)
        .bind(d.from_details)
        .bind(d.team_kills)
        .bind(d.team_deaths)
        .bind(d.replay_parsed)
        .bind(d.last_hits_at_10)
        .bind(d.last_hits_at_15)
        .bind(d.gold_at_10)
        .bind(d.gold_at_15)
        .bind(d.xp_at_10)
        .bind(d.xp_at_15)
        .bind(d.bkb_seconds)
        .bind(d.blink_seconds)
        .bind(d.midas_seconds)
        .bind(d.teamfight_participation)
        .execute(&mut *tx)
        .await?
        .rows_affected();

        inserted += rows;
    }

    tx.commit().await?;
    Ok(inserted)
}

pub async fn list_by_player(
    pool: &PgPool,
    dota_player_id: Uuid,
    limit: i64,
    offset: i64,
) -> Result<Vec<Match>, sqlx::Error> {
    sqlx::query_as::<_, Match>(concat!(
        "SELECT ",
        columns!(),
        ", mm.kda AS metrics_kda
           FROM matches m
           LEFT JOIN match_metrics mm ON mm.match_id = m.id
          WHERE m.dota_player_id = $1
          ORDER BY m.started_at DESC
          LIMIT $2 OFFSET $3"
    ))
    .bind(dota_player_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
}

pub async fn count_by_player(pool: &PgPool, dota_player_id: Uuid) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT COUNT(*) FROM matches WHERE dota_player_id = $1")
        .bind(dota_player_id)
        .fetch_one(pool)
        .await
}

/// Fetch a match **scoped to its owner**.
///
/// Ownership is part of the query rather than a check afterwards: there is no
/// code path that loads someone else's match and then decides what to do.
pub async fn find_owned(
    pool: &PgPool,
    id: Uuid,
    dota_player_id: Uuid,
) -> Result<Option<Match>, sqlx::Error> {
    sqlx::query_as::<_, Match>(concat!(
        "SELECT ",
        columns!(),
        ", mm.kda AS metrics_kda
           FROM matches m
           LEFT JOIN match_metrics mm ON mm.match_id = m.id
          WHERE m.id = $1 AND m.dota_player_id = $2"
    ))
    .bind(id)
    .bind(dota_player_id)
    .fetch_optional(pool)
    .await
}

/// Fetch matches by id, for recomputing their metrics.
pub async fn find_many(pool: &PgPool, ids: &[Uuid]) -> Result<Vec<Match>, sqlx::Error> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }

    sqlx::query_as::<_, Match>(concat!(
        "SELECT ",
        columns!(),
        ", mm.kda AS metrics_kda
           FROM matches m
           LEFT JOIN match_metrics mm ON mm.match_id = m.id
          WHERE m.id = ANY($1)"
    ))
    .bind(ids)
    .fetch_all(pool)
    .await
}

/// Stored matches that predate a fact the metrics engine now needs.
///
/// Deduplication means an existing match is never re-fetched by the normal
/// sync path, so without this a schema addition would only ever apply to
/// matches synced after it landed.
pub async fn missing_facts(
    pool: &PgPool,
    dota_player_id: Uuid,
    limit: i64,
) -> Result<Vec<i64>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT match_id
           FROM matches
          WHERE dota_player_id = $1
            AND team_kills IS NULL
          ORDER BY started_at DESC
          LIMIT $2",
    )
    .bind(dota_player_id)
    .bind(limit)
    .fetch_all(pool)
    .await
}

/// Write freshly fetched facts onto an existing match row.
pub async fn update_facts(
    pool: &PgPool,
    dota_player_id: Uuid,
    d: &crate::domain::r#match::NormalizedMatch,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE matches SET
             denies = COALESCE($3, denies),
             net_worth = COALESCE($4, net_worth),
             hero_damage = COALESCE($5, hero_damage),
             tower_damage = COALESCE($6, tower_damage),
             hero_healing = COALESCE($7, hero_healing),
             team_kills = COALESCE($8, team_kills),
             team_deaths = COALESCE($9, team_deaths),
             replay_parsed = $10,
             last_hits_at_10 = COALESCE($11, last_hits_at_10),
             last_hits_at_15 = COALESCE($12, last_hits_at_15),
             gold_at_10 = COALESCE($13, gold_at_10),
             gold_at_15 = COALESCE($14, gold_at_15),
             xp_at_10 = COALESCE($15, xp_at_10),
             xp_at_15 = COALESCE($16, xp_at_15),
             bkb_seconds = COALESCE($17, bkb_seconds),
             blink_seconds = COALESCE($18, blink_seconds),
             midas_seconds = COALESCE($19, midas_seconds),
             teamfight_participation = COALESCE($20, teamfight_participation),
             detail_synced = TRUE
           WHERE dota_player_id = $1 AND match_id = $2",
    )
    .bind(dota_player_id)
    .bind(d.match_id)
    .bind(d.denies)
    .bind(d.net_worth)
    .bind(d.hero_damage)
    .bind(d.tower_damage)
    .bind(d.hero_healing)
    .bind(d.team_kills)
    .bind(d.team_deaths)
    .bind(d.replay_parsed)
    .bind(d.last_hits_at_10)
    .bind(d.last_hits_at_15)
    .bind(d.gold_at_10)
    .bind(d.gold_at_15)
    .bind(d.xp_at_10)
    .bind(d.xp_at_15)
    .bind(d.bkb_seconds)
    .bind(d.blink_seconds)
    .bind(d.midas_seconds)
    .bind(d.teamfight_participation)
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

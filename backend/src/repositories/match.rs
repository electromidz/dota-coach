use std::collections::HashSet;

use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::r#match::{Match, NewMatch};

/// A macro rather than a `const` because sqlx only accepts `&'static str`
/// queries; `concat!` keeps the composed SQL a compile-time literal.
macro_rules! columns {
    () => {
        "id, dota_player_id, match_id, hero_id, hero_name, role, lane_role, won, \
         duration_seconds, kills, deaths, assists, gpm, xpm, last_hits, denies, \
         net_worth, hero_damage, tower_damage, hero_healing, game_mode, \
         lobby_type, party_size, started_at, detail_synced, created_at, updated_at"
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
                 party_size, started_at, detail_synced
             )
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16,
                     $17, $18, $19, $20, $21, $22, $23, $24)
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
        " FROM matches
           WHERE dota_player_id = $1
           ORDER BY started_at DESC
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
        " FROM matches WHERE id = $1 AND dota_player_id = $2"
    ))
    .bind(id)
    .bind(dota_player_id)
    .fetch_optional(pool)
    .await
}

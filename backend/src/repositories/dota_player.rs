use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::domain::player::{DotaPlayer, PlayerIdentity};

/// A macro rather than a `const` because sqlx only accepts `&'static str`
/// queries; `concat!` keeps the composed SQL a compile-time literal.
macro_rules! columns {
    () => {
        "id, user_id, steam_id, dota_account_id, rank_tier, last_synced_at, \
         created_at, updated_at"
    };
}

/// Link the account to its Dota identity, or return the existing link.
///
/// The identity is derived from the authenticated SteamID64, so re-linking is
/// always a no-op rather than a way to point an account somewhere new.
pub async fn link(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    identity: PlayerIdentity,
) -> Result<DotaPlayer, sqlx::Error> {
    sqlx::query_as::<_, DotaPlayer>(concat!(
        "INSERT INTO dota_players (user_id, steam_id, dota_account_id)
         VALUES ($1, $2, $3)
         ON CONFLICT (user_id) DO UPDATE SET user_id = EXCLUDED.user_id
         RETURNING ",
        columns!()
    ))
    .bind(user_id)
    .bind(identity.steam_id)
    .bind(identity.account_id)
    .fetch_one(&mut **tx)
    .await
}

pub async fn find_by_user_id(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Option<DotaPlayer>, sqlx::Error> {
    sqlx::query_as::<_, DotaPlayer>(concat!(
        "SELECT ",
        columns!(),
        " FROM dota_players WHERE user_id = $1"
    ))
    .bind(user_id)
    .fetch_optional(pool)
    .await
}

pub async fn update_rank(
    pool: &PgPool,
    id: Uuid,
    rank_tier: Option<i32>,
) -> Result<DotaPlayer, sqlx::Error> {
    sqlx::query_as::<_, DotaPlayer>(concat!(
        "UPDATE dota_players SET rank_tier = COALESCE($2, rank_tier) WHERE id = $1 RETURNING ",
        columns!()
    ))
    .bind(id)
    .bind(rank_tier)
    .fetch_one(pool)
    .await
}

pub async fn mark_synced(pool: &PgPool, id: Uuid) -> Result<DotaPlayer, sqlx::Error> {
    sqlx::query_as::<_, DotaPlayer>(concat!(
        "UPDATE dota_players SET last_synced_at = now() WHERE id = $1 RETURNING ",
        columns!()
    ))
    .bind(id)
    .fetch_one(pool)
    .await
}

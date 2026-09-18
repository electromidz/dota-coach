use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::domain::user::{SteamProfileUpdate, User};

/// Every account with no `subscriptions` row at all. Feeds the one-off
/// backfill (`bin/backfill_subscriptions`) — everyone else already gets one
/// materialised at their next login (Phase 3).
pub async fn ids_without_subscription(pool: &PgPool) -> Result<Vec<Uuid>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT u.id FROM users u
           LEFT JOIN subscriptions s ON s.user_id = u.id
          WHERE s.user_id IS NULL",
    )
    .fetch_all(pool)
    .await
}

/// A macro rather than a `const` because sqlx only accepts `&'static str`
/// queries; `concat!` keeps the composed SQL a compile-time literal.
macro_rules! columns {
    () => {
        "id, steam_id, persona_name, avatar_url, profile_url, last_login_at, \
         is_admin, status, created_at, updated_at"
    };
}

/// Find or create the account for a proven SteamID64, and stamp the login.
pub async fn upsert_by_steam_id(
    tx: &mut Transaction<'_, Postgres>,
    steam_id: i64,
) -> Result<User, sqlx::Error> {
    sqlx::query_as::<_, User>(concat!(
        "INSERT INTO users (steam_id, last_login_at)
         VALUES ($1, now())
         ON CONFLICT (steam_id) DO UPDATE SET last_login_at = now()
         RETURNING ",
        columns!()
    ))
    .bind(steam_id)
    .fetch_one(&mut **tx)
    .await
}

/// Refresh the Steam display fields. `None` leaves the stored value alone.
pub async fn update_profile(
    pool: &PgPool,
    id: Uuid,
    update: &SteamProfileUpdate,
) -> Result<User, sqlx::Error> {
    sqlx::query_as::<_, User>(concat!(
        "UPDATE users
            SET persona_name = COALESCE($2, persona_name),
                avatar_url   = COALESCE($3, avatar_url),
                profile_url  = COALESCE($4, profile_url)
          WHERE id = $1
        RETURNING ",
        columns!()
    ))
    .bind(id)
    .bind(update.persona_name.as_deref())
    .bind(update.avatar_url.as_deref())
    .bind(update.profile_url.as_deref())
    .fetch_one(pool)
    .await
}

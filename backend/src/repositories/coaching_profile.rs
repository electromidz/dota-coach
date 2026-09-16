use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::coaching_profile::{CoachingProfile, StoredProfile};
use crate::domain::role::CoachableRole;

/// The player's current coaching profile, if they have chosen a role.
///
/// `None` also covers a stored role this build no longer recognises — see
/// [`StoredProfile::hydrate`]. Both mean the same thing to a caller: there is
/// no role scope yet, so ask for one.
pub async fn find(
    pool: &PgPool,
    dota_player_id: Uuid,
) -> Result<Option<CoachingProfile>, sqlx::Error> {
    let stored = sqlx::query_as::<_, StoredProfile>(
        "SELECT id, selected_role, recommended_role, analyzed_matches, selected_at, updated_at
           FROM coaching_profiles
          WHERE dota_player_id = $1",
    )
    .bind(dota_player_id)
    .fetch_optional(pool)
    .await?;

    Ok(stored.and_then(StoredProfile::hydrate))
}

/// Record the player's choice, replacing whatever it was before.
///
/// `selected_at` only moves when the role actually changes. Re-confirming the
/// same role is not the start of a new stretch of work, and treating it as one
/// would reset the baseline that progress is eventually measured from.
pub async fn upsert(
    pool: &PgPool,
    dota_player_id: Uuid,
    selected: CoachableRole,
    recommended: Option<CoachableRole>,
    analyzed_matches: i64,
) -> Result<CoachingProfile, sqlx::Error> {
    let stored = sqlx::query_as::<_, StoredProfile>(
        "INSERT INTO coaching_profiles
             (dota_player_id, selected_role, recommended_role, analyzed_matches)
         VALUES ($1, $2, $3, $4)
         ON CONFLICT (dota_player_id) DO UPDATE SET
             selected_role    = EXCLUDED.selected_role,
             recommended_role = EXCLUDED.recommended_role,
             analyzed_matches = EXCLUDED.analyzed_matches,
             selected_at      = CASE
                 WHEN coaching_profiles.selected_role IS DISTINCT FROM EXCLUDED.selected_role
                 THEN now()
                 ELSE coaching_profiles.selected_at
             END
         RETURNING id, selected_role, recommended_role, analyzed_matches, selected_at, updated_at",
    )
    .bind(dota_player_id)
    .bind(selected.slug())
    .bind(recommended.map(CoachableRole::slug))
    .bind(analyzed_matches.clamp(0, i32::MAX as i64) as i32)
    .fetch_one(pool)
    .await?;

    // The row was just written from a known role, so this cannot fail; the
    // alternative would be a second, unreachable error variant.
    Ok(stored
        .hydrate()
        .expect("a profile written from a CoachableRole reads back as one"))
}

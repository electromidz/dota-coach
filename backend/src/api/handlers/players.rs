//! The signed-in player's Dota profile and match synchronization.
//!
//! Every handler here derives the player from the session. No route accepts a
//! user id, player id or Steam id from the caller.

use axum::extract::State;
use axum::Json;
use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::api::extract::CurrentUser;
use crate::domain::player::DotaPlayer;
use crate::domain::user::User;
use crate::error::{AppError, AppResult};
use crate::repositories;
use crate::services::sync::{sync_player, SyncReport};
use crate::state::AppState;

#[derive(Serialize)]
pub struct MeResponse {
    /// Steam account: persona, avatar, profile URL.
    pub user: User,
    /// Linked Dota identity: account id, rank, last sync.
    pub dota_player: DotaPlayer,
    pub matches_stored: i64,
}

/// `GET /api/players/me`
///
/// A pure read. Profile refresh happens on sync, so this never blocks on the
/// Dota provider and never fails because the provider is down.
pub async fn me(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> AppResult<Json<MeResponse>> {
    let dota_player = load_linked_player(&state, &user).await?;
    let matches_stored = repositories::r#match::count_by_player(&state.db, dota_player.id).await?;

    Ok(Json(MeResponse {
        user,
        dota_player,
        matches_stored,
    }))
}

#[derive(Serialize)]
pub struct SyncResponse {
    pub dota_player: DotaPlayer,
    pub sync: SyncReport,
}

/// `POST /api/players/me/sync`
///
/// Rate limited per player: syncing hits a third-party API, and Dota match
/// history does not change second to second.
pub async fn sync(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> AppResult<Json<SyncResponse>> {
    let dota_player = load_linked_player(&state, &user).await?;
    enforce_sync_cooldown(&state, &dota_player)?;

    let (report, dota_player) = sync_player(
        &state.db,
        state.dota.as_ref(),
        user.id,
        &dota_player,
        state.config.dota.sync_match_limit,
    )
    .await?;

    Ok(Json(SyncResponse {
        dota_player,
        sync: report,
    }))
}

/// The Dota identity is created during login, so a missing row means the link
/// step failed — a distinct, actionable state rather than a generic 404.
async fn load_linked_player(state: &AppState, user: &User) -> AppResult<DotaPlayer> {
    repositories::dota_player::find_by_user_id(&state.db, user.id)
        .await?
        .ok_or(AppError::DotaAccountNotLinked)
}

fn enforce_sync_cooldown(state: &AppState, player: &DotaPlayer) -> AppResult<()> {
    match cooldown_remaining(
        state.config.dota.sync_cooldown_seconds,
        player.last_synced_at,
        Utc::now(),
    ) {
        None => Ok(()),
        Some(wait) => Err(AppError::TooManyRequests(format!(
            "Already synced recently. Try again in {wait}s."
        ))),
    }
}

/// Seconds the caller must still wait, or `None` if a sync is allowed now.
fn cooldown_remaining(
    cooldown_seconds: i64,
    last_synced_at: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
) -> Option<i64> {
    if cooldown_seconds <= 0 {
        return None;
    }

    // A clock that moved backwards must not lock the player out forever.
    let elapsed = (now - last_synced_at?).num_seconds().max(0);
    (elapsed < cooldown_seconds).then_some(cooldown_seconds - elapsed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn a_player_who_never_synced_is_never_throttled() {
        assert_eq!(cooldown_remaining(30, None, Utc::now()), None);
    }

    #[test]
    fn a_recent_sync_reports_the_remaining_wait() {
        let now = Utc::now();
        let last = now - Duration::seconds(10);
        assert_eq!(cooldown_remaining(30, Some(last), now), Some(20));
    }

    #[test]
    fn the_cooldown_expires() {
        let now = Utc::now();
        let last = now - Duration::seconds(31);
        assert_eq!(cooldown_remaining(30, Some(last), now), None);
    }

    #[test]
    fn a_zero_cooldown_disables_throttling() {
        let now = Utc::now();
        assert_eq!(cooldown_remaining(0, Some(now), now), None);
    }

    #[test]
    fn a_future_timestamp_does_not_lock_the_player_out() {
        let now = Utc::now();
        let last = now + Duration::seconds(3_600);
        assert_eq!(cooldown_remaining(30, Some(last), now), Some(30));
    }
}

//! The signed-in player's Dota profile and match synchronization.
//!
//! Every handler here derives the player from the session. No route accepts a
//! user id, player id or Steam id from the caller.

use axum::extract::State;
use axum::Json;
use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::api::extract::CurrentUser;
use crate::api::handlers::coach;
use crate::domain::player::DotaPlayer;
use crate::domain::user::User;
use crate::error::{AppError, AppResult};
use crate::repositories;
use crate::services::coaching_session;
use crate::services::sync::{sync_player, SyncReport};
use crate::state::AppState;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
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
#[utoipa::path(
    get, path = "/api/players/me", tag = "players",
    summary = "The linked Dota player",
    security(("session" = [])),
    responses(
        (status = 200, description = "Profile and how much history is stored", body = MeResponse),
        (status = 409, description = "No Dota account linked to this user yet", body = crate::error::ErrorBody),
        (status = 401, description = "No session cookie, or it has expired", body = crate::error::ErrorBody),
        (status = 500, description = "Database or internal failure", body = crate::error::ErrorBody),
    )
)]
pub async fn me(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> AppResult<Json<MeResponse>> {
    let dota_player = load_linked_player(&state, &user).await?;
    // Every stored match, whatever mode, hero or result — this is the "you have
    // N matches" figure, not a view of them.
    let matches_stored = repositories::r#match::count_by_player(
        &state.db,
        dota_player.id,
        &repositories::r#match::MatchFilter::default(),
    )
    .await?;

    Ok(Json(MeResponse {
        user,
        dota_player,
        matches_stored,
    }))
}

#[derive(Serialize, ToSchema)]
pub struct SyncResponse {
    pub dota_player: DotaPlayer,
    pub sync: SyncReport,
}

/// `POST /api/players/me/sync`
///
/// Rate limited per player: syncing hits a third-party API, and Dota match
/// history does not change second to second.
#[utoipa::path(
    post, path = "/api/players/me/sync", tag = "players",
    summary = "Pull new matches from the Dota provider",
    description = "Rate limited per player by `SYNC_COOLDOWN_SECONDS`. Metrics are recomputed for whatever it fetches, so a successful sync changes every number the rest of the API reports.",
    security(("session" = [])),
    responses(
        (status = 200, description = "What was fetched and stored", body = SyncResponse),
        (status = 429, description = "Called again inside the cooldown", body = crate::error::ErrorBody),
        (status = 502, description = "The Dota provider failed or rate limited us", body = crate::error::ErrorBody),
        (status = 409, description = "No Dota account linked to this user yet", body = crate::error::ErrorBody),
        (status = 401, description = "No session cookie, or it has expired", body = crate::error::ErrorBody),
        (status = 500, description = "Database or internal failure", body = crate::error::ErrorBody),
    )
)]
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
        state.config.roles.analysis_match_limit,
    )
    .await?;

    checkpoint_after_sync(&state, &user, &dota_player).await;

    Ok(Json(SyncResponse {
        dota_player,
        sync: report,
    }))
}

/// Record where the player stands, now that new matches have landed.
///
/// This lives here rather than inside `sync_player` because a snapshot needs
/// the benchmark provider, and the sync service deliberately knows nothing
/// about providers beyond Dota or about configuration at all. The handler is
/// where those are already in scope.
///
/// **Never fatal.** A player must not lose their sync because a snapshot could
/// not be written — the matches are stored either way, and the next sync will
/// checkpoint instead. Same contract as `player_model::refresh` inside the sync
/// service, which is also logged rather than propagated.
///
/// Silent by design when the player has not chosen a role: sessions are
/// per-role, so there is nothing to snapshot yet.
async fn checkpoint_after_sync(state: &AppState, user: &User, player: &DotaPlayer) {
    let scope = match coach::scope_for_checkpoint(state, player).await {
        Ok(Some(scope)) => scope,
        // No role chosen. Not a problem, and not worth a log line on every
        // sync a new player runs.
        Ok(None) => return,
        Err(e) => {
            tracing::warn!(error = %e, "coaching session skipped: could not resolve the role");
            return;
        }
    };

    match coach::checkpoint(
        state,
        user,
        player,
        &scope,
        coaching_session::MIN_NEW_MATCHES,
    )
    .await
    {
        Ok(_) => {}
        Err(e) => tracing::warn!(error = %e, "coaching session skipped after sync"),
    }
}

/// The Dota identity is created during login, so a missing row means the link
/// step failed — a distinct, actionable state rather than a generic 404.
async fn load_linked_player(state: &AppState, user: &User) -> AppResult<DotaPlayer> {
    repositories::dota_player::find_by_user_id(&state.db, user.id)
        .await?
        .ok_or(AppError::DotaAccountNotLinked)
}

fn enforce_sync_cooldown(state: &AppState, player: &DotaPlayer) -> AppResult<()> {
    match cooldown_remaining_for(state, player) {
        None => Ok(()),
        Some(wait) => Err(AppError::TooManyRequests(format!(
            "Already synced recently. Try again in {wait}s."
        ))),
    }
}

/// Seconds this player must still wait before another sync, or `None` if one is
/// allowed now.
///
/// Shared with the match list, which refreshes stale history behind a read: one
/// cooldown for every path that calls the provider, so a second entry point
/// cannot quietly double the request rate the configuration allows.
pub fn cooldown_remaining_for(state: &AppState, player: &DotaPlayer) -> Option<i64> {
    cooldown_remaining(
        state.config.dota.sync_cooldown_seconds,
        player.last_synced_at,
        Utc::now(),
    )
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

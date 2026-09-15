//! Aggregated player analytics.
//!
//! Every number here is computed in SQL over stored metrics — the client does
//! no arithmetic, and neither does the LLM.

use axum::extract::State;
use axum::Json;
use serde::Serialize;

use crate::api::extract::CurrentUser;
use crate::domain::metrics::{HeroStats, PlayerStats, RoleStats};
use crate::domain::player::DotaPlayer;
use crate::domain::user::User;
use crate::error::{AppError, AppResult};
use crate::repositories;
use crate::services::metrics::METRICS_VERSION;
use crate::state::AppState;
use utoipa::ToSchema;

/// Heroes returned by `/api/stats`. The full list lives on the heroes page.
const TOP_HEROES: i64 = 8;

#[derive(Serialize, ToSchema)]
pub struct StatsResponse {
    pub overall: PlayerStats,
    pub heroes: Vec<HeroStats>,
    pub roles: Vec<RoleStats>,
    /// Which formula set produced these numbers.
    pub metrics_version: i32,
}

/// `GET /api/stats`
#[utoipa::path(
    get, path = "/api/stats", tag = "stats",
    summary = "Aggregated performance",
    description = "Computed in SQL over stored metrics. No model is involved and no arithmetic happens on the client.",
    security(("session" = [])),
    responses(
        (status = 200, description = "Overall, per-hero and per-role aggregates", body = StatsResponse),
        (status = 409, description = "No Dota account linked to this user yet", body = crate::error::ErrorBody),
        (status = 401, description = "No session cookie, or it has expired", body = crate::error::ErrorBody),
        (status = 500, description = "Database or internal failure", body = crate::error::ErrorBody),
    )
)]
pub async fn get(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> AppResult<Json<StatsResponse>> {
    let player = load_linked_player(&state, &user).await?;

    Ok(Json(StatsResponse {
        overall: repositories::metrics::player_stats(&state.db, player.id).await?,
        heroes: repositories::metrics::hero_stats(&state.db, player.id, TOP_HEROES).await?,
        roles: repositories::metrics::role_stats(&state.db, player.id).await?,
        metrics_version: METRICS_VERSION,
    }))
}

async fn load_linked_player(state: &AppState, user: &User) -> AppResult<DotaPlayer> {
    repositories::dota_player::find_by_user_id(&state.db, user.id)
        .await?
        .ok_or(AppError::DotaAccountNotLinked)
}

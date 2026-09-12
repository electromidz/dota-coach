use axum::extract::State;
use axum::Json;
use serde::Serialize;

use crate::error::AppResult;
use crate::state::AppState;

#[derive(Serialize)]
pub struct HealthResponse {
    status: &'static str,
    version: &'static str,
    database: DependencyStatus,
    llm_configured: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyStatus {
    Up,
    Down,
}

/// Liveness probe. Never touches external services.
pub async fn liveness() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok" }))
}

/// Readiness probe: verifies the database is actually reachable.
pub async fn health(State(state): State<AppState>) -> AppResult<Json<HealthResponse>> {
    let database = match sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&state.db)
        .await
    {
        Ok(_) => DependencyStatus::Up,
        Err(e) => {
            tracing::warn!(error = %e, "health check: database unreachable");
            DependencyStatus::Down
        }
    };

    Ok(Json(HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
        database,
        llm_configured: state.config.llm.is_configured(),
    }))
}

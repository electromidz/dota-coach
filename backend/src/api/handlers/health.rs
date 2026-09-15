use axum::extract::State;
use axum::Json;
use serde::Serialize;

use crate::error::AppResult;
use crate::state::AppState;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
pub struct HealthResponse {
    status: &'static str,
    version: &'static str,
    database: DependencyStatus,
    llm_configured: bool,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum DependencyStatus {
    Up,
    Down,
}

/// Liveness probe. Never touches external services.
#[utoipa::path(
    get, path = "/health/live", tag = "health",
    summary = "Liveness probe",
    description = "Answers as long as the process is running. Touches nothing else, so it stays green during a database outage — use `/health` to learn whether the service can actually serve traffic.",
    responses((status = 200, description = "Process is alive", body = serde_json::Value))
)]
pub async fn liveness() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok" }))
}

/// Readiness probe: verifies the database is actually reachable.
#[utoipa::path(
    get, path = "/health", tag = "health",
    summary = "Readiness probe",
    description = "Verifies the database is reachable and reports whether a coaching model is configured. Also mounted at `/api/health`, the only route under `/api` that needs no session.",
    responses(
        (status = 200, description = "Dependencies checked; read `database` for the verdict", body = HealthResponse),
        (status = 500, description = "Probe itself failed", body = crate::error::ErrorBody),
    )
)]
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

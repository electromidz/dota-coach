use std::time::Duration;

use axum::http::{header, HeaderValue, Method, StatusCode};
use axum::routing::{get, post};
use axum::Router;
use tower_http::cors::CorsLayer;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;

use crate::api::handlers::{auth, benchmark, health, matches, players, stats};
use crate::config::Config;
use crate::error::AppError;
use crate::state::AppState;

/// Single place where every route is mounted.
///
/// Everything under `/api` except `/api/health` requires a session; the
/// requirement is expressed by the `CurrentUser` extractor in each handler
/// rather than by a layer, so a route cannot silently lose it.
///
/// Phase 4 adds `POST /api/matches/:id/analyze` and the coaching endpoints.
pub fn build(state: AppState, config: &Config) -> Router {
    let api = Router::new()
        .route("/health", get(health::health))
        // Spec-mandated auth surface. `steam` starts the flow, `callback`
        // completes it; both are browser navigations, not fetch targets.
        .route("/auth/steam", get(auth::login))
        .route("/auth/steam/callback", get(auth::callback))
        .route("/auth/me", get(auth::me))
        .route("/auth/logout", post(auth::logout))
        .route("/players/me", get(players::me))
        .route("/players/me/sync", post(players::sync))
        .route("/stats", get(stats::get))
        .route("/benchmark", get(benchmark::overview))
        .route("/benchmark/{metric}", get(benchmark::metric))
        .route("/matches", get(matches::list))
        .route("/matches/{id}", get(matches::get));

    Router::new()
        .route("/health", get(health::health))
        .route("/health/live", get(health::liveness))
        .nest("/api", api)
        // Unknown paths answer with the same envelope as everything else.
        .fallback(not_found)
        .layer(TraceLayer::new_for_http())
        // Upstream Dota/LLM calls must not hold a request open indefinitely;
        // 504 is the honest answer.
        .layer(TimeoutLayer::with_status_code(
            StatusCode::GATEWAY_TIMEOUT,
            Duration::from_secs(30),
        ))
        .layer(cors_layer(config))
        .with_state(state)
}

async fn not_found() -> AppError {
    AppError::NotFound("That endpoint does not exist.".into())
}

/// The frontend sends the session cookie cross-origin, which requires
/// `allow_credentials`. That in turn forbids a wildcard origin, so the
/// allowlist is always explicit.
fn cors_layer(config: &Config) -> CorsLayer {
    let origins: Vec<HeaderValue> = config
        .cors_origins
        .iter()
        .filter_map(|o| o.parse::<HeaderValue>().ok())
        .collect();

    CorsLayer::new()
        .allow_origin(origins)
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([header::CONTENT_TYPE])
        .allow_credentials(true)
}

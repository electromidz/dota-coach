use std::time::Duration;

use axum::http::{header, HeaderValue, Method, StatusCode};
use axum::routing::get;
use axum::Router;
use tower_http::cors::CorsLayer;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;

use crate::api::handlers::health;
use crate::config::Config;
use crate::state::AppState;

/// Single place where every route is mounted.
///
/// Phase 2+ adds `/api/players` and `/api/matches` sub-routers here.
pub fn build(state: AppState, config: &Config) -> Router {
    let api = Router::new().route("/health", get(health::health));

    Router::new()
        .route("/health", get(health::health))
        .route("/health/live", get(health::liveness))
        .nest("/api", api)
        .layer(TraceLayer::new_for_http())
        // Upstream Dota/LLM calls in later phases must not hold a request open
        // indefinitely; 504 is the honest answer.
        .layer(TimeoutLayer::with_status_code(
            StatusCode::GATEWAY_TIMEOUT,
            Duration::from_secs(30),
        ))
        .layer(cors_layer(config))
        .with_state(state)
}

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
}

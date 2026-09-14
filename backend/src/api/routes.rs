use std::time::Duration;

use axum::http::{header, HeaderValue, Method, StatusCode};
use axum::middleware;
use axum::routing::{get, post};
use axum::Router;
use tower_http::cors::CorsLayer;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;

use crate::api::handlers::{
    auth, benchmark, billing, coach, health, heroes, matches, players, stats,
};
use crate::api::observability;
use crate::config::Config;
use crate::error::AppError;
use crate::state::AppState;

/// Single place where every route is mounted.
///
/// Everything under `/api` except `/api/health` requires a session; the
/// requirement is expressed by the `CurrentUser` extractor in each handler
/// rather than by a layer, so a route cannot silently lose it.
///
/// Generation endpoints answer `503 FEATURE_UNAVAILABLE` when no model is
/// configured, and `429` when a player is over their budget; every read path
/// keeps working either way.
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
        // Hero Intelligence. `/heroes` is local-only by design, so it keeps
        // answering while the meta provider is down.
        .route("/heroes", get(heroes::pool))
        .route("/heroes/recommendations", get(heroes::recommendations))
        .route("/hero-intelligence", get(heroes::intelligence))
        // Coaching. Reading is free and always answers; generating is the
        // only rate-limited verb in the API.
        .route("/coach", get(coach::get))
        .route("/coach/analyze", post(coach::analyze))
        .route("/coach/player-model", get(coach::player_model))
        .route("/coach/training-focus", get(coach::training_focus))
        // Billing. Reading is always allowed — an expired account still needs
        // to see why it is expired and how to fix it.
        .route("/billing", get(billing::overview))
        // The offer itself, for the signed-out landing page. No session, and
        // nothing on it that is not public pricing copy.
        .route("/billing/plan", get(billing::plan))
        .route("/billing/subscription", get(billing::subscription))
        .route("/billing/payments", get(billing::payments))
        .route("/billing/checkout", post(billing::checkout))
        // The only unauthenticated write in the API. It is safe because it
        // believes nothing that is not signed by the payment provider.
        .route("/billing/webhook", post(billing::webhook))
        .route("/matches", get(matches::list))
        .route("/matches/{id}", get(matches::get))
        .route("/matches/{id}/analysis", get(coach::match_analysis))
        .route("/matches/{id}/analyze", post(coach::analyze_match));

    Router::new()
        .route("/health", get(health::health))
        .route("/health/live", get(health::liveness))
        .nest("/api", api)
        // Unknown paths answer with the same envelope as everything else.
        .fallback(not_found)
        // Outermost, so the request id is on every log line the request
        // produces — including the timeout layer's and the handler's.
        .layer(middleware::from_fn(observability::request_id))
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
        // So a user reporting a failure can quote the id the server logged
        // against it; without this the browser hides the header.
        .expose_headers([observability::REQUEST_ID_HEADER])
        .allow_credentials(true)
}

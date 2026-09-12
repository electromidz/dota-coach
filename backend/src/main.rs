mod api;
mod config;
mod db;
mod domain;
mod error;
mod repositories;
mod services;
mod state;

use std::process::ExitCode;

use tokio::net::TcpListener;
use tokio::signal;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use crate::config::Config;
use crate::state::AppState;

#[tokio::main]
async fn main() -> ExitCode {
    // `.env` is a developer convenience; in Docker the values come from the
    // compose environment and this call is a no-op.
    let _ = dotenvy::dotenv();
    init_tracing();

    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            tracing::error!("startup failed: {e}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env()?;
    let addr = config.bind_address();

    let pool = db::connect(&config).await?;
    db::run_migrations(&pool).await?;
    tracing::info!("database connected, migrations applied");

    if !config.llm.is_configured() {
        tracing::warn!("LLM_API_KEY not set - AI analysis will be unavailable");
    }

    let state = AppState::new(pool, config.clone());
    let app = api::routes::build(state, &config);

    let listener = TcpListener::bind(&addr).await?;
    tracing::info!("listening on http://{addr}");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

fn init_tracing() {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            "dota_coach_backend=info,tower_http=info,axum::rejection=trace".into()
        }))
        .with(tracing_subscriber::fmt::layer())
        .init();
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c().await.expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("shutdown signal received");
}

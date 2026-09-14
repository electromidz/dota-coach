use std::process::ExitCode;
use std::sync::Arc;

use tokio::net::TcpListener;
use tokio::signal;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use dota_coach_backend::config::Config;
use dota_coach_backend::services::auth::steam_openid::{self, SteamOpenId};
use dota_coach_backend::services::benchmarks::opendota::OpenDotaBenchmarkProvider;
use dota_coach_backend::services::dota::opendota::OpenDotaProvider;
use dota_coach_backend::services::hero_meta::opendota::OpenDotaHeroMetaProvider;
use dota_coach_backend::services::llm::openai::OpenAiLlmProvider;
use dota_coach_backend::state::{AppState, Providers};
use dota_coach_backend::{api, db, repositories};

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

    if !config.auth.cookie_secure && !config.auth.public_base_url.starts_with("http://localhost") {
        tracing::warn!(
            public_base_url = %config.auth.public_base_url,
            "COOKIE_SECURE is false outside localhost - session cookies will be sent over plain HTTP"
        );
    }

    match repositories::session::delete_expired(&pool).await {
        Ok(0) => {}
        Ok(n) => tracing::info!(removed = n, "expired sessions swept"),
        Err(e) => tracing::warn!(error = %e, "session sweep failed"),
    }

    // Concrete providers are chosen exactly once, here; everything downstream
    // sees trait objects.
    let dota = Arc::new(OpenDotaProvider::new(&config.dota)?);
    tracing::info!(base_url = %config.dota.base_url, "dota provider ready");

    let steam = Arc::new(SteamOpenId::new(reqwest::Client::new(), &config.auth));
    tracing::info!(
        openid_url = %config.auth.steam_openid_url,
        return_to = %format!("{}{}", config.auth.public_base_url, steam_openid::CALLBACK_PATH),
        "steam openid ready"
    );

    let benchmarks = OpenDotaBenchmarkProvider::new(
        reqwest::Client::new(),
        &config.dota.base_url,
        config.dota.api_key.clone(),
        pool.clone(),
        config.dota.benchmark_ttl_hours,
    );

    // Hero meta shares OpenDota's base URL and key, but not its client-side
    // limits: it is one document for the whole roster, cached for a day.
    let hero_meta = OpenDotaHeroMetaProvider::new(
        reqwest::Client::new(),
        &config.dota.base_url,
        config.dota.api_key.clone(),
        pool.clone(),
        config.heroes.meta_ttl_hours,
        config.heroes.meta_weights,
    );

    let llm = OpenAiLlmProvider::new(
        &config.llm,
        std::time::Duration::from_secs(config.coach.request_timeout_seconds),
    )?;

    let state = AppState::new(
        pool,
        config.clone(),
        Providers {
            dota,
            steam: steam.clone(),
            steam_verifier: steam,
            benchmarks,
            hero_meta,
            llm,
        },
    );
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
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
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

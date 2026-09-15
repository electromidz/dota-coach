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
use dota_coach_backend::services::payments::nowpayments::NowPaymentsProvider;
use dota_coach_backend::services::payments::{PaymentProvider, UnconfiguredPaymentProvider};
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

    warn_about_deployment(&config);

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

    // Billing is optional: without credentials the product runs in full, and
    // only checkout is unavailable. Choosing the null provider here rather than
    // branching later keeps every caller free of the question.
    let payments: Arc<dyn PaymentProvider> = if config.billing.is_configured() {
        NowPaymentsProvider::new(
            &config.billing,
            std::time::Duration::from_secs(config.billing.request_timeout_seconds),
        )?
    } else {
        tracing::warn!(
            "NOWPAYMENTS_API_KEY/NOWPAYMENTS_IPN_SECRET not set - checkout will be unavailable"
        );
        Arc::new(UnconfiguredPaymentProvider)
    };

    if config.billing.enforce {
        tracing::info!(
            price_cents = config.billing.price_cents,
            currency = %config.billing.currency,
            trial_days = config.billing.trial_days,
            "billing entitlements are enforced"
        );
    } else {
        tracing::warn!("BILLING_ENFORCE is off - premium features are open to every account");
    }

    if config.docs_enabled {
        tracing::info!(
            docs = api::docs::UI_PATH,
            spec = api::docs::SPEC_PATH,
            "api documentation mounted"
        );
    }

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
            payments,
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

/// Say out loud, once at boot, what is wrong with this deployment.
///
/// None of these are fatal — a misconfigured origin should not stop a service
/// that is otherwise fine from starting — but every one of them is something an
/// operator would rather read in the first ten log lines than discover from a
/// user whose login silently fails.
fn warn_about_deployment(config: &Config) {
    let local = |url: &str| url.starts_with("http://localhost") || url.starts_with("http://127.");

    if !config.auth.cookie_secure && !local(&config.auth.public_base_url) {
        tracing::warn!(
            public_base_url = %config.auth.public_base_url,
            "COOKIE_SECURE is false outside localhost - session cookies will be sent over plain HTTP"
        );
    }

    // The inverse mistake, and a worse one to debug: `Secure` cookies are
    // silently dropped by the browser over plain HTTP, so login appears to
    // succeed and every subsequent request is anonymous.
    if config.auth.cookie_secure && config.auth.public_base_url.starts_with("http://") {
        tracing::warn!(
            public_base_url = %config.auth.public_base_url,
            "COOKIE_SECURE is true but the public base URL is plain HTTP - the browser will discard the session cookie"
        );
    }

    // Not an error — an operator may well want this — but it publishes a full
    // map of the API to anyone who guesses the path, so it should never be a
    // surprise found later.
    if config.docs_enabled && !local(&config.auth.public_base_url) {
        tracing::warn!(
            docs = crate::api::docs::UI_PATH,
            spec = crate::api::docs::SPEC_PATH,
            "DOCS_ENABLED is true outside localhost - the API documentation is publicly reachable"
        );
    }

    if config.cors_origins.is_empty() {
        tracing::warn!("CORS_ORIGINS is empty - no browser origin may call this API");
    } else if !config.cors_origins.contains(&config.auth.frontend_base_url) {
        tracing::warn!(
            frontend_base_url = %config.auth.frontend_base_url,
            "FRONTEND_BASE_URL is not in CORS_ORIGINS - the frontend it redirects to cannot call the API"
        );
    }
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

use std::time::Duration;

use sqlx::postgres::{PgPool, PgPoolOptions};

use crate::config::Config;

/// Build the shared connection pool. Called once at startup.
///
/// Sized from config rather than hard-coded. Against a remote database every
/// query costs a network round trip, and a page that fires several requests at
/// once can hold every connection for seconds. With a tight acquire timeout
/// the requests at the back of that queue failed with a generic database
/// error even though nothing was wrong except the wait. Keeping a couple of
/// connections warm also spares each burst a fresh TLS handshake.
pub async fn connect(config: &Config) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(config.db.max_connections)
        .min_connections(config.db.min_connections.min(config.db.max_connections))
        .acquire_timeout(Duration::from_secs(config.db.acquire_timeout_seconds))
        .connect(&config.database_url)
        .await
}

/// Apply every pending migration in `backend/migrations`.
pub async fn run_migrations(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("./migrations").run(pool).await
}

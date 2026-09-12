use std::sync::Arc;

use sqlx::PgPool;

use crate::config::Config;

/// Shared, cheaply-cloneable application state handed to every handler.
///
/// Later phases add provider trait objects here (`Arc<dyn DotaDataProvider>`,
/// `Arc<dyn LlmProvider>`) so handlers depend on abstractions, not concrete
/// clients.
#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub config: Arc<Config>,
}

impl AppState {
    pub fn new(db: PgPool, config: Config) -> Self {
        Self {
            db,
            config: Arc::new(config),
        }
    }
}

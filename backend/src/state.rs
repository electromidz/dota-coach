use std::sync::Arc;

use sqlx::PgPool;

use crate::config::Config;
use crate::services::auth::steam_openid::{SteamOpenId, SteamVerifier};
use crate::services::dota::DotaDataProvider;

/// Shared, cheaply-cloneable application state handed to every handler.
///
/// Handlers depend on abstractions — never on OpenDota or on Valve's OpenID
/// endpoint directly — so both can be stubbed in tests and swapped in
/// production. Phase 4 adds `Arc<dyn LlmProvider>` alongside them.
#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub config: Arc<Config>,
    pub dota: Arc<dyn DotaDataProvider>,
    /// Builds Steam login URLs and verifies the assertions that come back.
    pub steam: Arc<SteamOpenId>,
    /// Verification behind a trait object so tests never call Valve.
    pub steam_verifier: Arc<dyn SteamVerifier>,
}

impl AppState {
    pub fn new(
        db: PgPool,
        config: Config,
        dota: Arc<dyn DotaDataProvider>,
        steam: Arc<SteamOpenId>,
        steam_verifier: Arc<dyn SteamVerifier>,
    ) -> Self {
        Self {
            db,
            config: Arc::new(config),
            dota,
            steam,
            steam_verifier,
        }
    }
}

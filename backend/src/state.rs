use std::sync::Arc;

use sqlx::PgPool;

use crate::config::Config;
use crate::services::auth::steam_openid::{SteamOpenId, SteamVerifier};
use crate::services::benchmarks::BenchmarkProvider;
use crate::services::cache::postgres::PostgresCoachingCache;
use crate::services::cache::{CoachingCache, NoCache};
use crate::services::dota::DotaDataProvider;
use crate::services::hero_meta::HeroMetaProvider;
use crate::services::llm::LlmProvider;
use crate::services::payments::PaymentProvider;
use crate::services::voucher::RateLimiter;

/// Shared, cheaply-cloneable application state handed to every handler.
///
/// Handlers depend on abstractions — never on OpenDota, Valve's OpenID
/// endpoint or an LLM vendor directly — so every one of them can be stubbed in
/// tests and swapped in production without a handler changing.
#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub config: Arc<Config>,
    pub dota: Arc<dyn DotaDataProvider>,
    /// Builds Steam login URLs and verifies the assertions that come back.
    pub steam: Arc<SteamOpenId>,
    /// Verification behind a trait object so tests never call Valve.
    pub steam_verifier: Arc<dyn SteamVerifier>,
    /// Peer distributions. Swappable for STRATZ without touching the engine.
    pub benchmarks: Arc<dyn BenchmarkProvider>,
    /// What the wider player base is doing. Degradable: Hero Intelligence
    /// still answers from the player's own history when this is down.
    pub hero_meta: Arc<dyn HeroMetaProvider>,
    /// Interpretation only. Every number it is shown was computed here first,
    /// and everything it returns is validated against that evidence.
    pub llm: Arc<dyn LlmProvider>,
    /// Takes money and signs for it. Never decides what a payment *means* —
    /// that stays in `services::billing`.
    pub payments: Arc<dyn PaymentProvider>,
    /// Per-user abuse guard for `POST /api/subscribe/redeem`. In-memory, not
    /// a provider: nothing external to swap, so no trait.
    pub redeem_rate_limiter: Arc<RateLimiter>,
    /// Read-through cache for the coaching context.
    ///
    /// Chosen from configuration rather than injected like the providers,
    /// because there is nothing external to swap yet — `COACH_CACHE_ENABLED`
    /// picks between the Postgres implementation and a no-op. The trait is
    /// what a Redis implementation would land behind if a second instance
    /// ever existed.
    pub cache: Arc<dyn CoachingCache>,
}

/// Every external dependency, chosen once at startup.
///
/// Grouped rather than passed one by one: the list grows with each phase, and
/// a positional call of eight trait objects is a swap waiting to happen.
pub struct Providers {
    pub dota: Arc<dyn DotaDataProvider>,
    pub steam: Arc<SteamOpenId>,
    pub steam_verifier: Arc<dyn SteamVerifier>,
    pub benchmarks: Arc<dyn BenchmarkProvider>,
    pub hero_meta: Arc<dyn HeroMetaProvider>,
    pub llm: Arc<dyn LlmProvider>,
    pub payments: Arc<dyn PaymentProvider>,
}

impl AppState {
    pub fn new(db: PgPool, config: Config, providers: Providers) -> Self {
        // Read before `config` is moved into the Arc below.
        let config_cache_enabled = config.coach.cache_enabled;
        let cache_ttl = config.coach.cache_ttl_minutes;
        let db_for_cache = db.clone();

        Self {
            db,
            config: Arc::new(config),
            dota: providers.dota,
            steam: providers.steam,
            steam_verifier: providers.steam_verifier,
            benchmarks: providers.benchmarks,
            hero_meta: providers.hero_meta,
            llm: providers.llm,
            payments: providers.payments,
            redeem_rate_limiter: Arc::new(RateLimiter::new()),
            cache: if config_cache_enabled {
                PostgresCoachingCache::new(db_for_cache, cache_ttl)
            } else {
                Arc::new(NoCache)
            },
        }
    }
}

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use sqlx::PgPool;
use uuid::Uuid;

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
    /// Which players already have a background sync running.
    ///
    /// In-memory and not a provider, for the same reason `redeem_rate_limiter`
    /// is not: there is nothing external to swap. It keeps a player with several
    /// tabs open from firing several syncs at the same provider; the cooldown in
    /// `players::cooldown_remaining_for` is what bounds the rate over time.
    pub sync_in_flight: Arc<SyncGuard>,
    /// Read-through cache for the coaching context.
    ///
    /// Chosen from configuration rather than injected like the providers,
    /// because there is nothing external to swap yet — `COACH_CACHE_ENABLED`
    /// picks between the Postgres implementation and a no-op. The trait is
    /// what a Redis implementation would land behind if a second instance
    /// ever existed.
    pub cache: Arc<dyn CoachingCache>,
}

/// Who is already syncing, so the same work is not started twice.
///
/// A set behind a `std::sync::Mutex` rather than an async one on purpose: the
/// lock is held for a set insert and nothing else, and it must never be held
/// across an `await` — the sync itself runs outside it.
#[derive(Debug, Default)]
pub struct SyncGuard {
    players: Mutex<HashSet<Uuid>>,
}

impl SyncGuard {
    /// Take the slot for this player. `false` means somebody else already has it,
    /// and the caller must not start a second sync.
    pub fn claim(&self, dota_player_id: Uuid) -> bool {
        self.lock().insert(dota_player_id)
    }

    /// Give the slot back. Must run however the sync ended, or that player never
    /// syncs again for the lifetime of the process.
    pub fn release(&self, dota_player_id: Uuid) {
        self.lock().remove(&dota_player_id);
    }

    /// A poisoned lock cannot corrupt a set of ids: the worst a panicking holder
    /// can leave behind is a stale id, and refusing to serve match lists for the
    /// rest of the process is the worse failure.
    fn lock(&self) -> std::sync::MutexGuard<'_, HashSet<Uuid>> {
        self.players.lock().unwrap_or_else(|e| e.into_inner())
    }
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
            sync_in_flight: Arc::new(SyncGuard::default()),
            cache: if config_cache_enabled {
                PostgresCoachingCache::new(db_for_cache, cache_ttl)
            } else {
                Arc::new(NoCache)
            },
        }
    }
}

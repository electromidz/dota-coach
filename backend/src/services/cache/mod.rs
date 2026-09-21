//! The coaching cache, behind a trait.
//!
//! # Why there is no Redis here
//!
//! There is none in this project, and this phase deliberately does not add
//! one. The reasoning, rather than the preference:
//!
//!   - The deployment is a single instance. A second network hop to reach a
//!     value Postgres can return in the same round trip is not a saving.
//!   - Postgres is *already* the shared, restart-surviving cache tier here,
//!     twice over — `benchmark_snapshots` and `hero_meta_snapshots` both use
//!     the same read-through, lazy-expiry, degrade-to-recompute shape.
//!   - The genuinely expensive operation in this service is the model call,
//!     and that is already cached permanently by content hash in
//!     `coaching_analyses`.
//!   - Every cache is a second place data can be wrong. Adding a second
//!     *datastore* to be wrong in is the exact failure this feature exists to
//!     avoid.
//!
//! So the trait is the seam. A `RedisCoachingCache` drops in behind it the day
//! a second instance exists, with no caller changing — the same arrangement
//! `BenchmarkProvider` already has for STRATZ.
//!
//! # The contract every implementation must keep
//!
//! 1. **The database is the source of truth.** Everything stored here is
//!    reconstructable from it. Nothing is ever read from the cache that could
//!    not be derived, and dropping the cache entirely must cost only time.
//! 2. **A failure is a miss.** No cache error may reach a caller. A cache that
//!    can break a page is worse than no cache.
//! 3. **Invalidation is by key derivation.** Callers build a key that carries
//!    a fingerprint of everything the value depends on, so a stale entry is
//!    unreachable rather than evicted. Implementations must not try to be
//!    clever about expiry beyond the TTL backstop.

pub mod postgres;

use async_trait::async_trait;
use uuid::Uuid;

/// A namespaced, fingerprinted cache key.
///
/// Built rather than formatted at call sites so the version and the namespace
/// cannot drift apart across implementations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheKey(String);

impl CacheKey {
    /// Bumped when a cached value's *shape* changes in a way that would make
    /// an old entry deserialize into something wrong rather than fail.
    ///
    /// A shape change that simply fails to deserialize needs no bump — the
    /// caller treats that as a miss. This is for the dangerous kind, where the
    /// old JSON still parses and means something different.
    const VERSION: u32 = 1;

    pub fn new(namespace: &str, player: Uuid, discriminator: &str, fingerprint: &str) -> Self {
        Self(format!(
            "{namespace}:v{}:{player}:{discriminator}:{fingerprint}",
            Self::VERSION
        ))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Read-through storage for values that are pure functions of stored rows.
#[async_trait]
pub trait CoachingCache: Send + Sync {
    /// The cached JSON, or `None` for a miss.
    ///
    /// Returns `None` on failure too. The distinction between "not cached" and
    /// "the cache is broken" matters to an operator reading logs, not to a
    /// caller deciding what to do — both mean recompute.
    async fn get(&self, key: &CacheKey) -> Option<serde_json::Value>;

    /// Store a value. Failures are logged and swallowed.
    async fn put(&self, key: &CacheKey, player: Uuid, value: &serde_json::Value);

    /// Whether this implementation stores anything at all.
    ///
    /// Exists so a deployment can turn caching off without a second code path
    /// and without the callers knowing.
    fn is_enabled(&self) -> bool {
        true
    }
}

/// Typed read-through around a [`CoachingCache`].
///
/// Serialization failures are treated as misses on the way in and as "do not
/// store" on the way out, which keeps every caller free of `serde` handling
/// for something that is only ever an optimisation.
pub async fn read_through<T, F, Fut>(
    cache: &dyn CoachingCache,
    key: &CacheKey,
    player: Uuid,
    compute: F,
) -> Result<T, crate::error::AppError>
where
    T: serde::Serialize + serde::de::DeserializeOwned,
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<T, crate::error::AppError>>,
{
    if let Some(raw) = cache.get(key).await {
        match serde_json::from_value::<T>(raw) {
            Ok(hit) => return Ok(hit),
            // A shape that no longer parses is a miss, not an error. The
            // stored value was only ever a copy of something derivable.
            Err(e) => {
                tracing::debug!(error = %e, "cached value did not deserialize; recomputing")
            }
        }
    }

    let value = compute().await?;

    match serde_json::to_value(&value) {
        Ok(raw) => cache.put(key, player, &raw).await,
        Err(e) => tracing::warn!(error = %e, "cached value did not serialize; not stored"),
    }

    Ok(value)
}

/// A cache that stores nothing.
///
/// The implementation a deployment gets when caching is switched off, and the
/// one the tests use when they need to prove a path does not depend on a hit.
pub struct NoCache;

#[async_trait]
impl CoachingCache for NoCache {
    async fn get(&self, _key: &CacheKey) -> Option<serde_json::Value> {
        None
    }

    async fn put(&self, _key: &CacheKey, _player: Uuid, _value: &serde_json::Value) {}

    fn is_enabled(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_carries_its_namespace_version_player_and_fingerprint() {
        let player = Uuid::nil();
        let key = CacheKey::new("coach-evidence", player, "carry", "abc123");

        assert_eq!(
            key.as_str(),
            "coach-evidence:v1:00000000-0000-0000-0000-000000000000:carry:abc123"
        );
    }

    #[test]
    fn two_roles_never_share_a_key() {
        let player = Uuid::nil();
        // The failure this prevents: a support player served their carry
        // evidence, which would look exactly like a correct answer.
        assert_ne!(
            CacheKey::new("coach-evidence", player, "carry", "abc"),
            CacheKey::new("coach-evidence", player, "hard_support", "abc"),
        );
    }

    #[test]
    fn two_players_never_share_a_key() {
        assert_ne!(
            CacheKey::new("coach-evidence", Uuid::new_v4(), "carry", "abc"),
            CacheKey::new("coach-evidence", Uuid::new_v4(), "carry", "abc"),
        );
    }

    #[test]
    fn a_moved_fingerprint_is_a_different_key() {
        let player = Uuid::nil();
        // This is the whole invalidation mechanism: a stale entry is not
        // evicted, it becomes unreachable.
        assert_ne!(
            CacheKey::new("coach-evidence", player, "carry", "before"),
            CacheKey::new("coach-evidence", player, "carry", "after"),
        );
    }

    #[tokio::test]
    async fn the_disabled_cache_always_misses_and_never_stores() {
        let cache = NoCache;
        let key = CacheKey::new("coach-evidence", Uuid::nil(), "carry", "abc");

        assert!(!cache.is_enabled());
        assert!(cache.get(&key).await.is_none());
        cache
            .put(&key, Uuid::nil(), &serde_json::json!({"a": 1}))
            .await;
        assert!(cache.get(&key).await.is_none());
    }

    #[tokio::test]
    async fn read_through_computes_on_a_miss_and_still_returns_the_value() {
        let key = CacheKey::new("coach-evidence", Uuid::nil(), "carry", "abc");

        let value: Vec<i32> =
            read_through(&NoCache, &key, Uuid::nil(), || async { Ok(vec![1, 2, 3]) })
                .await
                .unwrap();

        assert_eq!(value, vec![1, 2, 3]);
    }

    #[tokio::test]
    async fn a_computation_failure_is_not_swallowed_by_the_cache() {
        // The cache absorbs its own failures, never the caller's.
        let key = CacheKey::new("coach-evidence", Uuid::nil(), "carry", "abc");

        let result: Result<Vec<i32>, _> = read_through(&NoCache, &key, Uuid::nil(), || async {
            Err(crate::error::AppError::NotFound("gone".into()))
        })
        .await;

        assert!(result.is_err());
    }
}

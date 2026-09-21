//! The Postgres implementation of [`CoachingCache`].
//!
//! Same shape as the two provider snapshot caches already in this service:
//! read-through, freshness as a SQL predicate, and every failure degraded to a
//! miss with a warning rather than propagated.

use std::sync::Arc;

use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use super::{CacheKey, CoachingCache};
use crate::repositories;

pub struct PostgresCoachingCache {
    db: PgPool,
    /// A backstop, not the invalidation mechanism — the key handles that. This
    /// bounds how long an entry nobody will look up again occupies space.
    ttl_minutes: i64,
}

impl PostgresCoachingCache {
    pub fn new(db: PgPool, ttl_minutes: i64) -> Arc<Self> {
        Arc::new(Self { db, ttl_minutes })
    }
}

#[async_trait]
impl CoachingCache for PostgresCoachingCache {
    async fn get(&self, key: &CacheKey) -> Option<serde_json::Value> {
        match repositories::coaching_cache::get(&self.db, key.as_str(), self.ttl_minutes).await {
            Ok(hit) => hit,
            // A broken cache must not become a broken page. The caller cannot
            // act on the difference between "absent" and "unreadable", so it
            // is not told — but an operator can see it here.
            Err(e) => {
                tracing::warn!(error = %e, "coaching cache read failed");
                None
            }
        }
    }

    async fn put(&self, key: &CacheKey, player: Uuid, value: &serde_json::Value) {
        if let Err(e) =
            repositories::coaching_cache::put(&self.db, key.as_str(), player, value).await
        {
            tracing::warn!(error = %e, "coaching cache write failed");
        }
    }
}

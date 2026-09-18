//! Recording what happened, without ever being the reason it failed.
//!
//! Tracking is an observation about a request, not part of what the request
//! promises to do: a database hiccup here must not turn a successful login,
//! checkout or generation into a failed one. Every call is therefore
//! fire-and-forget from the caller's point of view — failures are logged, the
//! same way a stale-status rewrite is logged in `services::billing`.

use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::event::EventType;
use crate::repositories;

pub async fn track(pool: &PgPool, user_id: Uuid, event_type: EventType, metadata: serde_json::Value) {
    if let Err(e) = repositories::event::insert(pool, user_id, event_type.slug(), metadata).await {
        tracing::warn!(
            error = %e,
            user_id = %user_id,
            event_type = event_type.slug(),
            "could not record an event"
        );
    }
}

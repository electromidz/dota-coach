//! Recording admin actions, without ever being the reason one fails.
//!
//! Same philosophy as `services::events`: an audit row is an observation
//! about what an admin did, not part of what the action promises to do. A
//! database hiccup here must not turn a successful account disable into a
//! failed one — losing one audit entry is a smaller problem than an admin
//! who can no longer disable a malicious account because logging it failed.

use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::audit::{AuditAction, AuditTargetType};
use crate::repositories;

pub async fn record(
    pool: &PgPool,
    admin_id: Uuid,
    action: AuditAction,
    target_type: AuditTargetType,
    target_id: Uuid,
    metadata: serde_json::Value,
) {
    if let Err(e) = repositories::audit::record(
        pool,
        admin_id,
        action.slug(),
        target_type.slug(),
        target_id,
        metadata,
    )
    .await
    {
        tracing::warn!(
            error = %e,
            admin_id = %admin_id,
            action = action.slug(),
            target_id = %target_id,
            "could not record an admin audit log entry"
        );
    }
}

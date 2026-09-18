use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::audit::AuditLogEntry;

/// Append one entry. Never updated, never deleted except by cascade — and
/// even then only `admin_id` goes to `NULL`; the row itself survives the
/// admin's own account being deleted later.
pub async fn record(
    pool: &PgPool,
    admin_id: Uuid,
    action: &str,
    target_type: &str,
    target_id: Uuid,
    metadata: serde_json::Value,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO admin_audit_log (admin_id, action, target_type, target_id, metadata)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(admin_id)
    .bind(action)
    .bind(target_type)
    .bind(target_id)
    .bind(metadata)
    .execute(pool)
    .await?;

    Ok(())
}

/// Every action any admin has taken, newest first — the audit log view.
pub async fn list(pool: &PgPool, limit: i64, offset: i64) -> Result<Vec<AuditLogEntry>, sqlx::Error> {
    sqlx::query_as::<_, AuditLogEntry>(
        "SELECT a.id, a.admin_id, u.persona_name AS admin_persona_name,
                a.action, a.target_type, a.target_id, a.metadata, a.created_at
           FROM admin_audit_log a
           LEFT JOIN users u ON u.id = a.admin_id
          ORDER BY a.created_at DESC
          LIMIT $1 OFFSET $2",
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
}

pub async fn count(pool: &PgPool) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT count(*) FROM admin_audit_log")
        .fetch_one(pool)
        .await
}

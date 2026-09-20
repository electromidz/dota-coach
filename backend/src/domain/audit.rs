//! Who did what to which account or voucher, and when.
//!
//! Mirrors `domain::event`'s split: a closed enum decides what gets written,
//! so a write can never carry a typo'd action name, while the read side
//! (`AuditLogEntry`) keeps `action`/`target_type` as plain strings — read-only
//! display data nothing acts on, so an entry from a future build this binary
//! doesn't know about still shows up in the log instead of vanishing.

use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditAction {
    ExtendAccess,
    DisableUser,
    EnableUser,
    CreateVoucher,
    DeactivateVoucher,
}

impl AuditAction {
    pub fn slug(self) -> &'static str {
        match self {
            AuditAction::ExtendAccess => "extend_access",
            AuditAction::DisableUser => "disable_user",
            AuditAction::EnableUser => "enable_user",
            AuditAction::CreateVoucher => "create_voucher",
            AuditAction::DeactivateVoucher => "deactivate_voucher",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditTargetType {
    User,
    Voucher,
}

impl AuditTargetType {
    pub fn slug(self) -> &'static str {
        match self {
            AuditTargetType::User => "user",
            AuditTargetType::Voucher => "voucher",
        }
    }
}

/// One row, as the admin audit-log view reads it.
#[derive(Debug, Clone, Serialize, sqlx::FromRow, ToSchema)]
pub struct AuditLogEntry {
    pub id: Uuid,
    /// `None` once the admin who took this action no longer has an account.
    pub admin_id: Option<Uuid>,
    /// The admin's Steam persona, joined in at read time — not stored on the
    /// row itself, so a later name change is reflected here automatically.
    pub admin_persona_name: Option<String>,
    pub action: String,
    pub target_type: String,
    pub target_id: Uuid,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

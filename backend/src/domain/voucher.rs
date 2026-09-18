//! Vouchers: admin-issued codes that grant subscription time without a
//! payment.
//!
//! Two models, mirroring the two tables in `migrations/0016_vouchers.sql`:
//! `Voucher` is a thing an admin made, `VoucherRedemption` is a thing a user
//! did to it. Neither carries logic yet — `used_count < max_uses`, "already
//! redeemed", and the actual extension arithmetic live in
//! `services::voucher`, once that phase adds them, the same split
//! `domain::billing` keeps between stored fact and computed answer.

use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, sqlx::FromRow, ToSchema)]
pub struct Voucher {
    pub id: Uuid,
    pub code: String,
    pub duration_days: i32,
    pub max_uses: i32,
    pub used_count: i32,
    /// `None` means the voucher never expires on its own — `active` is the
    /// other switch that can still turn it off.
    pub expires_at: Option<DateTime<Utc>>,
    pub active: bool,
    /// The admin's own label. Never shown to the redeeming user.
    pub note: Option<String>,
    /// `None` once the admin who made this voucher no longer has an account.
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

/// One redemption. The row that makes "you can only use a code once" a
/// database constraint (`UNIQUE(voucher_id, user_id)`) rather than an
/// application-level race.
#[derive(Debug, Clone, Serialize, sqlx::FromRow, ToSchema)]
pub struct VoucherRedemption {
    pub id: Uuid,
    pub voucher_id: Uuid,
    pub user_id: Uuid,
    pub redeemed_at: DateTime<Utc>,
}

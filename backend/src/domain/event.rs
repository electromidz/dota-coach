//! Raw product-analytics events.
//!
//! One append-only fact per row: something a user did, or something the
//! system decided about their account. The admin panel's questions are all
//! "count/group these", so a generic shape answers them without a schema
//! change per new question — see `migrations/0015_events.sql`.

use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

/// One row, as the admin panel's per-user timeline displays it.
///
/// `event_type` is kept as the raw stored string rather than parsed into
/// `EventType`: this is read-only display data nothing decides on, so an
/// event type from a future build this binary doesn't know about should still
/// show up in the timeline rather than silently vanish — the opposite of how
/// `SubscriptionStatus`/`PaymentStatus` are handled, where an unparseable
/// value is dropped because something *acts* on it.
#[derive(Debug, Clone, Serialize, sqlx::FromRow, ToSchema)]
pub struct EventRecord {
    pub id: Uuid,
    #[sqlx(rename = "type")]
    #[serde(rename = "type")]
    pub event_type: String,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

/// What happened. Kept narrow on purpose: only what Phase 4's admin stats
/// actually need to answer "how many users, what happens during trials, how
/// many buy". `feature_used` carries which feature in its metadata rather than
/// growing a variant per feature.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventType {
    Login,
    Logout,
    TrialStarted,
    TrialExpired,
    /// A paid period lapsing — the `Active → Expired` counterpart to
    /// `TrialExpired`'s `Trialing → Expired`. Kept as a distinct fact:
    /// "the trial ran out" and "the subscription lapsed" are different
    /// things to see in an account's timeline, even though both correct the
    /// same stored `status`.
    SubscriptionExpired,
    Purchase,
    VoucherRedeemed,
    FeatureUsed,
    PageView,
}

impl EventType {
    pub fn slug(self) -> &'static str {
        match self {
            EventType::Login => "login",
            EventType::Logout => "logout",
            EventType::TrialStarted => "trial_started",
            EventType::TrialExpired => "trial_expired",
            EventType::SubscriptionExpired => "subscription_expired",
            EventType::Purchase => "purchase",
            EventType::VoucherRedeemed => "voucher_redeemed",
            EventType::FeatureUsed => "feature_used",
            EventType::PageView => "page_view",
        }
    }
}

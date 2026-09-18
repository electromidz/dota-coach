//! What the admin panel answers: how many people use the product, what
//! happens during their trial, and how many buy.
//!
//! Every count here is computed in SQL over `users`, `subscriptions`, `payments`
//! and `events` — the same "the backend is the source of truth for numbers"
//! rule the rest of the API follows. Nothing here is estimated.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Serialize, Serializer};
use uuid::Uuid;
use utoipa::ToSchema;

use super::event::EventRecord;

/// SteamID64 exceeds JavaScript's safe integer range, so it travels as a
/// string — the same rule `User::steam_id` follows.
fn as_string<S: Serializer>(value: &i64, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.collect_str(value)
}

/// One row of `GET /admin/users`, and the base of the detail view.
#[derive(Debug, Clone, Serialize, sqlx::FromRow, ToSchema)]
pub struct AdminUserSummary {
    pub id: Uuid,
    #[serde(serialize_with = "as_string")]
    pub steam_id: i64,
    pub persona_name: Option<String>,
    pub avatar_url: Option<String>,
    /// `active` or `disabled` — see `User::is_disabled`.
    pub status: String,
    pub is_admin: bool,
    pub created_at: DateTime<Utc>,
    pub last_login_at: Option<DateTime<Utc>>,
    /// `None` only for an account that has never been resolved through
    /// `services::billing::subscription_for` — practically, one that has
    /// never logged in since Phase 3 started materialising the trial at
    /// signup.
    pub subscription_status: Option<String>,
    pub subscription_plan: Option<String>,
    pub trial_ends_at: Option<DateTime<Utc>>,
    pub current_period_end: Option<DateTime<Utc>>,
}

/// `GET /admin/users/:id`: the summary, plus the account's recent history.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AdminUserDetail {
    #[serde(flatten)]
    pub summary: AdminUserSummary,
    /// Newest first. Capped — see `repositories::event::list_for_user`.
    pub events: Vec<EventRecord>,
}

/// One day of `GET /admin/stats`'s signup/login/purchase series.
#[derive(Debug, Clone, Serialize, sqlx::FromRow, ToSchema)]
pub struct DailyStat {
    pub date: NaiveDate,
    pub signups: i64,
    /// Distinct accounts that logged in that day, not total login events —
    /// ten logins from one person is one active user, not ten.
    pub logins: i64,
    /// Raw purchase events that day — two charges from one account on the
    /// same day are two purchases, unlike `logins`.
    pub purchases: i64,
}

/// One redemption of a voucher, as the admin voucher-detail page shows it —
/// who redeemed it, not just an account id.
#[derive(Debug, Clone, Serialize, sqlx::FromRow, ToSchema)]
pub struct VoucherRedemptionSummary {
    pub id: Uuid,
    pub user_id: Uuid,
    #[serde(serialize_with = "as_string")]
    pub steam_id: i64,
    pub persona_name: Option<String>,
    pub redeemed_at: DateTime<Utc>,
}

/// `GET /admin/vouchers/:id`: the voucher, plus everyone who has redeemed it.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AdminVoucherDetail {
    #[serde(flatten)]
    pub voucher: super::voucher::Voucher,
    pub redemptions: Vec<VoucherRedemptionSummary>,
}

/// Everything `GET /admin/stats` answers.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AdminStats {
    /// The window `expired_trials`, `revenue_cents` and `daily` were computed
    /// over. `dau`/`wau`/`mau` are fixed-width windows ending at `to`
    /// regardless of `from` — see the field docs below.
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,

    pub total_users: i64,
    /// Distinct accounts with a `login` event in the 24 hours ending `to`.
    pub dau: i64,
    /// The 7 days ending `to`.
    pub wau: i64,
    /// The 30 days ending `to`.
    pub mau: i64,

    /// `subscriptions.status = 'trialing'` right now.
    pub active_trials: i64,
    /// `trial_expired` events raised inside `[from, to]`.
    pub trials_expired: i64,
    /// Distinct accounts with a `trial_started` event, all-time — the top of
    /// the signup→trial→paid funnel.
    pub trials_started: i64,
    /// Distinct accounts with at least one `purchase` event, all-time — "how
    /// many have ever bought", independent of the window.
    pub paid_users: i64,
    /// `subscriptions.status = 'active'` right now — a point-in-time count,
    /// smaller than `paid_users` whenever someone bought and later lapsed.
    pub currently_paid: i64,
    /// Of accounts whose trial *started* inside `[from, to]`, the percentage
    /// that have purchased by the time this was computed — a cohort
    /// conversion rate, not a snapshot. `None` when no trial started in the
    /// window, so "0%" and "no data" are never confused.
    pub trial_to_paid_conversion_pct: Option<f64>,
    /// `voucher_redeemed` events inside `[from, to]` — conversions via a
    /// code, kept separate from `trial_to_paid_conversion_pct` because that
    /// figure is specifically about paying, and a voucher redemption is not one.
    pub voucher_redemptions: i64,

    /// Settled payments (`status = 'paid'`) inside `[from, to]`, in minor
    /// units of `currency`.
    pub revenue_cents: i64,
    pub currency: String,

    /// Signups and logins per day, oldest first, `from`..=`to` with no gaps —
    /// a day with nothing is `0`, not a missing point.
    pub daily: Vec<DailyStat>,
}

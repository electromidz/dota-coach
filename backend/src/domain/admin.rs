//! What the admin panel answers: how many people use the product, what
//! happens during their trial, and how many buy.
//!
//! Every count here is computed in SQL over `users`, `subscriptions`, `payments`
//! and `events` — the same "the backend is the source of truth for numbers"
//! rule the rest of the API follows. Nothing here is estimated.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Serialize, Serializer};
use utoipa::ToSchema;
use uuid::Uuid;

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
    /// Admin actions taken on this account (extend, disable, enable), newest
    /// first — with who took them and the reason they gave.
    pub admin_actions: Vec<super::audit::AuditLogEntry>,
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
/// One row of the admin voucher list: the voucher, and who redeemed it.
///
/// Vouchers are single-use, so "who redeemed it" is at most one account; the
/// latest redemption is reported for older multi-use codes.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct VoucherListItem {
    #[serde(flatten)]
    pub voucher: super::voucher::Voucher,
    pub redeemed_by: Option<VoucherRedemptionSummary>,
}

/// What `GET /admin/vouchers?status=` narrows by. Mirrors the priority the
/// UI shows: deactivated beats used up, which beats expired.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoucherStatusFilter {
    Active,
    Used,
    Expired,
    Deactivated,
}

impl VoucherStatusFilter {
    pub fn parse(raw: &str) -> Option<Self> {
        Some(match raw {
            "active" => Self::Active,
            "used" => Self::Used,
            "expired" => Self::Expired,
            "deactivated" => Self::Deactivated,
            _ => return None,
        })
    }

    pub fn slug(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Used => "used",
            Self::Expired => "expired",
            Self::Deactivated => "deactivated",
        }
    }
}

/// How the admin user list may be ordered. A closed set, so the `ORDER BY`
/// it becomes is always one of these literals — never caller text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UserSort {
    Name,
    SteamId,
    Status,
    Subscription,
    Plan,
    #[default]
    Joined,
    LastSeen,
}

impl UserSort {
    pub fn parse(raw: &str) -> Option<Self> {
        Some(match raw {
            "name" => Self::Name,
            "steam_id" => Self::SteamId,
            "status" => Self::Status,
            "subscription" => Self::Subscription,
            "plan" => Self::Plan,
            "joined" => Self::Joined,
            "last_seen" => Self::LastSeen,
            _ => return None,
        })
    }

    /// The column expression to sort on.
    pub fn column(self) -> &'static str {
        match self {
            Self::Name => "lower(coalesce(u.persona_name, u.steam_id::text))",
            Self::SteamId => "u.steam_id",
            Self::Status => "u.status",
            Self::Subscription => "s.status",
            Self::Plan => "s.plan",
            Self::Joined => "u.created_at",
            Self::LastSeen => "u.last_login_at",
        }
    }
}

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

    /// Window-scoped counts for `[from, to]`.
    pub period: PeriodCounts,
    /// The same counts for the window of equal length just before `from`, so
    /// every period figure has something to be compared against.
    pub previous: PeriodCounts,
    pub previous_from: DateTime<Utc>,
    pub previous_to: DateTime<Utc>,

    /// All-time signup → trial → paid, with the rate between each step.
    pub funnel: Funnel,
}

/// Counts inside one window. The same query answers the current window and
/// the one before it, so the two can never be measured differently.
#[derive(Debug, Clone, Default, PartialEq, Serialize, sqlx::FromRow, ToSchema)]
pub struct PeriodCounts {
    pub signups: i64,
    /// Distinct accounts that logged in.
    pub active_users: i64,
    /// Distinct accounts whose trial started.
    pub trials_started: i64,
    pub trials_expired: i64,
    /// Raw `purchase` events.
    pub purchases: i64,
    pub voucher_redemptions: i64,
    pub revenue_cents: i64,
}

/// The all-time funnel. A rate is `None` when its denominator is zero —
/// "no trials yet" is a different statement from "0% converted".
#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
pub struct Funnel {
    pub signed_up: i64,
    pub trial_started: i64,
    pub purchased: i64,
    pub signup_to_trial_pct: Option<f64>,
    pub trial_to_paid_pct: Option<f64>,
    pub signup_to_paid_pct: Option<f64>,
}

impl Funnel {
    pub fn new(signed_up: i64, trial_started: i64, purchased: i64) -> Self {
        Self {
            signed_up,
            trial_started,
            purchased,
            signup_to_trial_pct: pct(trial_started, signed_up),
            trial_to_paid_pct: pct(purchased, trial_started),
            signup_to_paid_pct: pct(purchased, signed_up),
        }
    }
}

/// `part / whole` as a percentage, or `None` when there is no whole.
pub fn pct(part: i64, whole: i64) -> Option<f64> {
    (whole > 0).then(|| part as f64 / whole as f64 * 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_funnel_rate_with_no_denominator_is_absent_not_zero() {
        let empty = Funnel::new(0, 0, 0);
        assert_eq!(empty.signup_to_trial_pct, None);
        assert_eq!(empty.trial_to_paid_pct, None);

        let no_trials = Funnel::new(10, 0, 0);
        assert_eq!(no_trials.signup_to_trial_pct, Some(0.0));
        assert_eq!(no_trials.trial_to_paid_pct, None);
    }

    #[test]
    fn funnel_rates_are_step_to_step() {
        let f = Funnel::new(200, 100, 25);
        assert_eq!(f.signup_to_trial_pct, Some(50.0));
        assert_eq!(f.trial_to_paid_pct, Some(25.0));
        assert_eq!(f.signup_to_paid_pct, Some(12.5));
    }

    #[test]
    fn sort_and_status_filters_accept_only_known_values() {
        assert_eq!(UserSort::parse("last_seen"), Some(UserSort::LastSeen));
        assert_eq!(UserSort::parse("u.created_at; DROP TABLE users"), None);
        assert_eq!(VoucherStatusFilter::parse("used"), Some(VoucherStatusFilter::Used));
        assert_eq!(VoucherStatusFilter::parse("used_up"), None);
    }
}

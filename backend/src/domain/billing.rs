//! Trial, subscription and payment state.
//!
//! One rule governs this module: **access is a function of stored timestamps,
//! not of a stored label**. A row that says `trialing` whose `trial_ends_at`
//! passed last night grants nothing, whether or not a sweep has got round to
//! rewriting its status yet. That keeps entitlement correct without depending
//! on a scheduler, and makes it testable with a clock argument instead of a
//! database.
//!
//! Nothing here knows what a payment provider is.

use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use uuid::Uuid;
use utoipa::ToSchema;

/// What an account may do right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Entitlement {
    /// Trial spent, nothing paid for. The measured product still works; the
    /// parts that cost money per use do not.
    Free,
    Trial,
    Pro,
}

impl Entitlement {
    /// The single premium check. Every gate in the API goes through this, so
    /// "what does a trial include?" has exactly one answer to change.
    pub fn allows_premium(self) -> bool {
        matches!(self, Entitlement::Trial | Entitlement::Pro)
    }

    pub fn slug(self) -> &'static str {
        match self {
            Entitlement::Free => "free",
            Entitlement::Trial => "trial",
            Entitlement::Pro => "pro",
        }
    }
}

/// Lifecycle of the subscription row itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionStatus {
    Trialing,
    Active,
    Expired,
    Cancelled,
    PastDue,
}

impl SubscriptionStatus {
    pub fn slug(self) -> &'static str {
        match self {
            SubscriptionStatus::Trialing => "trialing",
            SubscriptionStatus::Active => "active",
            SubscriptionStatus::Expired => "expired",
            SubscriptionStatus::Cancelled => "cancelled",
            SubscriptionStatus::PastDue => "past_due",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        Some(match raw {
            "trialing" => SubscriptionStatus::Trialing,
            "active" => SubscriptionStatus::Active,
            "expired" => SubscriptionStatus::Expired,
            "cancelled" => SubscriptionStatus::Cancelled,
            "past_due" => SubscriptionStatus::PastDue,
            _ => return None,
        })
    }

    pub fn label(self) -> &'static str {
        match self {
            SubscriptionStatus::Trialing => "Trial",
            SubscriptionStatus::Active => "Active",
            SubscriptionStatus::Expired => "Expired",
            SubscriptionStatus::Cancelled => "Cancelled",
            SubscriptionStatus::PastDue => "Past due",
        }
    }
}

/// Where this subscription's current access actually came from.
///
/// Independent of `status`: a `trialing` row is always `Trial`, but an
/// `active` one could have got there by paying, redeeming a voucher, or an
/// admin granting time directly. Whichever of those happens most recently
/// overwrites this — there is one current answer to "why does this account
/// have access", not a history of every way it ever got some.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SubscriptionSource {
    Trial,
    Payment,
    Voucher,
    Admin,
}

impl SubscriptionSource {
    pub fn slug(self) -> &'static str {
        match self {
            SubscriptionSource::Trial => "trial",
            SubscriptionSource::Payment => "payment",
            SubscriptionSource::Voucher => "voucher",
            SubscriptionSource::Admin => "admin",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        Some(match raw {
            "trial" => SubscriptionSource::Trial,
            "payment" => SubscriptionSource::Payment,
            "voucher" => SubscriptionSource::Voucher,
            "admin" => SubscriptionSource::Admin,
            _ => return None,
        })
    }

    pub fn label(self) -> &'static str {
        match self {
            SubscriptionSource::Trial => "Trial",
            SubscriptionSource::Payment => "Payment",
            SubscriptionSource::Voucher => "Voucher",
            SubscriptionSource::Admin => "Admin grant",
        }
    }
}

/// How far a charge has got.
///
/// Deliberately coarser than any provider's own vocabulary: the domain only
/// needs to know whether money is still expected, has arrived, or never will.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum PaymentStatus {
    /// Created, nothing received yet.
    Pending,
    /// Seen on-chain, not yet settled. Grants nothing.
    Confirming,
    /// Settled. The only status that buys anything.
    Paid,
    Failed,
    Expired,
    Refunded,
}

impl PaymentStatus {
    pub fn slug(self) -> &'static str {
        match self {
            PaymentStatus::Pending => "pending",
            PaymentStatus::Confirming => "confirming",
            PaymentStatus::Paid => "paid",
            PaymentStatus::Failed => "failed",
            PaymentStatus::Expired => "expired",
            PaymentStatus::Refunded => "refunded",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        Some(match raw {
            "pending" => PaymentStatus::Pending,
            "confirming" => PaymentStatus::Confirming,
            "paid" => PaymentStatus::Paid,
            "failed" => PaymentStatus::Failed,
            "expired" => PaymentStatus::Expired,
            "refunded" => PaymentStatus::Refunded,
            _ => return None,
        })
    }

    pub fn label(self) -> &'static str {
        match self {
            PaymentStatus::Pending => "Awaiting payment",
            PaymentStatus::Confirming => "Confirming",
            PaymentStatus::Paid => "Paid",
            PaymentStatus::Failed => "Failed",
            PaymentStatus::Expired => "Expired",
            PaymentStatus::Refunded => "Refunded",
        }
    }

    /// Whether this charge can still become `Paid`. A settled, failed, expired
    /// or refunded charge is final, so a late notification cannot revive it.
    pub fn is_open(self) -> bool {
        matches!(self, PaymentStatus::Pending | PaymentStatus::Confirming)
    }
}

/// An account's billing state.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Subscription {
    pub id: Uuid,
    pub status: SubscriptionStatus,
    pub status_label: &'static str,
    pub source: SubscriptionSource,
    pub plan: String,
    pub trial_started_at: DateTime<Utc>,
    pub trial_ends_at: DateTime<Utc>,
    pub current_period_start: Option<DateTime<Utc>>,
    pub current_period_end: Option<DateTime<Utc>>,
    pub provider: Option<String>,
    /// Provider identifiers are storage detail, not something the browser has
    /// any use for, so they stop at the repository.
    #[serde(skip)]
    pub provider_customer_id: Option<String>,
    #[serde(skip)]
    pub provider_subscription_id: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl Subscription {
    /// What this subscription grants at `now`.
    ///
    /// The two windows are checked in order of generosity, and the status label
    /// deliberately does not appear: a paid window that has not elapsed is
    /// honoured whatever the row calls itself. That is what makes `cancelled`
    /// and `past_due` behave correctly for free — cancelling asks us not to
    /// renew, not to confiscate days already bought.
    pub fn entitlement(&self, now: DateTime<Utc>) -> Entitlement {
        if self.current_period_end.is_some_and(|end| end > now) {
            Entitlement::Pro
        } else if self.status == SubscriptionStatus::Trialing && self.trial_ends_at > now {
            Entitlement::Trial
        } else {
            Entitlement::Free
        }
    }

    /// When the current access ends, whichever window is providing it.
    pub fn access_ends_at(&self, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
        match self.entitlement(now) {
            Entitlement::Pro => self.current_period_end,
            Entitlement::Trial => Some(self.trial_ends_at),
            Entitlement::Free => None,
        }
    }

    /// Whole days of access left, floored, for display. `None` once access has
    /// run out — "0 days left" and "expired" are different statements.
    pub fn days_remaining(&self, now: DateTime<Utc>) -> Option<i64> {
        self.access_ends_at(now)
            .map(|end| (end - now).num_days().max(0))
    }

    /// The status this row *should* carry at `now`, given its timestamps.
    ///
    /// Returns `None` when the stored status is already correct, so the caller
    /// writes only when something actually changed.
    pub fn drifted_status(&self, now: DateTime<Utc>) -> Option<SubscriptionStatus> {
        let expired = self.entitlement(now) == Entitlement::Free;

        match self.status {
            SubscriptionStatus::Trialing | SubscriptionStatus::Active if expired => {
                Some(SubscriptionStatus::Expired)
            }
            _ => None,
        }
    }
}

/// A charge, as the domain sees it.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Payment {
    pub id: Uuid,
    pub provider: String,
    /// The provider's id for this charge. Storage detail: it identifies the
    /// charge to the provider, and the browser has no use for it.
    #[serde(skip)]
    pub provider_payment_id: Option<String>,
    pub status: PaymentStatus,
    pub status_label: &'static str,
    /// Minor units of `currency`. The frontend formats; it never computes.
    pub amount_cents: i64,
    pub currency: String,
    pub pay_currency: Option<String>,
    pub payment_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

/// The offer, as configured. Sent to the frontend so the price is displayed
/// from one place rather than written into the markup.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Plan {
    pub name: String,
    pub amount_cents: i64,
    pub currency: String,
    pub period_days: i64,
    pub trial_days: i64,
}

/// Everything `/api/billing` answers in one object.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct BillingOverview {
    pub entitlement: Entitlement,
    pub subscription: Subscription,
    pub plan: Plan,
    pub days_remaining: Option<i64>,
    pub access_ends_at: Option<DateTime<Utc>>,
    pub payments: Vec<Payment>,
    /// Whether checkout can be started at all on this deployment. False when
    /// no payment provider is configured — the honest answer to a button that
    /// would otherwise 503.
    pub checkout_available: bool,
}

/// Extend a paid window by `period_days`.
///
/// Renewing early must not cost the user the days they already paid for, so a
/// period that is still running is extended from its own end rather than from
/// now. An expired one starts again at `now`.
pub fn next_period(
    current_end: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
    period_days: i64,
) -> (DateTime<Utc>, DateTime<Utc>) {
    let start = match current_end {
        Some(end) if end > now => end,
        _ => now,
    };

    (start, start + Duration::days(period_days))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(hours: i64) -> DateTime<Utc> {
        DateTime::from_timestamp(1_700_000_000 + hours * 3_600, 0).unwrap()
    }

    fn subscription(status: SubscriptionStatus) -> Subscription {
        Subscription {
            id: Uuid::nil(),
            status,
            status_label: status.label(),
            source: SubscriptionSource::Trial,
            plan: "pro".into(),
            trial_started_at: at(0),
            // A 14-day trial in hours.
            trial_ends_at: at(24 * 14),
            current_period_start: None,
            current_period_end: None,
            provider: None,
            provider_customer_id: None,
            provider_subscription_id: None,
            created_at: at(0),
        }
    }

    #[test]
    fn a_running_trial_grants_premium_and_an_elapsed_one_does_not() {
        let sub = subscription(SubscriptionStatus::Trialing);

        assert_eq!(sub.entitlement(at(24)), Entitlement::Trial);
        assert!(sub.entitlement(at(24)).allows_premium());

        // One hour past the end of the window.
        assert_eq!(sub.entitlement(at(24 * 14 + 1)), Entitlement::Free);
        assert!(!sub.entitlement(at(24 * 14 + 1)).allows_premium());
    }

    #[test]
    fn entitlement_follows_the_timestamps_even_when_the_status_is_stale() {
        // Nothing has swept this row yet; it still says `active`.
        let mut sub = subscription(SubscriptionStatus::Active);
        sub.current_period_start = Some(at(0));
        sub.current_period_end = Some(at(24));

        assert_eq!(sub.entitlement(at(23)), Entitlement::Pro);
        assert_eq!(sub.entitlement(at(25)), Entitlement::Free);
        assert_eq!(
            sub.drifted_status(at(25)),
            Some(SubscriptionStatus::Expired),
            "an elapsed window should be rewritten"
        );
        assert_eq!(sub.drifted_status(at(23)), None);
    }

    #[test]
    fn paying_during_the_trial_upgrades_rather_than_downgrades() {
        let mut sub = subscription(SubscriptionStatus::Active);
        sub.current_period_start = Some(at(1));
        sub.current_period_end = Some(at(24 * 31));

        assert_eq!(sub.entitlement(at(2)), Entitlement::Pro);
    }

    #[test]
    fn cancelling_keeps_what_was_already_paid_for() {
        let mut sub = subscription(SubscriptionStatus::Cancelled);
        sub.current_period_start = Some(at(0));
        sub.current_period_end = Some(at(24 * 10));

        assert_eq!(sub.entitlement(at(24)), Entitlement::Pro);
        assert_eq!(sub.entitlement(at(24 * 11)), Entitlement::Free);
        // A cancellation is a user decision, not drift; it is never rewritten.
        assert_eq!(sub.drifted_status(at(24 * 11)), None);
    }

    #[test]
    fn a_past_due_account_keeps_nothing_once_its_window_closes() {
        let mut sub = subscription(SubscriptionStatus::PastDue);
        sub.current_period_end = Some(at(24));

        assert_eq!(sub.entitlement(at(12)), Entitlement::Pro);
        assert_eq!(sub.entitlement(at(36)), Entitlement::Free);
    }

    #[test]
    fn renewing_early_adds_to_the_window_instead_of_replacing_it() {
        let end = at(24 * 10);
        let (start, next) = next_period(Some(end), at(24), 30);

        assert_eq!(start, end, "the paid-for days are not forfeited");
        assert_eq!(next, end + Duration::days(30));
    }

    #[test]
    fn renewing_after_a_lapse_starts_from_now() {
        let now = at(24 * 40);
        let (start, next) = next_period(Some(at(24 * 10)), now, 30);

        assert_eq!(start, now);
        assert_eq!(next, now + Duration::days(30));
    }

    #[test]
    fn days_remaining_floors_and_stops_at_expiry() {
        let sub = subscription(SubscriptionStatus::Trialing);

        assert_eq!(sub.days_remaining(at(0)), Some(14));
        // 13 days and 23 hours left is 13 whole days, not 14.
        assert_eq!(sub.days_remaining(at(1)), Some(13));
        assert_eq!(sub.days_remaining(at(24 * 15)), None);
    }

    #[test]
    fn only_an_open_charge_can_still_settle() {
        assert!(PaymentStatus::Pending.is_open());
        assert!(PaymentStatus::Confirming.is_open());
        assert!(!PaymentStatus::Paid.is_open());
        assert!(!PaymentStatus::Refunded.is_open());
    }

    #[test]
    fn every_status_survives_a_round_trip_through_storage() {
        for status in [
            SubscriptionStatus::Trialing,
            SubscriptionStatus::Active,
            SubscriptionStatus::Expired,
            SubscriptionStatus::Cancelled,
            SubscriptionStatus::PastDue,
        ] {
            assert_eq!(SubscriptionStatus::parse(status.slug()), Some(status));
        }

        for status in [
            PaymentStatus::Pending,
            PaymentStatus::Confirming,
            PaymentStatus::Paid,
            PaymentStatus::Failed,
            PaymentStatus::Expired,
            PaymentStatus::Refunded,
        ] {
            assert_eq!(PaymentStatus::parse(status.slug()), Some(status));
        }

        assert_eq!(SubscriptionStatus::parse("gifted"), None);
    }
}

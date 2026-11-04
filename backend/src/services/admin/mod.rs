//! Assembling `GET /admin/stats` out of the repository's individual counts.
//!
//! Everything numeric is computed in SQL (`repositories::admin`); this module
//! only decides the default window, and turns two counts into a percentage
//! without inventing precision a small cohort can't support.

use chrono::{DateTime, Duration, Utc};
use sqlx::PgPool;

use crate::domain::admin::{AdminStats, Funnel};
use crate::repositories;

/// The series and window-scoped counts default to the last 30 days when the
/// caller supplies neither bound — long enough to see a trend, short enough
/// that the daily series stays a chart rather than a table.
const DEFAULT_WINDOW_DAYS: i64 = 30;

pub async fn stats(
    pool: &PgPool,
    currency: &str,
    from: Option<DateTime<Utc>>,
    to: Option<DateTime<Utc>>,
) -> Result<AdminStats, sqlx::Error> {
    let to = to.unwrap_or_else(Utc::now);
    let from = from.unwrap_or(to - Duration::days(DEFAULT_WINDOW_DAYS));

    let total_users = repositories::admin::count_total_users(pool).await?;

    // Fixed-width activity windows ending at `to`, independent of `from`:
    // "how many people were active this week" doesn't change meaning because
    // the caller also asked for a wider revenue window.
    let dau = repositories::admin::count_active_between(pool, to - Duration::days(1), to).await?;
    let wau = repositories::admin::count_active_between(pool, to - Duration::days(7), to).await?;
    let mau = repositories::admin::count_active_between(pool, to - Duration::days(30), to).await?;

    let active_trials = repositories::admin::count_subscription_status(pool, "trialing").await?;
    let currently_paid = repositories::admin::count_subscription_status(pool, "active").await?;
    let trials_expired = repositories::admin::count_trial_expired_events(pool, from, to).await?;
    let trials_started = repositories::admin::count_trial_started_alltime(pool).await?;
    let paid_users = repositories::admin::count_paid_users_alltime(pool).await?;

    let (cohort_size, purchased) = repositories::admin::trial_conversion(pool, from, to).await?;
    let trial_to_paid_conversion_pct =
        (cohort_size > 0).then(|| purchased as f64 / cohort_size as f64 * 100.0);
    let voucher_redemptions =
        repositories::admin::count_voucher_redemptions(pool, from, to).await?;

    let revenue_cents = repositories::admin::revenue_cents(pool, from, to).await?;
    let daily = repositories::admin::daily_series(pool, from.date_naive(), to.date_naive()).await?;

    // The comparison window: the same length, ending where this one starts.
    let (previous_from, previous_to) = previous_window(from, to);
    let period = repositories::admin::period_counts(pool, from, to).await?;
    let previous = repositories::admin::period_counts(pool, previous_from, previous_to).await?;

    Ok(AdminStats {
        from,
        to,
        total_users,
        dau,
        wau,
        mau,
        active_trials,
        trials_expired,
        trials_started,
        paid_users,
        currently_paid,
        trial_to_paid_conversion_pct,
        voucher_redemptions,
        revenue_cents,
        currency: currency.to_string(),
        daily,
        period,
        previous,
        previous_from,
        previous_to,
        funnel: Funnel::new(total_users, trials_started, paid_users),
    })
}

/// The window of equal length immediately before `[from, to]`. Its end stops
/// a millisecond short of `from`, because both windows are inclusive and an
/// event exactly at `from` belongs to the current one only.
fn previous_window(from: DateTime<Utc>, to: DateTime<Utc>) -> (DateTime<Utc>, DateTime<Utc>) {
    let length = to - from;
    let previous_to = from - Duration::milliseconds(1);
    (previous_to - length, previous_to)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn the_previous_window_has_the_same_length_and_does_not_overlap() {
        let from = Utc.with_ymd_and_hms(2026, 10, 1, 0, 0, 0).unwrap();
        let to = Utc.with_ymd_and_hms(2026, 10, 31, 0, 0, 0).unwrap();

        let (prev_from, prev_to) = previous_window(from, to);

        assert_eq!(prev_to - prev_from, to - from);
        assert!(prev_to < from);
        assert_eq!(from - prev_to, Duration::milliseconds(1));
    }
}

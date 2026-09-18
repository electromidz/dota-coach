//! Queries behind the admin panel. Nothing here is reachable except through
//! `api::extract::AdminUser` — every function still takes only what it needs
//! and trusts nothing about who is calling, the same as every other
//! repository, but the gate lives at the extractor, not here.

use chrono::{DateTime, NaiveDate, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::admin::{AdminUserSummary, DailyStat, VoucherRedemptionSummary};

// `macro_rules!` rather than `const`, matching `subscription_columns!` in
// `repositories::billing`: sqlx requires a `&'static str` literal, and only a
// macro expansion — not a `const` reference — composes as one via `concat!`.
macro_rules! user_columns {
    () => {
        "u.id, u.steam_id, u.persona_name, u.avatar_url, u.status, u.is_admin,
         u.created_at, u.last_login_at,
         s.status AS subscription_status, s.plan AS subscription_plan,
         s.trial_ends_at, s.current_period_end"
    };
}

macro_rules! user_join {
    () => {
        "FROM users u LEFT JOIN subscriptions s ON s.user_id = u.id"
    };
}

/// What `GET /admin/users` may narrow the list by. `None` on a field means
/// "don't filter on this" — every predicate below is written so a `NULL`
/// parameter is a no-op rather than matching nothing.
#[derive(Debug, Clone, Default)]
pub struct UserFilters {
    /// `active` or `disabled` — `users.status`.
    pub status: Option<String>,
    /// `trialing`, `active`, `expired`, `cancelled` or `past_due` —
    /// `subscriptions.status`, the account's billing lifecycle rather than
    /// `users.status`.
    pub plan: Option<String>,
    /// Accounts that haven't been seen since before this instant — including
    /// ones that have never logged in at all.
    pub last_login_before: Option<DateTime<Utc>>,
    /// Matched against `persona_name` (case-insensitive, partial) or
    /// `steam_id` (exact) — this product has no email or username.
    pub search: Option<String>,
}

pub async fn list_users(
    pool: &PgPool,
    filters: &UserFilters,
    limit: i64,
    offset: i64,
) -> Result<Vec<AdminUserSummary>, sqlx::Error> {
    sqlx::query_as::<_, AdminUserSummary>(concat!(
        "SELECT ",
        user_columns!(),
        " ",
        user_join!(),
        " WHERE ($3::text IS NULL OR u.status = $3)
            AND ($4::text IS NULL OR s.status = $4)
            AND ($5::timestamptz IS NULL OR u.last_login_at IS NULL OR u.last_login_at < $5)
            AND ($6::text IS NULL OR u.persona_name ILIKE '%' || $6 || '%' OR u.steam_id::text = $6)
          ORDER BY u.created_at DESC
          LIMIT $1 OFFSET $2"
    ))
    .bind(limit)
    .bind(offset)
    .bind(&filters.status)
    .bind(&filters.plan)
    .bind(filters.last_login_before)
    .bind(&filters.search)
    .fetch_all(pool)
    .await
}

pub async fn count_users(pool: &PgPool, filters: &UserFilters) -> Result<i64, sqlx::Error> {
    // Same predicate as `list_users`, renumbered from $1: there is no
    // limit/offset pair here to take $1/$2.
    sqlx::query_scalar(concat!(
        "SELECT count(*) ",
        user_join!(),
        " WHERE ($1::text IS NULL OR u.status = $1)
            AND ($2::text IS NULL OR s.status = $2)
            AND ($3::timestamptz IS NULL OR u.last_login_at IS NULL OR u.last_login_at < $3)
            AND ($4::text IS NULL OR u.persona_name ILIKE '%' || $4 || '%' OR u.steam_id::text = $4)"
    ))
    .bind(&filters.status)
    .bind(&filters.plan)
    .bind(filters.last_login_before)
    .bind(&filters.search)
    .fetch_one(pool)
    .await
}

pub async fn find_user_summary(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Option<AdminUserSummary>, sqlx::Error> {
    sqlx::query_as::<_, AdminUserSummary>(concat!(
        "SELECT ",
        user_columns!(),
        " ",
        user_join!(),
        " WHERE u.id = $1"
    ))
    .bind(user_id)
    .fetch_optional(pool)
    .await
}

/// Add `days` to whichever window currently governs the account's access —
/// `trial_ends_at` for a `trialing` or `expired` row, `current_period_end`
/// for an `active` or `past_due` one — and pull a lapsed row back to
/// `trialing` in the same write. An `expired` row always resumes as
/// `trialing` regardless of whether it lapsed from a trial or a paid period:
/// "give them more time" from an admin means "let them back in", not a
/// guess at which window they last had.
///
/// Guarded to accounts that already have a subscription row — every real
/// account gets one at first login (Phase 3), so a missing row means "this id
/// has never actually signed in", which extending access cannot fix.
/// Returns `false` when there was nothing to extend.
pub async fn extend_access(pool: &PgPool, user_id: Uuid, days: i64) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE subscriptions
            SET trial_ends_at = CASE
                                   WHEN status IN ('trialing', 'expired')
                                   THEN trial_ends_at + make_interval(days => $2)
                                   ELSE trial_ends_at
                                 END,
                current_period_end = CASE
                                        WHEN status IN ('active', 'past_due')
                                        THEN COALESCE(current_period_end, now())
                                             + make_interval(days => $2)
                                        ELSE current_period_end
                                      END,
                status = CASE WHEN status = 'expired' THEN 'trialing' ELSE status END,
                updated_at = now()
          WHERE user_id = $1",
    )
    .bind(user_id)
    .bind(i32::try_from(days).unwrap_or(i32::MAX))
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

/// Returns `false` when the id does not exist.
pub async fn disable_user(pool: &PgPool, user_id: Uuid) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("UPDATE users SET status = 'disabled' WHERE id = $1")
        .bind(user_id)
        .execute(pool)
        .await?;

    Ok(result.rows_affected() > 0)
}

/// Returns `false` when the id does not exist.
pub async fn enable_user(pool: &PgPool, user_id: Uuid) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("UPDATE users SET status = 'active' WHERE id = $1")
        .bind(user_id)
        .execute(pool)
        .await?;

    Ok(result.rows_affected() > 0)
}

pub async fn count_total_users(pool: &PgPool) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT count(*) FROM users")
        .fetch_one(pool)
        .await
}

/// Distinct accounts with a `login` event since `since`. Distinct, not a raw
/// event count: `dau`/`wau`/`mau` answer "how many people", not "how many
/// times".
pub async fn count_active_since(pool: &PgPool, since: DateTime<Utc>) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT count(DISTINCT user_id) FROM events WHERE type = 'login' AND created_at >= $1",
    )
    .bind(since)
    .fetch_one(pool)
    .await
}

/// A live count of the billing lifecycle stage named — `'trialing'` for
/// active trials, `'active'` for currently-paid accounts. Read straight off
/// `subscriptions.status`, which the sweep in `services::billing` keeps
/// accurate independently of whether anyone is looking.
pub async fn count_subscription_status(pool: &PgPool, status: &str) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT count(*) FROM subscriptions WHERE status = $1")
        .bind(status)
        .fetch_one(pool)
        .await
}

pub async fn count_trial_expired_events(
    pool: &PgPool,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT count(*) FROM events
          WHERE type = 'trial_expired' AND created_at BETWEEN $1 AND $2",
    )
    .bind(from)
    .bind(to)
    .fetch_one(pool)
    .await
}

/// Distinct accounts with a `purchase` event, ever — "how many have bought",
/// independent of whether they later cancelled or lapsed.
pub async fn count_paid_users_alltime(pool: &PgPool) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT count(DISTINCT user_id) FROM events WHERE type = 'purchase'")
        .fetch_one(pool)
        .await
}

/// Distinct accounts with a `trial_started` event, ever — the top of the
/// signup→trial→paid funnel. All-time, like `count_paid_users_alltime`: a
/// funnel comparing "trials running right now" against "everyone who ever
/// bought" would understate conversion for no reason but a timing mismatch.
pub async fn count_trial_started_alltime(pool: &PgPool) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT count(DISTINCT user_id) FROM events WHERE type = 'trial_started'")
        .fetch_one(pool)
        .await
}

/// Of the accounts whose trial started inside `[from, to]`, how many have
/// ever purchased — a cohort measured at the time this runs, not restricted
/// to purchases inside the same window. Returns `(cohort_size, purchased)`.
pub async fn trial_conversion(
    pool: &PgPool,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> Result<(i64, i64), sqlx::Error> {
    sqlx::query_as(
        "WITH cohort AS (
             SELECT DISTINCT user_id FROM events
              WHERE type = 'trial_started' AND created_at BETWEEN $1 AND $2
         )
         SELECT
             (SELECT count(*) FROM cohort) AS cohort_size,
             (SELECT count(*) FROM cohort c
               WHERE EXISTS (
                   SELECT 1 FROM events e
                    WHERE e.user_id = c.user_id AND e.type = 'purchase'
               )) AS purchased",
    )
    .bind(from)
    .bind(to)
    .fetch_one(pool)
    .await
}

pub async fn count_voucher_redemptions(
    pool: &PgPool,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT count(*) FROM events
          WHERE type = 'voucher_redeemed' AND created_at BETWEEN $1 AND $2",
    )
    .bind(from)
    .bind(to)
    .fetch_one(pool)
    .await
}

/// Every redemption of one voucher, newest first, with who redeemed it —
/// the admin voucher-detail page. Lives here rather than
/// `repositories::voucher` because it joins in `users`, the same reasoning
/// `AdminUserSummary`'s query joins in `subscriptions`.
pub async fn list_voucher_redemptions(
    pool: &PgPool,
    voucher_id: Uuid,
) -> Result<Vec<VoucherRedemptionSummary>, sqlx::Error> {
    sqlx::query_as::<_, VoucherRedemptionSummary>(
        "SELECT r.id, r.user_id, u.steam_id, u.persona_name, r.redeemed_at
           FROM voucher_redemptions r
           JOIN users u ON u.id = r.user_id
          WHERE r.voucher_id = $1
          ORDER BY r.redeemed_at DESC",
    )
    .bind(voucher_id)
    .fetch_all(pool)
    .await
}

/// Settled payments inside `[from, to]`, in minor units. Single-figure
/// because this deployment prices in one configured currency
/// (`BillingConfig::currency`) — see `domain::admin::AdminStats::currency`.
pub async fn revenue_cents(
    pool: &PgPool,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount_cents), 0)::bigint FROM payments
          WHERE status = 'paid' AND completed_at BETWEEN $1 AND $2",
    )
    .bind(from)
    .bind(to)
    .fetch_one(pool)
    .await
}

/// Signups, logins and purchases per calendar day, `from`..=`to` inclusive
/// with no gaps — a day nothing happened is `0`, not a missing point a chart
/// would have to interpolate across.
///
/// Logins count distinct accounts (ten logins from one person is one active
/// user); purchases count raw events (two charges from one person on the
/// same day are two purchases, not one).
pub async fn daily_series(
    pool: &PgPool,
    from: NaiveDate,
    to: NaiveDate,
) -> Result<Vec<DailyStat>, sqlx::Error> {
    sqlx::query_as(
        "SELECT d.day::date AS date,
                COALESCE(s.count, 0) AS signups,
                COALESCE(l.count, 0) AS logins,
                COALESCE(p.count, 0) AS purchases
           FROM generate_series($1::date, $2::date, interval '1 day') AS d(day)
           LEFT JOIN (
               SELECT date_trunc('day', created_at) AS day, count(*) AS count
                 FROM users
                WHERE created_at >= $1::date AND created_at < $2::date + interval '1 day'
                GROUP BY 1
           ) s ON s.day = d.day
           LEFT JOIN (
               SELECT date_trunc('day', created_at) AS day, count(DISTINCT user_id) AS count
                 FROM events
                WHERE type = 'login'
                  AND created_at >= $1::date AND created_at < $2::date + interval '1 day'
                GROUP BY 1
           ) l ON l.day = d.day
           LEFT JOIN (
               SELECT date_trunc('day', created_at) AS day, count(*) AS count
                 FROM events
                WHERE type = 'purchase'
                  AND created_at >= $1::date AND created_at < $2::date + interval '1 day'
                GROUP BY 1
           ) p ON p.day = d.day
          ORDER BY d.day",
    )
    .bind(from)
    .bind(to)
    .fetch_all(pool)
    .await
}

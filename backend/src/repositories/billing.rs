use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::domain::billing::{Payment, PaymentStatus, Subscription, SubscriptionStatus};

/// A macro rather than a `const`: sqlx needs a `&'static str` query, and
/// `concat!` only composes literals.
macro_rules! subscription_columns {
    () => {
        "id, status, plan, trial_started_at, trial_ends_at, current_period_start,
         current_period_end, provider, provider_customer_id, provider_subscription_id,
         created_at"
    };
}

macro_rules! payment_columns {
    () => {
        "id, provider, provider_payment_id, amount_cents, currency,
         pay_currency, status, payment_url, created_at, completed_at"
    };
}

/// The account's subscription, creating the trial row if it is not there yet.
///
/// The trial is anchored to `users.created_at`, not to now: a row materialised
/// on a user's first visit to the billing page must describe the same fourteen
/// days it would have described on the day they signed up. `ON CONFLICT DO
/// NOTHING` makes two concurrent first requests agree on one row.
pub async fn ensure_subscription(
    pool: &PgPool,
    user_id: Uuid,
    plan: &str,
    trial_days: i64,
) -> Result<Subscription, sqlx::Error> {
    if let Some(existing) = find_by_user(pool, user_id).await? {
        return Ok(existing);
    }

    sqlx::query(
        "INSERT INTO subscriptions
             (user_id, status, plan, trial_started_at, trial_ends_at)
         SELECT u.id, 'trialing', $2, u.created_at, u.created_at + make_interval(days => $3)
           FROM users u
          WHERE u.id = $1
         ON CONFLICT (user_id) DO NOTHING",
    )
    .bind(user_id)
    .bind(plan)
    .bind(i32::try_from(trial_days).unwrap_or(i32::MAX))
    .execute(pool)
    .await?;

    find_by_user(pool, user_id)
        .await?
        // The insert selects from `users`, so the only way to get here is an
        // account that vanished mid-request.
        .ok_or(sqlx::Error::RowNotFound)
}

pub async fn find_by_user(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Option<Subscription>, sqlx::Error> {
    let row = sqlx::query_as::<_, SubscriptionRow>(concat!(
        "SELECT ",
        subscription_columns!(),
        " FROM subscriptions WHERE user_id = $1"
    ))
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.and_then(SubscriptionRow::into_domain))
}

/// Rewrite a status whose window has elapsed.
///
/// Guarded on the old status so a sweep racing an activation cannot demote a
/// subscription that was just paid for.
pub async fn mark_status(
    pool: &PgPool,
    subscription_id: Uuid,
    from: SubscriptionStatus,
    to: SubscriptionStatus,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE subscriptions
            SET status = $3, updated_at = now()
          WHERE id = $1 AND status = $2",
    )
    .bind(subscription_id)
    .bind(from.slug())
    .bind(to.slug())
    .execute(pool)
    .await?;

    Ok(())
}

/// A charge, reserved before the provider is called.
///
/// The row exists first so the `order_id` we hand the provider already names
/// something durable: a notification can never arrive about a charge we have
/// no record of.
pub struct NewPayment<'a> {
    pub user_id: Uuid,
    pub subscription_id: Uuid,
    pub provider: &'a str,
    pub amount_cents: i64,
    pub currency: &'a str,
}

pub async fn reserve_payment(pool: &PgPool, payment: &NewPayment<'_>) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO payments
             (user_id, subscription_id, provider, amount_cents, currency, status)
         VALUES ($1, $2, $3, $4, $5, 'pending')
         RETURNING id",
    )
    .bind(payment.user_id)
    .bind(payment.subscription_id)
    .bind(payment.provider)
    .bind(payment.amount_cents)
    .bind(payment.currency)
    .fetch_one(pool)
    .await
}

/// Attach what the provider told us when the charge was opened.
pub async fn attach_provider_payment(
    pool: &PgPool,
    payment_id: Uuid,
    provider_payment_id: &str,
    payment_url: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE payments
            SET provider_payment_id = $2,
                payment_url         = COALESCE($3, payment_url),
                updated_at          = now()
          WHERE id = $1",
    )
    .bind(payment_id)
    .bind(provider_payment_id)
    .bind(payment_url)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn find_payment(pool: &PgPool, id: Uuid) -> Result<Option<Payment>, sqlx::Error> {
    let row = sqlx::query_as::<_, PaymentRow>(concat!(
        "SELECT ",
        payment_columns!(),
        " FROM payments WHERE id = $1"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(row.and_then(PaymentRow::into_domain))
}

/// Look a charge up the way a notification identifies it.
pub async fn find_payment_by_provider_id(
    pool: &PgPool,
    provider: &str,
    provider_payment_id: &str,
) -> Result<Option<Payment>, sqlx::Error> {
    let row = sqlx::query_as::<_, PaymentRow>(concat!(
        "SELECT ",
        payment_columns!(),
        " FROM payments WHERE provider = $1 AND provider_payment_id = $2"
    ))
    .bind(provider)
    .bind(provider_payment_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.and_then(PaymentRow::into_domain))
}

/// The account's charges, newest first.
pub async fn payments_for_user(
    pool: &PgPool,
    user_id: Uuid,
    limit: i64,
) -> Result<Vec<Payment>, sqlx::Error> {
    let rows = sqlx::query_as::<_, PaymentRow>(concat!(
        "SELECT ",
        payment_columns!(),
        " FROM payments
           WHERE user_id = $1
           ORDER BY created_at DESC
           LIMIT $2"
    ))
    .bind(user_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .filter_map(PaymentRow::into_domain)
        .collect())
}

/// The most recent charge that can still settle, if any.
///
/// Checkout reuses it instead of opening a second one: a user who reloads the
/// billing page should not end up with two invoices for the same month.
pub async fn open_payment_for_user(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Option<Payment>, sqlx::Error> {
    let row = sqlx::query_as::<_, PaymentRow>(concat!(
        "SELECT ",
        payment_columns!(),
        " FROM payments
           WHERE user_id = $1 AND status IN ('pending', 'confirming')
           ORDER BY created_at DESC
           LIMIT 1"
    ))
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.and_then(PaymentRow::into_domain))
}

/// Record a notification before it is applied.
///
/// Returns `false` when this exact event has already been recorded, which is
/// the whole idempotency mechanism: the unique index decides, not a read
/// followed by a write.
pub async fn record_webhook_event(
    tx: &mut Transaction<'_, Postgres>,
    provider: &str,
    event_key: &str,
    payment_id: Option<Uuid>,
    status: PaymentStatus,
    payload: &serde_json::Value,
) -> Result<bool, sqlx::Error> {
    let inserted: Option<Uuid> = sqlx::query_scalar(
        "INSERT INTO billing_webhook_events
             (provider, event_key, payment_id, status, payload)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (provider, event_key) DO NOTHING
         RETURNING id",
    )
    .bind(provider)
    .bind(event_key)
    .bind(payment_id)
    .bind(status.slug())
    .bind(payload)
    .fetch_optional(&mut **tx)
    .await?;

    Ok(inserted.is_some())
}

/// Move a charge along, inside the caller's transaction.
///
/// `completed_at` is stamped once, by `COALESCE`, so a redelivered settlement
/// cannot move the moment money arrived.
pub async fn update_payment_status(
    tx: &mut Transaction<'_, Postgres>,
    payment_id: Uuid,
    status: PaymentStatus,
    provider_payment_id: Option<&str>,
    pay_currency: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE payments
            SET status              = $2,
                provider_payment_id = COALESCE($3, provider_payment_id),
                pay_currency        = COALESCE($4, pay_currency),
                completed_at        = CASE
                                        WHEN $2 = 'paid' THEN COALESCE(completed_at, now())
                                        ELSE completed_at
                                      END,
                updated_at          = now()
          WHERE id = $1",
    )
    .bind(payment_id)
    .bind(status.slug())
    .bind(provider_payment_id)
    .bind(pay_currency)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

/// Grant a paid window, inside the caller's transaction.
///
/// The period is computed by the domain and passed in, so this statement makes
/// no decisions about time at all.
pub async fn activate_subscription(
    tx: &mut Transaction<'_, Postgres>,
    subscription_id: Uuid,
    period_start: DateTime<Utc>,
    period_end: DateTime<Utc>,
    provider: &str,
    provider_subscription_id: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE subscriptions
            SET status                   = 'active',
                current_period_start     = $2,
                current_period_end       = $3,
                provider                 = $4,
                provider_subscription_id = COALESCE($5, provider_subscription_id),
                updated_at               = now()
          WHERE id = $1",
    )
    .bind(subscription_id)
    .bind(period_start)
    .bind(period_end)
    .bind(provider)
    .bind(provider_subscription_id)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

/// The subscription a charge belongs to, locked for the duration of the
/// transaction so two notifications for the same account cannot each read the
/// old period end and both extend from it.
pub async fn lock_subscription_for_payment(
    tx: &mut Transaction<'_, Postgres>,
    payment_id: Uuid,
) -> Result<Option<Subscription>, sqlx::Error> {
    let row = sqlx::query_as::<_, SubscriptionRow>(concat!(
        // The join makes every bare column name ambiguous, so the subscription
        // is selected through a subquery rather than by prefixing the list.
        "SELECT ",
        subscription_columns!(),
        " FROM subscriptions
          WHERE id = (SELECT subscription_id FROM payments WHERE id = $1)
            FOR UPDATE"
    ))
    .bind(payment_id)
    .fetch_optional(&mut **tx)
    .await?;

    Ok(row.and_then(SubscriptionRow::into_domain))
}

/// Re-read a charge inside the transaction that is about to change it.
///
/// The status read outside the transaction is advisory; this one is the value
/// the state transition is decided on.
pub async fn lock_payment(
    tx: &mut Transaction<'_, Postgres>,
    payment_id: Uuid,
) -> Result<Option<Payment>, sqlx::Error> {
    let row = sqlx::query_as::<_, PaymentRow>(concat!(
        "SELECT ",
        payment_columns!(),
        " FROM payments WHERE id = $1 FOR UPDATE"
    ))
    .bind(payment_id)
    .fetch_optional(&mut **tx)
    .await?;

    Ok(row.and_then(PaymentRow::into_domain))
}

#[derive(sqlx::FromRow)]
struct SubscriptionRow {
    id: Uuid,
    status: String,
    plan: String,
    trial_started_at: DateTime<Utc>,
    trial_ends_at: DateTime<Utc>,
    current_period_start: Option<DateTime<Utc>>,
    current_period_end: Option<DateTime<Utc>>,
    provider: Option<String>,
    provider_customer_id: Option<String>,
    provider_subscription_id: Option<String>,
    created_at: DateTime<Utc>,
}

impl SubscriptionRow {
    /// A row whose status this build cannot parse came from a definition it
    /// does not have. Dropping it is safer than guessing: the caller
    /// materialises a fresh trial rather than granting access it cannot
    /// explain.
    fn into_domain(self) -> Option<Subscription> {
        let status = SubscriptionStatus::parse(&self.status)?;

        Some(Subscription {
            id: self.id,
            status,
            status_label: status.label(),
            plan: self.plan,
            trial_started_at: self.trial_started_at,
            trial_ends_at: self.trial_ends_at,
            current_period_start: self.current_period_start,
            current_period_end: self.current_period_end,
            provider: self.provider,
            provider_customer_id: self.provider_customer_id,
            provider_subscription_id: self.provider_subscription_id,
            created_at: self.created_at,
        })
    }
}

#[derive(sqlx::FromRow)]
struct PaymentRow {
    id: Uuid,
    provider: String,
    provider_payment_id: Option<String>,
    amount_cents: i64,
    currency: String,
    pay_currency: Option<String>,
    status: String,
    payment_url: Option<String>,
    created_at: DateTime<Utc>,
    completed_at: Option<DateTime<Utc>>,
}

impl PaymentRow {
    fn into_domain(self) -> Option<Payment> {
        let status = PaymentStatus::parse(&self.status)?;

        Some(Payment {
            id: self.id,
            provider: self.provider,
            provider_payment_id: self.provider_payment_id,
            status,
            status_label: status.label(),
            amount_cents: self.amount_cents,
            currency: self.currency,
            pay_currency: self.pay_currency,
            payment_url: self.payment_url,
            created_at: self.created_at,
            completed_at: self.completed_at,
        })
    }
}

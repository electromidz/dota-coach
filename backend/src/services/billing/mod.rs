//! Trial, entitlement and settlement.
//!
//! This is the one place that decides whether an account may use a premium
//! feature, and the one place that turns money into access. Three rules hold
//! everywhere in it:
//!
//!   1. **The backend is the source of truth.** Nothing here reads a claim
//!      from the browser. Access is derived from stored timestamps; a paid
//!      window is only ever opened by a signed provider notification or by
//!      asking the provider directly.
//!   2. **Applying a settlement is idempotent.** Providers retry. Every
//!      accepted notification is recorded under a unique key *in the same
//!      transaction* that applies it, so a redelivery cannot buy a second
//!      month.
//!   3. **A notification is validated against what we asked for.** Signature
//!      first, then the order id, the amount and the currency. A mismatch on
//!      any of them grants nothing.

use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

use crate::config::BillingConfig;
use crate::domain::billing::{
    next_period, BillingOverview, Entitlement, Payment, PaymentStatus, Plan, Subscription,
};
use crate::domain::user::User;
use crate::repositories;
use crate::services::payments::{CheckoutRequest, PaymentError, PaymentProvider, PaymentUpdate};

#[derive(Debug, thiserror::Error)]
pub enum BillingError {
    #[error(transparent)]
    Provider(#[from] PaymentError),
    /// The notification is about a charge this deployment never opened.
    #[error("no charge matches this notification")]
    UnknownPayment,
    /// Signed correctly, but describing a different amount or currency than
    /// the one we asked for. Logged loudly; grants nothing.
    #[error("notification does not match the charge it names")]
    AmountMismatch,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

/// What happened to one provider notification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebhookOutcome {
    /// Recorded and applied. `activated` says whether it bought a period.
    Applied { activated: bool },
    /// Already seen. Acknowledged without being applied a second time.
    Duplicate,
    /// Valid, but the charge had already reached a final state. Recorded for
    /// the audit trail, applied to nothing.
    Ignored,
}

/// The account's billing state, with the trial materialised if needed.
///
/// Also the point where a stale label is corrected: a row still saying
/// `trialing` whose fourteen days elapsed is rewritten to `expired` here.
/// Entitlement never depended on that label — the timestamps already decided —
/// so the write is bookkeeping, and a failure to perform it is logged rather
/// than propagated.
pub async fn subscription_for(
    pool: &PgPool,
    config: &BillingConfig,
    user_id: Uuid,
) -> Result<Subscription, sqlx::Error> {
    let mut subscription =
        repositories::billing::ensure_subscription(pool, user_id, &config.plan, config.trial_days)
            .await?;

    let now = Utc::now();
    if let Some(corrected) = subscription.drifted_status(now) {
        if let Err(e) = repositories::billing::mark_status(
            pool,
            subscription.id,
            subscription.status,
            corrected,
        )
        .await
        {
            tracing::warn!(error = %e, "could not rewrite an elapsed subscription status");
        }

        // Carried into the answer whether or not the write landed: the
        // timestamps already say the window closed, and reporting `trialing`
        // next to `entitlement: free` would be two answers to one question.
        subscription.status = corrected;
        subscription.status_label = corrected.label();
    }

    Ok(subscription)
}

/// The gate every premium handler goes through.
///
/// Returns what the account may do *now*. On a deployment with no payment
/// provider, `enforce` is off and this reports `Pro`: refusing a feature nobody
/// can pay to unlock would be a worse answer than giving it away.
pub async fn entitlement_for(
    pool: &PgPool,
    config: &BillingConfig,
    user_id: Uuid,
) -> Result<Entitlement, sqlx::Error> {
    if !config.enforce {
        return Ok(Entitlement::Pro);
    }

    Ok(subscription_for(pool, config, user_id)
        .await?
        .entitlement(Utc::now()))
}

/// Everything `/api/billing` answers.
pub async fn overview(
    pool: &PgPool,
    config: &BillingConfig,
    provider: &dyn PaymentProvider,
    user: &User,
) -> Result<BillingOverview, sqlx::Error> {
    let subscription = subscription_for(pool, config, user.id).await?;
    let payments =
        repositories::billing::payments_for_user(pool, user.id, config.history_limit).await?;

    let now = Utc::now();

    Ok(BillingOverview {
        entitlement: subscription.entitlement(now),
        days_remaining: subscription.days_remaining(now),
        access_ends_at: subscription.access_ends_at(now),
        plan: plan(config),
        subscription,
        payments,
        checkout_available: provider.is_configured(),
    })
}

pub fn plan(config: &BillingConfig) -> Plan {
    Plan {
        name: config.plan.clone(),
        amount_cents: config.price_cents,
        currency: config.currency.clone(),
        period_days: config.period_days,
        trial_days: config.trial_days,
    }
}

/// Open a charge, or hand back the one that is still open.
///
/// A user who reloads the billing page and clicks again should not end up with
/// two invoices for the same month, so an open charge is refreshed from the
/// provider and reused. The refresh is also the recovery path for a
/// notification that never arrived: the provider's own answer is authoritative,
/// and goes through exactly the same application logic a webhook does.
pub async fn start_checkout(
    pool: &PgPool,
    config: &BillingConfig,
    provider: &dyn PaymentProvider,
    public_base_url: &str,
    frontend_base_url: &str,
    user: &User,
) -> Result<Payment, BillingError> {
    if !provider.is_configured() {
        return Err(PaymentError::NotConfigured.into());
    }

    let subscription = subscription_for(pool, config, user.id).await?;

    if let Some(open) = repositories::billing::open_payment_for_user(pool, user.id).await? {
        if let Some(refreshed) = refresh(pool, config, provider, &open).await {
            return Ok(refreshed);
        }
    }

    // Reserved before the provider is called, so the order id we send already
    // names a durable row: a notification can never arrive about a charge we
    // have no record of.
    let payment_id = repositories::billing::reserve_payment(
        pool,
        &repositories::billing::NewPayment {
            user_id: user.id,
            subscription_id: subscription.id,
            provider: provider.name(),
            amount_cents: config.price_cents,
            currency: &config.currency,
        },
    )
    .await?;

    let request = CheckoutRequest {
        order_id: payment_id.to_string(),
        amount_cents: config.price_cents,
        currency: config.currency.clone(),
        description: format!("Dota Coach {} subscription", config.plan),
        success_url: format!("{frontend_base_url}/billing?checkout=success"),
        cancel_url: format!("{frontend_base_url}/billing?checkout=cancelled"),
        ipn_callback_url: format!("{public_base_url}/api/billing/webhook"),
    };

    let session = match provider.create_payment(&request).await {
        Ok(session) => session,
        Err(e) => {
            // The reserved row stays `pending` and expires with the rest; it is
            // cheaper than a charge the provider knows about and we do not.
            tracing::warn!(error = %e, %payment_id, "checkout could not be opened");
            return Err(e.into());
        }
    };

    repositories::billing::attach_provider_payment(
        pool,
        payment_id,
        &session.provider_payment_id,
        session.payment_url.as_deref(),
    )
    .await?;

    repositories::billing::find_payment(pool, payment_id)
        .await?
        .ok_or(BillingError::UnknownPayment)
}

/// Ask the provider what happened to an open charge, and apply the answer.
///
/// Best-effort by design: an unreachable provider must not stop a user from
/// paying, so a failure here returns `None` and the caller opens a fresh
/// charge. Returns the charge only while it is still usable.
async fn refresh(
    pool: &PgPool,
    config: &BillingConfig,
    provider: &dyn PaymentProvider,
    open: &Payment,
) -> Option<Payment> {
    let provider_payment_id = open.provider_payment_id.as_deref()?;

    match provider.get_payment_status(provider_payment_id).await {
        Ok(update) => {
            let payload = serde_json::json!({
                "source": "poll",
                "provider_payment_id": update.provider_payment_id,
                "status": update.status.slug(),
            });

            if let Err(e) = apply_update(pool, config, provider.name(), &update, &payload).await {
                tracing::warn!(error = %e, payment_id = %open.id, "could not apply a polled status");
            }
        }
        Err(e) => {
            tracing::warn!(error = %e, payment_id = %open.id, "could not refresh a charge");
        }
    }

    // Re-read: the poll may have settled or killed it.
    let current = repositories::billing::find_payment(pool, open.id)
        .await
        .unwrap_or(None)?;

    // A hosted page is what makes reuse useful; without one there is nothing to
    // send the user back to.
    current
        .status
        .is_open()
        .then_some(current)
        .filter(|p| p.payment_url.is_some())
}

/// Verify a provider notification and apply it.
///
/// The signature is checked by the provider implementation before a single
/// field of the body is believed; everything after that point is validation
/// against what we actually asked for.
pub async fn process_webhook(
    pool: &PgPool,
    config: &BillingConfig,
    provider: &dyn PaymentProvider,
    signature: Option<&str>,
    body: &[u8],
) -> Result<WebhookOutcome, BillingError> {
    let update = provider.handle_webhook(signature, body)?;

    // Stored only after verification, so this table never holds unauthenticated
    // input. A body that no longer parses as JSON cannot reach here.
    let payload = serde_json::from_slice(body).unwrap_or(serde_json::Value::Null);

    apply_update(pool, config, provider.name(), &update, &payload).await
}

/// The single path from a trusted update to stored access.
///
/// Both the webhook and the polling recovery path go through here, so there is
/// exactly one implementation of "what does a settled charge do to an account".
async fn apply_update(
    pool: &PgPool,
    config: &BillingConfig,
    provider_name: &str,
    update: &PaymentUpdate,
    payload: &serde_json::Value,
) -> Result<WebhookOutcome, BillingError> {
    let payment = locate(pool, provider_name, update).await?;
    validate_against_charge(&payment, update)?;

    let mut tx = pool.begin().await?;

    // The unique index decides, not a read followed by a write: two concurrent
    // redeliveries of the same event cannot both proceed.
    let first_time = repositories::billing::record_webhook_event(
        &mut tx,
        provider_name,
        &update.event_key,
        Some(payment.id),
        update.status,
        payload,
    )
    .await?;

    if !first_time {
        tx.commit().await?;
        tracing::info!(
            payment_id = %payment.id,
            event_key = %update.event_key,
            "duplicate billing notification acknowledged without being applied"
        );
        return Ok(WebhookOutcome::Duplicate);
    }

    // Re-read under the row lock: the status decided on must be the stored one,
    // not the one read before the transaction started.
    let locked = repositories::billing::lock_payment(&mut tx, payment.id)
        .await?
        .ok_or(BillingError::UnknownPayment)?;

    if !locked.status.is_open() {
        tx.commit().await?;
        tracing::info!(
            payment_id = %payment.id,
            stored = locked.status.slug(),
            reported = update.status.slug(),
            "notification for a charge that had already finished"
        );
        return Ok(WebhookOutcome::Ignored);
    }

    repositories::billing::update_payment_status(
        &mut tx,
        payment.id,
        update.status,
        Some(update.provider_payment_id.as_str()),
        update.pay_currency.as_deref(),
    )
    .await?;

    let mut activated = false;
    if update.status == PaymentStatus::Paid {
        // Locked for the rest of the transaction, so two settlements for the
        // same account cannot both extend from the same period end.
        let subscription =
            repositories::billing::lock_subscription_for_payment(&mut tx, payment.id)
                .await?
                .ok_or(BillingError::UnknownPayment)?;

        let (start, end) = next_period(
            subscription.current_period_end,
            Utc::now(),
            config.period_days,
        );

        repositories::billing::activate_subscription(
            &mut tx,
            subscription.id,
            start,
            end,
            provider_name,
            Some(update.provider_payment_id.as_str()),
        )
        .await?;

        activated = true;
        tracing::info!(
            payment_id = %payment.id,
            subscription_id = %subscription.id,
            period_end = %end,
            "subscription activated by a verified payment"
        );
    }

    tx.commit().await?;
    Ok(WebhookOutcome::Applied { activated })
}

/// Find the charge a notification is about.
///
/// The order id is ours and is stable from the moment the row is reserved, so
/// it is tried first. The provider's own id is the fallback — with a hosted
/// invoice, the id known at checkout is the invoice's, and the real payment id
/// only appears on the first notification.
async fn locate(
    pool: &PgPool,
    provider_name: &str,
    update: &PaymentUpdate,
) -> Result<Payment, BillingError> {
    if let Some(order_id) = update.order_id.as_deref().and_then(|id| id.parse().ok()) {
        if let Some(payment) = repositories::billing::find_payment(pool, order_id).await? {
            return Ok(payment);
        }
    }

    let found = repositories::billing::find_payment_by_provider_id(
        pool,
        provider_name,
        &update.provider_payment_id,
    )
    .await?;

    found.ok_or_else(|| {
        tracing::warn!(
            provider_payment_id = %update.provider_payment_id,
            order_id = ?update.order_id,
            "notification names a charge this deployment never opened"
        );
        BillingError::UnknownPayment
    })
}

/// A signature proves who sent the message, not that the message describes
/// what we asked for. Amount and currency are checked against the reserved
/// charge; anything else is refused rather than part-honoured.
fn validate_against_charge(payment: &Payment, update: &PaymentUpdate) -> Result<(), BillingError> {
    if let Some(cents) = update.amount_cents {
        if cents != payment.amount_cents {
            tracing::error!(
                payment_id = %payment.id,
                expected = payment.amount_cents,
                reported = cents,
                "billing notification reports a different amount than the charge"
            );
            return Err(BillingError::AmountMismatch);
        }
    }

    if let Some(currency) = update.currency.as_deref() {
        if !currency.eq_ignore_ascii_case(&payment.currency) {
            tracing::error!(
                payment_id = %payment.id,
                expected = %payment.currency,
                reported = %currency,
                "billing notification reports a different currency than the charge"
            );
            return Err(BillingError::AmountMismatch);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::billing::PaymentStatus;
    use chrono::{DateTime, Utc};

    fn charge() -> Payment {
        Payment {
            id: Uuid::nil(),
            provider: "nowpayments".into(),
            provider_payment_id: Some("5745".into()),
            status: PaymentStatus::Pending,
            status_label: PaymentStatus::Pending.label(),
            amount_cents: 100,
            currency: "usd".into(),
            pay_currency: None,
            payment_url: Some("https://pay.example/i/1".into()),
            created_at: DateTime::<Utc>::from_timestamp(1_700_000_000, 0).unwrap(),
            completed_at: None,
        }
    }

    fn update(amount_cents: Option<i64>, currency: Option<&str>) -> PaymentUpdate {
        PaymentUpdate {
            provider_payment_id: "5745".into(),
            order_id: None,
            status: PaymentStatus::Paid,
            amount_cents,
            currency: currency.map(str::to_string),
            pay_currency: None,
            event_key: "5745:paid".into(),
        }
    }

    #[test]
    fn a_settlement_for_the_right_money_is_accepted() {
        assert!(validate_against_charge(&charge(), &update(Some(100), Some("usd"))).is_ok());
        // Casing is a provider formatting choice, not a different currency.
        assert!(validate_against_charge(&charge(), &update(Some(100), Some("USD"))).is_ok());
    }

    #[test]
    fn a_settlement_for_the_wrong_money_grants_nothing() {
        assert!(matches!(
            validate_against_charge(&charge(), &update(Some(1), Some("usd"))),
            Err(BillingError::AmountMismatch)
        ));
        assert!(matches!(
            validate_against_charge(&charge(), &update(Some(100), Some("eur"))),
            Err(BillingError::AmountMismatch)
        ));
    }

    #[test]
    fn a_notification_that_repeats_no_figures_leaves_the_stored_ones_standing() {
        // Some provider callbacks carry only an id and a status. That is not a
        // mismatch; there is simply nothing to disagree with.
        assert!(validate_against_charge(&charge(), &update(None, None)).is_ok());
    }
}

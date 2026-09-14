//! Trial, subscription and checkout.
//!
//! Every read here is derived server-side from stored timestamps; nothing in a
//! request body can influence an entitlement, and the only way a subscription
//! becomes active is a payment the provider signed for.

use axum::body::Bytes;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::Json;
use serde::Serialize;

use crate::api::extract::CurrentUser;
use crate::domain::billing::{BillingOverview, Entitlement, Payment, Plan, Subscription};
use crate::error::AppResult;
use crate::repositories;
use crate::services::billing::{self, WebhookOutcome};
use crate::state::AppState;

/// `GET /api/billing` — everything the billing page needs in one request.
pub async fn overview(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> AppResult<Json<BillingOverview>> {
    Ok(Json(
        billing::overview(
            &state.db,
            &state.config.billing,
            state.payments.as_ref(),
            &user,
        )
        .await?,
    ))
}

#[derive(Serialize)]
pub struct SubscriptionResponse {
    pub entitlement: Entitlement,
    pub subscription: Subscription,
    pub plan: Plan,
    pub days_remaining: Option<i64>,
}

/// `GET /api/billing/subscription` — the entitlement on its own.
pub async fn subscription(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> AppResult<Json<SubscriptionResponse>> {
    let subscription = billing::subscription_for(&state.db, &state.config.billing, user.id).await?;
    let now = chrono::Utc::now();

    Ok(Json(SubscriptionResponse {
        entitlement: subscription.entitlement(now),
        days_remaining: subscription.days_remaining(now),
        plan: billing::plan(&state.config.billing),
        subscription,
    }))
}

#[derive(Serialize)]
pub struct PaymentsResponse {
    pub payments: Vec<Payment>,
}

/// `GET /api/billing/payments` — this account's charges, newest first.
///
/// Scoped by the session's user id, never by one supplied by the caller.
pub async fn payments(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> AppResult<Json<PaymentsResponse>> {
    Ok(Json(PaymentsResponse {
        payments: repositories::billing::payments_for_user(
            &state.db,
            user.id,
            state.config.billing.history_limit,
        )
        .await?,
    }))
}

#[derive(Serialize)]
pub struct CheckoutResponse {
    pub payment: Payment,
}

/// `POST /api/billing/checkout` — open (or reuse) a charge.
///
/// Answers `503 FEATURE_UNAVAILABLE` when the deployment has no payment
/// provider, which is the honest answer to a button that cannot work.
pub async fn checkout(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> AppResult<Json<CheckoutResponse>> {
    let payment = billing::start_checkout(
        &state.db,
        &state.config.billing,
        state.payments.as_ref(),
        &state.config.auth.public_base_url,
        &state.config.auth.frontend_base_url,
        &user,
    )
    .await?;

    Ok(Json(CheckoutResponse { payment }))
}

#[derive(Serialize)]
pub struct WebhookResponse {
    pub received: bool,
    /// `applied`, `duplicate` or `ignored`. Useful in provider dashboards and
    /// in our own logs; it tells the provider nothing it can act on.
    pub outcome: &'static str,
}

/// `POST /api/billing/webhook` — the provider tells us a charge moved.
///
/// The only unauthenticated write in the API, and the only one that can grant
/// access. It is safe because it trusts nothing but a valid HMAC over the exact
/// bytes received: the body is read as raw [`Bytes`] rather than deserialized
/// first, since any re-serialization would change what was signed.
pub async fn webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> AppResult<Json<WebhookResponse>> {
    let signature = headers
        .get(state.payments.signature_header())
        .and_then(|value| value.to_str().ok());

    let outcome = billing::process_webhook(
        &state.db,
        &state.config.billing,
        state.payments.as_ref(),
        signature,
        &body,
    )
    .await?;

    Ok(Json(WebhookResponse {
        received: true,
        outcome: match outcome {
            WebhookOutcome::Applied { .. } => "applied",
            WebhookOutcome::Duplicate => "duplicate",
            WebhookOutcome::Ignored => "ignored",
        },
    }))
}

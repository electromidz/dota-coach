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
use utoipa::ToSchema;

/// `GET /api/billing` — everything the billing page needs in one request.
#[utoipa::path(
    get, path = "/api/billing", tag = "billing",
    summary = "Entitlement and billing state",
    description = "The server is the only source of truth for trial and subscription state. Reading is always allowed: an expired account still needs to see why it is expired and how to fix it.",
    security(("session" = [])),
    responses(
        (status = 200, description = "Plan, entitlement and subscription together", body = BillingOverview),
        (status = 401, description = "No session cookie, or it has expired", body = crate::error::ErrorBody),
        (status = 500, description = "Database or internal failure", body = crate::error::ErrorBody),
    )
)]
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

#[derive(Serialize, ToSchema)]
pub struct PlanResponse {
    pub plan: Plan,
    /// Whether this deployment can actually sell the plan. The landing page
    /// says "start your trial" either way; the billing page uses this to stop
    /// offering a button that would answer `503`.
    pub checkout_available: bool,
}

/// `GET /api/billing/plan` — the offer, for visitors who are not signed in.
///
/// The only billing route without a session, and deliberately so: the landing
/// page has to state the trial length and the price, and the alternative is
/// hard-coding them in the markup where they would drift from
/// `BILLING_PRICE_CENTS`. It exposes nothing that is not already on the pricing
/// copy — no account, no provider identifiers, no credentials.
#[utoipa::path(
    get, path = "/api/billing/plan", tag = "billing",
    summary = "The public offer",
    description = "Unauthenticated, for the signed-out landing page. Carries nothing that is not public pricing copy.",
    responses((status = 200, description = "Price, currency and trial length", body = PlanResponse))
)]
pub async fn plan(State(state): State<AppState>) -> Json<PlanResponse> {
    Json(PlanResponse {
        plan: billing::plan(&state.config.billing),
        checkout_available: state.payments.is_configured(),
    })
}

#[derive(Serialize, ToSchema)]
pub struct SubscriptionResponse {
    pub entitlement: Entitlement,
    pub subscription: Subscription,
    pub plan: Plan,
    pub days_remaining: Option<i64>,
}

/// `GET /api/billing/subscription` — the entitlement on its own.
#[utoipa::path(
    get, path = "/api/billing/subscription", tag = "billing",
    summary = "The current subscription",
    security(("session" = [])),
    responses(
        (status = 200, description = "Subscription, or null when there has never been one", body = SubscriptionResponse),
        (status = 401, description = "No session cookie, or it has expired", body = crate::error::ErrorBody),
        (status = 500, description = "Database or internal failure", body = crate::error::ErrorBody),
    )
)]
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

#[derive(Serialize, ToSchema)]
pub struct PaymentsResponse {
    pub payments: Vec<Payment>,
}

/// `GET /api/billing/payments` — this account's charges, newest first.
///
/// Scoped by the session's user id, never by one supplied by the caller.
#[utoipa::path(
    get, path = "/api/billing/payments", tag = "billing",
    summary = "Payment history",
    security(("session" = [])),
    responses(
        (status = 200, description = "Every payment recorded for this user", body = PaymentsResponse),
        (status = 401, description = "No session cookie, or it has expired", body = crate::error::ErrorBody),
        (status = 500, description = "Database or internal failure", body = crate::error::ErrorBody),
    )
)]
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

#[derive(Serialize, ToSchema)]
pub struct CheckoutResponse {
    pub payment: Payment,
}

/// `POST /api/billing/checkout` — open (or reuse) a charge.
///
/// Answers `503 FEATURE_UNAVAILABLE` when the deployment has no payment
/// provider, which is the honest answer to a button that cannot work.
#[utoipa::path(
    post, path = "/api/billing/checkout", tag = "billing",
    summary = "Open a checkout invoice",
    description = "Returns a hosted payment URL. Creating an invoice grants nothing: the subscription activates only when the provider's signed webhook confirms settlement.",
    security(("session" = [])),
    responses(
        (status = 200, description = "Invoice created; send the user to `payment_url`", body = CheckoutResponse),
        (status = 409, description = "The subscription is already active", body = crate::error::ErrorBody),
        (status = 502, description = "The payment provider failed", body = crate::error::ErrorBody),
        (status = 503, description = "No payment provider is configured on this server", body = crate::error::ErrorBody),
        (status = 401, description = "No session cookie, or it has expired", body = crate::error::ErrorBody),
        (status = 500, description = "Database or internal failure", body = crate::error::ErrorBody),
    )
)]
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

#[derive(Serialize, ToSchema)]
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
#[utoipa::path(
    post, path = "/api/billing/webhook", tag = "billing",
    summary = "Payment provider callback",
    description = "The only unauthenticated write in the API. Safe because it believes nothing that is not signed: the HMAC is verified against the IPN secret before the body is read, and processing is idempotent, so a replayed callback settles nothing twice. Never called by a browser.",
    // Taken as raw bytes, never as parsed JSON: the signature covers the exact
    // payload sent, and re-serializing it would change what was signed.
    request_body(
        content = String,
        content_type = "application/json",
        description = "The provider's IPN payload, verified byte-for-byte against the signature header",
    ),
    responses(
        (status = 200, description = "Accepted, or ignored as a duplicate", body = WebhookResponse),
        (status = 400, description = "Signature missing, malformed, or does not match", body = crate::error::ErrorBody),
        (status = 500, description = "Database failure while recording the payment", body = crate::error::ErrorBody),
    )
)]
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

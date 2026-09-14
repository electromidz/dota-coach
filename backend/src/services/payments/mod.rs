//! Payment provider abstraction.
//!
//! The boundary carries three verbs and no vendor vocabulary: open a charge,
//! ask what happened to a charge, and turn a signed notification into a
//! normalized update. Everything that decides what an update *means* for an
//! account — whether it activates a subscription, whether it has already been
//! applied — lives above this trait in `services::billing`, so it is testable
//! without a provider, a key or a network.
//!
//! Nothing a provider returns is trusted beyond its signature: the amount,
//! currency and order id on every update are re-checked against what we
//! actually asked for before a single day of access is granted.

pub mod nowpayments;

use async_trait::async_trait;

use crate::domain::billing::PaymentStatus;

/// What we ask a provider to collect.
#[derive(Debug, Clone)]
pub struct CheckoutRequest {
    /// Our payment row id. Comes back on the webhook and is how an update is
    /// matched to a charge.
    pub order_id: String,
    /// Minor units of `currency` — never a float, and never the coin amount.
    pub amount_cents: i64,
    /// ISO-4217, lowercase (`usd`).
    pub currency: String,
    pub description: String,
    /// Where the provider sends the browser afterwards.
    pub success_url: String,
    pub cancel_url: String,
    /// Where the provider sends the notification. Must be reachable from the
    /// public internet for anything to ever settle.
    pub ipn_callback_url: String,
}

/// An opened charge.
#[derive(Debug, Clone)]
pub struct CheckoutSession {
    pub provider_payment_id: String,
    /// Hosted checkout page, when the provider offers one.
    pub payment_url: Option<String>,
    pub status: PaymentStatus,
}

/// A provider's statement about one charge, normalized.
///
/// `amount_cents` and `currency` are `Option` because not every notification
/// repeats them; when they are present they are validated, and when they are
/// absent the stored figures stand.
#[derive(Debug, Clone)]
pub struct PaymentUpdate {
    pub provider_payment_id: String,
    /// The `order_id` we sent, echoed back. Checked against the charge it
    /// claims to be about.
    pub order_id: Option<String>,
    pub status: PaymentStatus,
    pub amount_cents: Option<i64>,
    pub currency: Option<String>,
    pub pay_currency: Option<String>,
    /// Stable key for this exact notification, used to make processing
    /// idempotent. Providers retry; a retry must not buy a second month.
    pub event_key: String,
}

#[derive(Debug, thiserror::Error)]
pub enum PaymentError {
    /// No API key or IPN secret. Distinct from an outage: nothing was
    /// attempted, and the fix is configuration rather than patience.
    #[error("payment provider is not configured")]
    NotConfigured,
    #[error("payment provider unavailable: {0}")]
    Unavailable(String),
    #[error("payment provider rate limited")]
    RateLimited,
    /// The notification did not carry a valid signature. Always logged,
    /// never explained to the caller.
    #[error("webhook signature rejected")]
    InvalidSignature,
    #[error("unexpected payment provider response: {0}")]
    InvalidResponse(String),
}

impl PaymentError {
    /// A short, user-safe explanation for the checkout path.
    pub fn user_note(&self) -> &'static str {
        match self {
            PaymentError::NotConfigured => {
                "Payments are not configured on this server, so checkout is unavailable."
            }
            PaymentError::RateLimited => {
                "The payment provider is rate limiting us. Try again in a minute."
            }
            _ => "The payment provider is unavailable right now. Try again shortly.",
        }
    }
}

/// One payment provider.
///
/// `handle_webhook` is deliberately synchronous and side-effect free: it
/// verifies and parses, and nothing more. Persisting the result is the billing
/// service's job, which is what keeps "is this signature valid?" testable
/// without a database.
#[async_trait]
pub trait PaymentProvider: Send + Sync {
    /// Stored on every row this provider creates, so a deployment that later
    /// switches providers can still explain its history.
    fn name(&self) -> &'static str;

    /// Whether a call is even possible. Checked before a payment row is
    /// reserved, so an unconfigured server never accumulates dead charges.
    fn is_configured(&self) -> bool;

    /// Which request header carries the notification signature. Asked rather
    /// than assumed, so the webhook handler stays free of vendor detail.
    fn signature_header(&self) -> &'static str;

    async fn create_payment(
        &self,
        request: &CheckoutRequest,
    ) -> Result<CheckoutSession, PaymentError>;

    /// Authoritative status, straight from the provider. Used to confirm a
    /// charge without relying on the browser, and to recover when a
    /// notification never arrives.
    async fn get_payment_status(
        &self,
        provider_payment_id: &str,
    ) -> Result<PaymentUpdate, PaymentError>;

    /// Verify a notification's signature and normalize its body.
    fn handle_webhook(
        &self,
        signature: Option<&str>,
        body: &[u8],
    ) -> Result<PaymentUpdate, PaymentError>;
}

/// A provider that is not there.
///
/// Used when no credentials are configured: checkout answers
/// `FEATURE_UNAVAILABLE` instead of the process refusing to start, because
/// every other part of the product works perfectly well without billing.
pub struct UnconfiguredPaymentProvider;

#[async_trait]
impl PaymentProvider for UnconfiguredPaymentProvider {
    fn name(&self) -> &'static str {
        "none"
    }

    fn is_configured(&self) -> bool {
        false
    }

    fn signature_header(&self) -> &'static str {
        "x-signature"
    }

    async fn create_payment(&self, _: &CheckoutRequest) -> Result<CheckoutSession, PaymentError> {
        Err(PaymentError::NotConfigured)
    }

    async fn get_payment_status(&self, _: &str) -> Result<PaymentUpdate, PaymentError> {
        Err(PaymentError::NotConfigured)
    }

    fn handle_webhook(&self, _: Option<&str>, _: &[u8]) -> Result<PaymentUpdate, PaymentError> {
        Err(PaymentError::NotConfigured)
    }
}

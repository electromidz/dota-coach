//! NOWPayments implementation of [`PaymentProvider`].
//!
//! Scope, as the provider documents it:
//!
//!   - `POST /v1/invoice` opens a hosted checkout page. Body carries
//!     `price_amount` / `price_currency` (the fiat figure we charge),
//!     `order_id` (ours), `ipn_callback_url`, `success_url`, `cancel_url`, and
//!     optionally `pay_currency`. The answer carries `id` and `invoice_url`.
//!   - `GET /v1/payment/{payment_id}` reports the authoritative state of one
//!     charge.
//!   - IPN notifications arrive as a POST carrying `x-nowpayments-sig`: an
//!     HMAC-SHA512 of the JSON body **with its keys sorted**, keyed by the IPN
//!     secret.
//!   - Requests authenticate with an `x-api-key` header. Neither key ever
//!     leaves this module.
//!
//! Two consequences are honoured rather than hidden:
//!
//!   1. An invoice id is not a payment id. Until the first notification
//!      arrives, the id stored against a charge is the invoice's; the
//!      notification carries the real `payment_id` and the billing service
//!      writes it back. Matching is done on `order_id`, which is ours and
//!      stable from the start.
//!   2. `actually_paid` can differ from what was invoiced (under- and
//!      overpayment). This module reports the status verbatim and lets the
//!      billing layer decide; `partially_paid` is never reported as settled.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use hmac::{Hmac, Mac};
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use sha2::Sha512;

use super::{CheckoutRequest, CheckoutSession, PaymentError, PaymentProvider, PaymentUpdate};
use crate::config::BillingConfig;
use crate::domain::billing::PaymentStatus;

pub const PROVIDER: &str = "nowpayments";

/// The header NOWPayments signs its notifications with.
pub const SIGNATURE_HEADER: &str = "x-nowpayments-sig";

pub struct NowPaymentsProvider {
    http: Client,
    base_url: String,
    api_key: Option<String>,
    /// Separate from the API key: this one only ever verifies notifications.
    ipn_secret: Option<String>,
    /// Restrict checkout to one coin when configured; otherwise the hosted
    /// page lets the user choose.
    pay_currency: Option<String>,
}

impl NowPaymentsProvider {
    pub fn new(config: &BillingConfig, timeout: Duration) -> Result<Arc<Self>, reqwest::Error> {
        Ok(Arc::new(Self {
            http: Client::builder().timeout(timeout).build()?,
            base_url: config.base_url.trim_end_matches('/').to_string(),
            api_key: config.api_key.clone(),
            ipn_secret: config.ipn_secret.clone(),
            pay_currency: config.pay_currency.clone(),
        }))
    }

    fn key(&self) -> Result<&str, PaymentError> {
        self.api_key
            .as_deref()
            .filter(|k| !k.is_empty())
            .ok_or(PaymentError::NotConfigured)
    }
}

#[async_trait]
impl PaymentProvider for NowPaymentsProvider {
    fn name(&self) -> &'static str {
        PROVIDER
    }

    /// Both secrets are required. An API key without an IPN secret could open
    /// charges it can never verify the settlement of, which is worse than no
    /// checkout at all.
    fn is_configured(&self) -> bool {
        self.api_key.as_ref().is_some_and(|k| !k.is_empty())
            && self.ipn_secret.as_ref().is_some_and(|s| !s.is_empty())
    }

    fn signature_header(&self) -> &'static str {
        SIGNATURE_HEADER
    }

    async fn create_payment(
        &self,
        request: &CheckoutRequest,
    ) -> Result<CheckoutSession, PaymentError> {
        let key = self.key()?;

        let mut body = serde_json::json!({
            "price_amount": decimal_amount(request.amount_cents),
            "price_currency": request.currency,
            "order_id": request.order_id,
            "order_description": request.description,
            "ipn_callback_url": request.ipn_callback_url,
            "success_url": request.success_url,
            "cancel_url": request.cancel_url,
        });
        if let Some(coin) = &self.pay_currency {
            body["pay_currency"] = serde_json::Value::String(coin.clone());
        }

        let response = self
            .http
            .post(format!("{}/invoice", self.base_url))
            .header("x-api-key", key)
            .json(&body)
            .send()
            .await
            .map_err(|e| PaymentError::Unavailable(e.to_string()))?;

        let invoice: InvoiceResponse = parse(response).await?;

        Ok(CheckoutSession {
            provider_payment_id: invoice.id.to_string(),
            payment_url: invoice.invoice_url,
            status: PaymentStatus::Pending,
        })
    }

    async fn get_payment_status(
        &self,
        provider_payment_id: &str,
    ) -> Result<PaymentUpdate, PaymentError> {
        let key = self.key()?;

        let response = self
            .http
            .get(format!("{}/payment/{provider_payment_id}", self.base_url))
            .header("x-api-key", key)
            .send()
            .await
            .map_err(|e| PaymentError::Unavailable(e.to_string()))?;

        let payment: PaymentResponse = parse(response).await?;
        payment.into_update()
    }

    /// Verify the signature, then read the body.
    ///
    /// In that order, and with no early return in between: an unsigned or
    /// wrongly-signed notification is indistinguishable from an attacker
    /// posting a `finished` status at the endpoint, so it is never parsed for
    /// anything but the log line.
    fn handle_webhook(
        &self,
        signature: Option<&str>,
        body: &[u8],
    ) -> Result<PaymentUpdate, PaymentError> {
        let secret = self
            .ipn_secret
            .as_deref()
            .filter(|s| !s.is_empty())
            .ok_or(PaymentError::NotConfigured)?;

        let signature = signature.ok_or(PaymentError::InvalidSignature)?;
        let payload: serde_json::Value = serde_json::from_slice(body)
            .map_err(|e| PaymentError::InvalidResponse(format!("body is not JSON: {e}")))?;

        verify_signature(secret, &payload, signature)?;

        let payment: PaymentResponse = serde_json::from_value(payload)
            .map_err(|e| PaymentError::InvalidResponse(e.to_string()))?;

        payment.into_update()
    }
}

/// HMAC-SHA512 over the body with its keys sorted, compared in constant time.
///
/// `serde_json`'s object is a `BTreeMap`, so re-serializing the parsed value
/// sorts every level for free — which is exactly the canonical form
/// NOWPayments signs.
fn verify_signature(
    secret: &str,
    payload: &serde_json::Value,
    signature: &str,
) -> Result<(), PaymentError> {
    let canonical =
        serde_json::to_vec(payload).map_err(|e| PaymentError::InvalidResponse(e.to_string()))?;

    let mut mac = Hmac::<Sha512>::new_from_slice(secret.as_bytes())
        .map_err(|_| PaymentError::NotConfigured)?;
    mac.update(&canonical);

    let expected = hex::decode(signature.trim()).map_err(|_| PaymentError::InvalidSignature)?;

    // `verify_slice` is constant-time and also rejects a wrong-length digest.
    mac.verify_slice(&expected)
        .map_err(|_| PaymentError::InvalidSignature)
}

/// Minor units to the decimal figure the provider expects. Integer maths
/// throughout: `amount_cents / 100.0` is exactly the kind of rounding that
/// turns $1.00 into $0.99 on some other machine.
fn decimal_amount(cents: i64) -> serde_json::Value {
    let text = format!("{}.{:02}", cents / 100, (cents % 100).abs());
    serde_json::from_str(&text).unwrap_or(serde_json::Value::String(text))
}

/// The provider's decimal figure back to minor units.
///
/// Accepts both a JSON number and a JSON string because NOWPayments uses both
/// depending on the endpoint. Rounds to the nearest cent — the only place a
/// float is allowed to touch money, and the result is checked against the
/// stored figure before it grants anything.
fn to_cents(value: Option<&serde_json::Value>) -> Option<i64> {
    let amount = match value? {
        serde_json::Value::Number(n) => n.as_f64()?,
        serde_json::Value::String(s) => s.trim().parse().ok()?,
        _ => return None,
    };

    if !amount.is_finite() {
        return None;
    }

    Some((amount * 100.0).round() as i64)
}

#[derive(Debug, Deserialize)]
struct InvoiceResponse {
    /// Numeric in the current API, quoted in older responses; either is fine
    /// as an opaque identifier.
    #[serde(deserialize_with = "id_as_string")]
    id: String,
    invoice_url: Option<String>,
}

/// One charge, as reported by either the status endpoint or an IPN.
#[derive(Debug, Deserialize)]
struct PaymentResponse {
    #[serde(deserialize_with = "id_as_string")]
    payment_id: String,
    payment_status: String,
    order_id: Option<String>,
    price_amount: Option<serde_json::Value>,
    price_currency: Option<String>,
    pay_currency: Option<String>,
}

impl PaymentResponse {
    fn into_update(self) -> Result<PaymentUpdate, PaymentError> {
        let status = map_status(&self.payment_status).ok_or_else(|| {
            PaymentError::InvalidResponse(format!("unknown payment status {}", self.payment_status))
        })?;

        Ok(PaymentUpdate {
            // Payment id plus status: the same charge moving from confirming to
            // finished is two events, but a redelivery of either is one.
            event_key: format!("{}:{}", self.payment_id, status.slug()),
            provider_payment_id: self.payment_id,
            order_id: self.order_id,
            status,
            amount_cents: to_cents(self.price_amount.as_ref()),
            currency: self.price_currency.map(|c| c.to_lowercase()),
            pay_currency: self.pay_currency.map(|c| c.to_lowercase()),
        })
    }
}

/// NOWPayments' vocabulary, mapped to the domain's.
///
/// `partially_paid` is deliberately *not* settled: the user owes the rest, and
/// treating it as paid would hand out a month for part of the price. An unknown
/// status is an error rather than a guess — a new state we have not read the
/// documentation for must not be allowed to mean "paid".
fn map_status(raw: &str) -> Option<PaymentStatus> {
    Some(match raw {
        "waiting" => PaymentStatus::Pending,
        "confirming" | "confirmed" | "sending" | "partially_paid" => PaymentStatus::Confirming,
        "finished" => PaymentStatus::Paid,
        "failed" => PaymentStatus::Failed,
        "expired" => PaymentStatus::Expired,
        "refunded" => PaymentStatus::Refunded,
        _ => return None,
    })
}

/// Identifiers are opaque, but their JSON type is not stable across the
/// provider's endpoints. Accept either and keep the text.
fn id_as_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    match serde_json::Value::deserialize(deserializer)? {
        serde_json::Value::String(s) => Ok(s),
        serde_json::Value::Number(n) => Ok(n.to_string()),
        other => Err(serde::de::Error::custom(format!(
            "expected an identifier, got {other}"
        ))),
    }
}

async fn parse<T: serde::de::DeserializeOwned>(
    response: reqwest::Response,
) -> Result<T, PaymentError> {
    let status = response.status();

    if status == StatusCode::TOO_MANY_REQUESTS {
        return Err(PaymentError::RateLimited);
    }
    if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
        // The key is wrong or revoked. Waiting will not fix it, and the detail
        // must not reach the user.
        tracing::error!(%status, "nowpayments rejected our credentials");
        return Err(PaymentError::NotConfigured);
    }
    if !status.is_success() {
        let detail = response.text().await.unwrap_or_default();
        tracing::warn!(%status, detail = %truncate(&detail), "nowpayments call failed");
        return Err(PaymentError::Unavailable(format!("HTTP {status}")));
    }

    response
        .json()
        .await
        .map_err(|e| PaymentError::InvalidResponse(e.to_string()))
}

/// Provider error bodies are unbounded; log a bounded prefix.
fn truncate(detail: &str) -> String {
    detail.chars().take(300).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &str = "ipn-secret";

    fn sign(secret: &str, body: &str) -> String {
        let value: serde_json::Value = serde_json::from_str(body).unwrap();
        let canonical = serde_json::to_vec(&value).unwrap();

        let mut mac = Hmac::<Sha512>::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(&canonical);
        hex::encode(mac.finalize().into_bytes())
    }

    fn provider() -> NowPaymentsProvider {
        NowPaymentsProvider {
            http: Client::new(),
            base_url: "https://pay.example/v1".into(),
            api_key: Some("key".into()),
            ipn_secret: Some(SECRET.into()),
            pay_currency: None,
        }
    }

    const BODY: &str = r#"{"payment_id":"5745","payment_status":"finished","order_id":"11111111-1111-4111-8111-111111111111","price_amount":1.0,"price_currency":"usd","pay_currency":"usdttrc20"}"#;

    #[test]
    fn a_correctly_signed_notification_is_normalized() {
        let signature = sign(SECRET, BODY);

        let update = provider()
            .handle_webhook(Some(&signature), BODY.as_bytes())
            .expect("a valid signature should be accepted");

        assert_eq!(update.provider_payment_id, "5745");
        assert_eq!(update.status, PaymentStatus::Paid);
        assert_eq!(
            update.order_id.as_deref(),
            Some("11111111-1111-4111-8111-111111111111")
        );
        assert_eq!(update.amount_cents, Some(100));
        assert_eq!(update.currency.as_deref(), Some("usd"));
    }

    #[test]
    fn key_order_in_the_body_does_not_change_the_signature() {
        // The provider sorts keys before signing; we must canonicalize the same
        // way rather than hashing the bytes as they arrived.
        let reordered = r#"{"price_currency":"usd","payment_status":"finished","pay_currency":"usdttrc20","order_id":"11111111-1111-4111-8111-111111111111","price_amount":1.0,"payment_id":"5745"}"#;
        let signature = sign(SECRET, BODY);

        assert!(provider()
            .handle_webhook(Some(&signature), reordered.as_bytes())
            .is_ok());
    }

    #[test]
    fn a_forged_or_missing_signature_is_rejected_before_the_body_is_believed() {
        let p = provider();

        assert!(matches!(
            p.handle_webhook(None, BODY.as_bytes()),
            Err(PaymentError::InvalidSignature)
        ));
        assert!(matches!(
            p.handle_webhook(Some(&sign("other-secret", BODY)), BODY.as_bytes()),
            Err(PaymentError::InvalidSignature)
        ));
        assert!(matches!(
            p.handle_webhook(Some("not-hex"), BODY.as_bytes()),
            Err(PaymentError::InvalidSignature)
        ));
    }

    #[test]
    fn tampering_with_the_amount_invalidates_the_signature() {
        let signature = sign(SECRET, BODY);
        let tampered = BODY.replace("\"price_amount\":1.0", "\"price_amount\":0.01");

        assert!(matches!(
            provider().handle_webhook(Some(&signature), tampered.as_bytes()),
            Err(PaymentError::InvalidSignature)
        ));
    }

    #[test]
    fn a_deployment_without_an_ipn_secret_verifies_nothing() {
        let p = NowPaymentsProvider {
            ipn_secret: None,
            ..provider()
        };

        assert!(!p.is_configured());
        assert!(matches!(
            p.handle_webhook(Some(&sign(SECRET, BODY)), BODY.as_bytes()),
            Err(PaymentError::NotConfigured)
        ));
    }

    #[test]
    fn every_settlement_state_maps_to_exactly_one_domain_status() {
        assert_eq!(map_status("waiting"), Some(PaymentStatus::Pending));
        assert_eq!(map_status("confirming"), Some(PaymentStatus::Confirming));
        assert_eq!(map_status("finished"), Some(PaymentStatus::Paid));
        assert_eq!(map_status("failed"), Some(PaymentStatus::Failed));
        assert_eq!(map_status("expired"), Some(PaymentStatus::Expired));
        assert_eq!(map_status("refunded"), Some(PaymentStatus::Refunded));
        // Underpayment is not payment.
        assert_eq!(
            map_status("partially_paid"),
            Some(PaymentStatus::Confirming)
        );
        // A status this build has never heard of must not become "paid".
        assert_eq!(map_status("settled_somehow"), None);
    }

    #[test]
    fn the_same_charge_twice_produces_the_same_event_key() {
        let signature = sign(SECRET, BODY);
        let p = provider();

        let first = p.handle_webhook(Some(&signature), BODY.as_bytes()).unwrap();
        let second = p.handle_webhook(Some(&signature), BODY.as_bytes()).unwrap();

        assert_eq!(first.event_key, second.event_key);
        assert_eq!(first.event_key, "5745:paid");
    }

    #[test]
    fn a_charge_moving_forward_produces_a_different_event_key() {
        let confirming = BODY.replace("finished", "confirming");
        let p = provider();

        let a = p
            .handle_webhook(Some(&sign(SECRET, BODY)), BODY.as_bytes())
            .unwrap();
        let b = p
            .handle_webhook(Some(&sign(SECRET, &confirming)), confirming.as_bytes())
            .unwrap();

        assert_ne!(a.event_key, b.event_key);
    }

    #[test]
    fn money_survives_the_round_trip_in_both_directions() {
        assert_eq!(decimal_amount(100).to_string(), "1.0");
        assert_eq!(decimal_amount(1_999).to_string(), "19.99");
        assert_eq!(decimal_amount(5).to_string(), "0.05");

        assert_eq!(to_cents(Some(&serde_json::json!(1.0))), Some(100));
        assert_eq!(to_cents(Some(&serde_json::json!("19.99"))), Some(1_999));
        assert_eq!(to_cents(Some(&serde_json::json!(0.05))), Some(5));
        assert_eq!(to_cents(None), None);
        assert_eq!(to_cents(Some(&serde_json::json!("free"))), None);
    }
}

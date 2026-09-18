use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;

use crate::services::billing::BillingError;
use crate::services::dota::ProviderError;
use crate::services::payments::PaymentError;
use crate::services::sync::SyncError;
use crate::services::voucher::VoucherError;
use utoipa::ToSchema;

/// Every error the API can return. Variants carry only what is safe to show a
/// user; the underlying cause is logged, never serialized.
// Phase 1 only has handlers that can fail on the database; the remaining
// variants are exercised as the sync, metrics and LLM endpoints are added.
#[allow(dead_code)]
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    BadRequest(String),

    #[error("{0}")]
    NotFound(String),

    #[error("not authenticated")]
    Unauthenticated,

    /// Authenticated, but disabled by an admin. Distinct from `Unauthenticated`:
    /// the session is valid, the account itself is not usable.
    #[error("account disabled")]
    AccountDisabled,

    /// Authenticated, correctly so, but not allowed to do this. Used only by
    /// the admin surface today — `AdminUser` for a non-admin caller.
    #[error("{0}")]
    Forbidden(String),

    /// Authenticated, but no Dota identity is linked to the account.
    #[error("no Dota account linked")]
    DotaAccountNotLinked,

    #[error("upstream service unavailable: {0}")]
    Upstream(String),

    #[error("{0}")]
    TooManyRequests(String),

    /// The request is well-formed, but the account is not in a state that can
    /// answer it — no synced matches to analyse, for instance.
    #[error("{0}")]
    PreconditionUnmet(String),

    /// The feature exists but is not configured on this deployment. Distinct
    /// from an outage: waiting will not help, and the fix is not the user's.
    #[error("{0}")]
    FeatureUnavailable(String),

    /// Authenticated, but the trial has run out and nothing is paid for.
    /// Nothing is wrong with the request; the account simply is not entitled.
    #[error("{0}")]
    PaymentRequired(String),

    /// The cause is carried in `Display` because that is what reaches the log;
    /// `public_message` stays generic, so nothing of it reaches the client.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("internal error")]
    Internal(String),
}

impl AppError {
    fn parts(&self) -> (StatusCode, &'static str) {
        match self {
            AppError::BadRequest(_) => (StatusCode::BAD_REQUEST, "BAD_REQUEST"),
            AppError::NotFound(_) => (StatusCode::NOT_FOUND, "NOT_FOUND"),
            AppError::Unauthenticated => (StatusCode::UNAUTHORIZED, "UNAUTHENTICATED"),
            AppError::AccountDisabled => (StatusCode::FORBIDDEN, "ACCOUNT_DISABLED"),
            AppError::Forbidden(_) => (StatusCode::FORBIDDEN, "FORBIDDEN"),
            AppError::DotaAccountNotLinked => (StatusCode::CONFLICT, "DOTA_ACCOUNT_NOT_LINKED"),
            AppError::Upstream(_) => (StatusCode::BAD_GATEWAY, "UPSTREAM_UNAVAILABLE"),
            AppError::TooManyRequests(_) => (StatusCode::TOO_MANY_REQUESTS, "RATE_LIMITED"),
            AppError::PreconditionUnmet(_) => (StatusCode::CONFLICT, "PRECONDITION_UNMET"),
            AppError::FeatureUnavailable(_) => {
                (StatusCode::SERVICE_UNAVAILABLE, "FEATURE_UNAVAILABLE")
            }
            AppError::PaymentRequired(_) => (StatusCode::PAYMENT_REQUIRED, "PAYMENT_REQUIRED"),
            AppError::Database(_) => (StatusCode::INTERNAL_SERVER_ERROR, "DATABASE_ERROR"),
            AppError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR"),
        }
    }

    /// User-facing message. Internal failures are deliberately generic so no
    /// stack traces, SQL, or provider payloads reach the client.
    fn public_message(&self) -> String {
        match self {
            AppError::BadRequest(m)
            | AppError::NotFound(m)
            | AppError::TooManyRequests(m)
            | AppError::PreconditionUnmet(m)
            | AppError::FeatureUnavailable(m)
            | AppError::Forbidden(m)
            | AppError::PaymentRequired(m) => m.clone(),
            AppError::Unauthenticated => "Sign in with Steam to continue.".into(),
            AppError::AccountDisabled => {
                "This account has been disabled. Contact support if you think that's wrong.".into()
            }
            AppError::DotaAccountNotLinked => {
                "No Dota account is linked to your Steam profile yet.".into()
            }
            AppError::Upstream(m) => format!("Upstream service unavailable: {m}"),
            AppError::Database(_) => "A database error occurred. Please try again.".into(),
            AppError::Internal(_) => "An unexpected error occurred. Please try again.".into(),
        }
    }
}

/// The envelope every failure uses, without exception — including the 404 for
/// an unknown path. Documented so a client can parse one shape for all errors.
#[derive(Serialize, ToSchema)]
pub(crate) struct ErrorBody {
    pub(crate) error: ErrorDetail,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct ErrorDetail {
    /// Stable machine-readable code, e.g. `UNAUTHENTICATED`, `RATE_LIMITED`.
    #[schema(example = "UNAUTHENTICATED")]
    pub(crate) code: &'static str,
    /// Safe to show a user. Never carries the underlying cause.
    pub(crate) message: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code) = self.parts();

        if status.is_server_error() {
            tracing::error!(error = %self, code, "request failed");
        } else {
            tracing::debug!(error = %self, code, "request rejected");
        }

        let body = ErrorBody {
            error: ErrorDetail {
                code,
                message: self.public_message(),
            },
        };
        (status, Json(body)).into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;

/// Provider failures are translated once, here, so no handler has to decide
/// what an OpenDota outage means in HTTP terms. The provider's own message is
/// logged by `IntoResponse`, never returned.
impl From<ProviderError> for AppError {
    fn from(error: ProviderError) -> Self {
        match error {
            ProviderError::NotFound => {
                AppError::NotFound("No Dota data found for that player.".into())
            }
            ProviderError::RateLimited => AppError::TooManyRequests(
                "The Dota data provider is rate limiting us. Try again in a minute.".into(),
            ),
            ProviderError::Unavailable(detail) => {
                tracing::warn!(detail, "dota provider unavailable");
                AppError::Upstream("the Dota data provider".into())
            }
            ProviderError::Decode(detail) => {
                tracing::warn!(detail, "dota provider returned an unexpected shape");
                AppError::Upstream("the Dota data provider".into())
            }
        }
    }
}

/// Billing failures, translated once. The distinction that matters here is
/// between "this deployment cannot take money" (503, the operator's problem)
/// and "this notification does not describe the charge it names" (400, and
/// nothing is granted).
impl From<BillingError> for AppError {
    fn from(error: BillingError) -> Self {
        match error {
            BillingError::Provider(PaymentError::NotConfigured) => {
                AppError::FeatureUnavailable(PaymentError::NotConfigured.user_note().into())
            }
            BillingError::Provider(PaymentError::InvalidSignature) => {
                // Never explained: an unsigned notification is indistinguishable
                // from someone probing the endpoint.
                AppError::BadRequest("That notification could not be verified.".into())
            }
            BillingError::Provider(PaymentError::RateLimited) => {
                AppError::TooManyRequests(PaymentError::RateLimited.user_note().into())
            }
            BillingError::Provider(e) => {
                tracing::warn!(error = %e, "payment provider failed");
                AppError::Upstream("the payment provider".into())
            }
            BillingError::UnknownPayment => {
                AppError::NotFound("No charge matches that notification.".into())
            }
            BillingError::AmountMismatch => {
                AppError::BadRequest("That notification does not match the charge it names.".into())
            }
            BillingError::Database(e) => AppError::Database(e),
        }
    }
}

impl From<SyncError> for AppError {
    fn from(error: SyncError) -> Self {
        match error {
            SyncError::Provider(e) => e.into(),
            SyncError::Database(e) => AppError::Database(e),
        }
    }
}

/// A voucher code is user-typed free text, not a system-generated id, so an
/// unknown one is `400` — the caller asked for something that does not
/// exist, not a resource that used to. Having already redeemed a code is
/// `409`: nothing is wrong with the request, the account's own history is
/// just why it cannot be honoured again.
impl From<VoucherError> for AppError {
    fn from(error: VoucherError) -> Self {
        match error {
            VoucherError::NotFound => AppError::BadRequest("That code is not valid.".into()),
            VoucherError::Inactive => {
                AppError::BadRequest("That code is no longer active.".into())
            }
            VoucherError::Expired => AppError::BadRequest("That code has expired.".into()),
            VoucherError::UsedUp => AppError::BadRequest(
                "That code has already been used the maximum number of times.".into(),
            ),
            VoucherError::AlreadyRedeemed => {
                AppError::PreconditionUnmet("You have already redeemed this code.".into())
            }
            VoucherError::RateLimited => {
                AppError::TooManyRequests("Too many attempts — try again in a minute.".into())
            }
            VoucherError::Database(e) => AppError::Database(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_errors_keep_their_message() {
        let error = AppError::NotFound("Player not found.".into());
        assert_eq!(error.parts().0, StatusCode::NOT_FOUND);
        assert_eq!(error.public_message(), "Player not found.");
    }

    #[test]
    fn internal_details_are_never_exposed() {
        let error = AppError::Internal("panic at repositories/player.rs:42".into());
        assert_eq!(error.parts().0, StatusCode::INTERNAL_SERVER_ERROR);
        assert!(!error.public_message().contains("repositories"));
    }

    #[test]
    fn database_errors_map_to_a_generic_message() {
        let error = AppError::Database(sqlx::Error::RowNotFound);
        assert_eq!(error.parts().1, "DATABASE_ERROR");
        assert!(!error.public_message().to_lowercase().contains("sql"));
    }

    #[test]
    fn provider_outages_become_bad_gateway_without_leaking_the_cause() {
        let error: AppError =
            ProviderError::Unavailable("dns failure for api.opendota.com".into()).into();

        assert_eq!(error.parts().0, StatusCode::BAD_GATEWAY);
        assert!(!error.public_message().contains("dns"));
    }

    #[test]
    fn provider_rate_limits_surface_as_429() {
        let error: AppError = ProviderError::RateLimited.into();
        assert_eq!(error.parts().0, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(error.parts().1, "RATE_LIMITED");
    }

    #[test]
    fn a_missing_account_at_the_provider_is_a_404() {
        let error: AppError = ProviderError::NotFound.into();
        assert_eq!(error.parts().0, StatusCode::NOT_FOUND);
    }

    #[test]
    fn an_anonymous_request_is_a_401_with_a_stable_code() {
        let error = AppError::Unauthenticated;
        assert_eq!(error.parts().0, StatusCode::UNAUTHORIZED);
        assert_eq!(error.parts().1, "UNAUTHENTICATED");
    }

    #[test]
    fn an_unlinked_dota_account_is_distinguishable_from_not_found() {
        let error = AppError::DotaAccountNotLinked;
        assert_eq!(error.parts().0, StatusCode::CONFLICT);
        assert_eq!(error.parts().1, "DOTA_ACCOUNT_NOT_LINKED");
    }
}

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;

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

    #[error("upstream service unavailable: {0}")]
    Upstream(String),

    #[error("database error")]
    Database(#[from] sqlx::Error),

    #[error("internal error")]
    Internal(String),
}

impl AppError {
    fn parts(&self) -> (StatusCode, &'static str) {
        match self {
            AppError::BadRequest(_) => (StatusCode::BAD_REQUEST, "BAD_REQUEST"),
            AppError::NotFound(_) => (StatusCode::NOT_FOUND, "NOT_FOUND"),
            AppError::Upstream(_) => (StatusCode::BAD_GATEWAY, "UPSTREAM_UNAVAILABLE"),
            AppError::Database(_) => (StatusCode::INTERNAL_SERVER_ERROR, "DATABASE_ERROR"),
            AppError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR"),
        }
    }

    /// User-facing message. Internal failures are deliberately generic so no
    /// stack traces, SQL, or provider payloads reach the client.
    fn public_message(&self) -> String {
        match self {
            AppError::BadRequest(m) | AppError::NotFound(m) => m.clone(),
            AppError::Upstream(m) => format!("Upstream service unavailable: {m}"),
            AppError::Database(_) => "A database error occurred. Please try again.".into(),
            AppError::Internal(_) => "An unexpected error occurred. Please try again.".into(),
        }
    }
}

#[derive(Serialize)]
struct ErrorBody {
    error: ErrorDetail,
}

#[derive(Serialize)]
struct ErrorDetail {
    code: &'static str,
    message: String,
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
}

//! Extractors that fail the way the rest of the API fails.
//!
//! Axum's built-in rejections answer with plain text and the serde error
//! verbatim, which breaks the single error envelope and leaks internals. These
//! wrappers keep the same ergonomics and route every rejection through
//! [`AppError`].

use axum::extract::rejection::{PathRejection, QueryRejection};
use axum::extract::{FromRef, FromRequestParts};
use axum::http::request::Parts;
use axum_extra::extract::CookieJar;

use crate::domain::session::{hash_token, SESSION_COOKIE};
use crate::domain::user::User;
use crate::error::AppError;
use crate::repositories;
use crate::state::AppState;

/// The authenticated account.
///
/// This is the **only** way a handler learns who is calling. Identity comes
/// from the session cookie, is resolved against the database on every request,
/// and no request body, query parameter or path segment can influence it.
pub struct CurrentUser(pub User);

impl<S> FromRequestParts<S> for CurrentUser
where
    AppState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let app_state = AppState::from_ref(state);

        let jar = CookieJar::from_headers(&parts.headers);
        let token = jar
            .get(SESSION_COOKIE)
            .map(|cookie| cookie.value().to_string())
            .ok_or(AppError::Unauthenticated)?;

        // Sessions are stored hashed, and the lookup filters on expiry, so an
        // expired cookie is indistinguishable from an unknown one.
        let user =
            repositories::session::find_user_by_token_hash(&app_state.db, &hash_token(&token))
                .await?
                .ok_or(AppError::Unauthenticated)?;

        Ok(Self(user))
    }
}

/// `Path<T>` with a JSON rejection.
pub struct AppPath<T>(pub T);

impl<S, T> FromRequestParts<S> for AppPath<T>
where
    S: Send + Sync,
    axum::extract::Path<T>: FromRequestParts<S, Rejection = PathRejection>,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        match axum::extract::Path::<T>::from_request_parts(parts, state).await {
            Ok(axum::extract::Path(value)) => Ok(Self(value)),
            Err(rejection) => {
                tracing::debug!(detail = %rejection, "path rejected");
                Err(AppError::BadRequest(
                    "That identifier is not valid.".to_string(),
                ))
            }
        }
    }
}

/// `Query<T>` with a JSON rejection.
pub struct AppQuery<T>(pub T);

impl<S, T> FromRequestParts<S> for AppQuery<T>
where
    S: Send + Sync,
    axum::extract::Query<T>: FromRequestParts<S, Rejection = QueryRejection>,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        match axum::extract::Query::<T>::from_request_parts(parts, state).await {
            Ok(axum::extract::Query(value)) => Ok(Self(value)),
            Err(rejection) => {
                tracing::debug!(detail = %rejection, "query rejected");
                Err(AppError::BadRequest(
                    "One of the query parameters is not valid.".to_string(),
                ))
            }
        }
    }
}

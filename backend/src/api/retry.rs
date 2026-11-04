//! One retry for a read that hit a transient database failure.
//!
//! A pooler that is momentarily full, or a connection it dropped, fails one
//! request and not the next. For a GET that is safe to repeat, so the server
//! repeats it once after a short pause instead of handing the user an error
//! page. Anything that writes is never retried here: a POST that failed
//! halfway may already have taken effect.

use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use axum::body::Body;
use axum::extract::Request;
use axum::http::Method;
use axum::response::Response;
use tower::{Layer, Service, ServiceExt};

/// Set on a response by `AppError` when the failure is worth one retry.
#[derive(Debug, Clone, Copy)]
pub struct Transient;

/// Pause before the retry: long enough for a pooler slot to free up.
const PAUSE: Duration = Duration::from_millis(250);

#[derive(Clone, Default)]
pub struct RetryTransientLayer;

impl<S> Layer<S> for RetryTransientLayer {
    type Service = RetryTransient<S>;
    fn layer(&self, inner: S) -> Self::Service {
        RetryTransient { inner }
    }
}

#[derive(Clone)]
pub struct RetryTransient<S> {
    inner: S,
}

impl<S> Service<Request> for RetryTransient<S>
where
    S: Service<Request, Response = Response, Error = Infallible> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = Response;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Response, Infallible>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: Request) -> Self::Future {
        // Take the ready service, leave a fresh clone behind (tower's rule).
        let clone = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, clone);

        let replay = matches!(*request.method(), Method::GET | Method::HEAD).then(|| {
            let mut copy = Request::new(Body::empty());
            *copy.method_mut() = request.method().clone();
            *copy.uri_mut() = request.uri().clone();
            *copy.version_mut() = request.version();
            *copy.headers_mut() = request.headers().clone();
            *copy.extensions_mut() = request.extensions().clone();
            copy
        });
        let retry_with = self.inner.clone();

        Box::pin(async move {
            let response = inner.call(request).await?;
            match replay {
                Some(copy) if response.extensions().get::<Transient>().is_some() => {
                    tracing::warn!("transient database failure on a read; retrying once");
                    tokio::time::sleep(PAUSE).await;
                    retry_with.oneshot(copy).await
                }
                _ => Ok(response),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::routing::{get, post};
    use axum::Router;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    fn app(calls: Arc<AtomicUsize>) -> Router {
        let flaky = move || {
            let calls = calls.clone();
            async move {
                if calls.fetch_add(1, Ordering::SeqCst) == 0 {
                    let mut r = Response::new(Body::from("busy"));
                    *r.status_mut() = axum::http::StatusCode::SERVICE_UNAVAILABLE;
                    r.extensions_mut().insert(Transient);
                    r
                } else {
                    Response::new(Body::from("ok"))
                }
            }
        };
        Router::new()
            .route("/read", get(flaky.clone()))
            .route("/write", post(flaky))
            .layer(RetryTransientLayer)
    }

    #[tokio::test]
    async fn a_read_is_retried_once_and_a_write_never() {
        let calls = Arc::new(AtomicUsize::new(0));
        let r = app(calls.clone())
            .oneshot(Request::get("/read").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(r.status(), axum::http::StatusCode::OK);
        assert_eq!(calls.load(Ordering::SeqCst), 2);

        let calls = Arc::new(AtomicUsize::new(0));
        let r = app(calls.clone())
            .oneshot(Request::post("/write").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(r.status(), axum::http::StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
}

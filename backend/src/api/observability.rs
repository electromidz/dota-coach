//! Request identity and the one log line per request.
//!
//! Every request gets an id — the caller's if it sent a usable one, a fresh one
//! otherwise — which is attached to a span around the whole request and echoed
//! back in `x-request-id`. That is what makes a user's "it failed at 14:05"
//! findable: the id in their network tab appears on every log line the request
//! produced, including the ones a handler emitted three layers down.
//!
//! An id from the outside is treated as untrusted input: it lands in log lines,
//! so anything that is not a short, plain token is replaced rather than
//! sanitized in place.

use std::time::Instant;

use axum::extract::Request;
use axum::http::{HeaderName, HeaderValue};
use axum::middleware::Next;
use axum::response::Response;
use tracing::Instrument;
use uuid::Uuid;

pub const REQUEST_ID_HEADER: HeaderName = HeaderName::from_static("x-request-id");

/// Ceiling on an id we are willing to repeat into our own logs.
const MAX_ID_LENGTH: usize = 64;
const MIN_ID_LENGTH: usize = 8;

/// The id of the request being served, for anything that wants to mention it.
#[derive(Debug, Clone)]
pub struct RequestId(pub String);

/// Tag the request, time it, and log its outcome once.
pub async fn request_id(mut request: Request, next: Next) -> Response {
    let id = usable_id(
        request
            .headers()
            .get(&REQUEST_ID_HEADER)
            .and_then(|value| value.to_str().ok()),
    )
    .unwrap_or_else(|| Uuid::new_v4().to_string());

    let method = request.method().clone();
    let path = request.uri().path().to_string();

    request.extensions_mut().insert(RequestId(id.clone()));

    // The outermost layer, so every event from the timeout layer, the trace
    // layer and the handler itself inherits the id.
    let span = tracing::info_span!("request", request_id = %id, %method, path = %path);

    let started = Instant::now();
    let mut response = next.run(request).instrument(span.clone()).await;
    let latency_ms = started.elapsed().as_millis();

    span.in_scope(|| {
        let status = response.status().as_u16();
        // A 5xx is the operator's problem and should be visible at the default
        // filter; everything else is routine traffic.
        if response.status().is_server_error() {
            tracing::error!(status, latency_ms, "request completed");
        } else {
            tracing::info!(status, latency_ms, "request completed");
        }
    });

    if let Ok(value) = HeaderValue::from_str(&id) {
        response.headers_mut().insert(REQUEST_ID_HEADER, value);
    }

    response
}

/// Whether an inbound id can be repeated verbatim.
///
/// Rejects anything that is not a plain token of a sensible length: a log line
/// carrying a newline or a control character from a request header is a
/// forgery waiting to happen, and a kilobyte of it is a cheap way to bloat
/// every log this request touches.
fn usable_id(raw: Option<&str>) -> Option<String> {
    let candidate = raw?.trim();

    if candidate.len() < MIN_ID_LENGTH || candidate.len() > MAX_ID_LENGTH {
        return None;
    }
    if !candidate
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    {
        return None;
    }

    Some(candidate.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plain_correlation_id_is_kept_so_traces_join_up() {
        let id = "0195f0c5-9a7c-7c1e-9b1a-5f0e7a4c2b31";
        assert_eq!(usable_id(Some(id)).as_deref(), Some(id));
        assert_eq!(
            usable_id(Some("  req_01HZY8  ")).as_deref(),
            Some("req_01HZY8"),
            "surrounding whitespace is formatting, not content"
        );
    }

    #[test]
    fn anything_that_could_forge_a_log_line_is_replaced() {
        for hostile in [
            "abcdefgh\nERROR payment settled",
            "abcdefgh\r\nx",
            "abcd efgh",
            "abcdefgh\u{1b}[31m",
            "id=<script>alert(1)</script>",
        ] {
            assert_eq!(
                usable_id(Some(hostile)),
                None,
                "{hostile:?} must be dropped"
            );
        }
    }

    #[test]
    fn an_absent_or_unreasonable_id_is_declined() {
        assert_eq!(usable_id(None), None);
        assert_eq!(usable_id(Some("")), None);
        assert_eq!(usable_id(Some("short")), None);
        assert_eq!(usable_id(Some(&"a".repeat(MAX_ID_LENGTH + 1))), None);
        // Exactly at the bounds is fine.
        assert!(usable_id(Some(&"a".repeat(MAX_ID_LENGTH))).is_some());
        assert!(usable_id(Some(&"a".repeat(MIN_ID_LENGTH))).is_some());
    }
}

//! The OpenAPI document, and the Swagger UI that renders it.
//!
//! The spec is derived from the handlers themselves rather than maintained
//! beside them: every path here is a `#[utoipa::path]` on the function that
//! actually serves it, and every schema is a `ToSchema` on the type that is
//! actually serialized. A response that changes shape changes the spec in the
//! same commit, and a route that is documented but deleted fails the build.
//!
//! Authentication is a session cookie, which Swagger UI's "Authorize" button
//! cannot set — a cookie-scheme credential is not something JavaScript may
//! write for an HTTP-only cookie. Sign in through `/api/auth/steam` in the same
//! browser first; the cookie then rides along with every "Try it out" call.

use utoipa::openapi::security::{ApiKey, ApiKeyValue, SecurityScheme};
use utoipa::{Modify, OpenApi};
use utoipa_swagger_ui::SwaggerUi;

use crate::domain::session::SESSION_COOKIE;

/// Where the raw document is served. Swagger UI reads it from here, and so can
/// a code generator.
pub const SPEC_PATH: &str = "/api-docs/openapi.json";

/// Where the rendered UI lives.
pub const UI_PATH: &str = "/docs";

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Dota Coach API",
        description = "Personal AI coach for Dota 2.\n\n\
Every number this API returns is computed deterministically in Rust — GPM, \
XPM, KDA, percentiles, benchmarks, win rates and sample sizes included. The \
language model is an interpretation layer over those numbers and is never \
their source: an insight that cites evidence the server did not produce is \
discarded before it reaches a response.\n\n\
**Authentication** is a server-side session established through Steam OpenID, \
carried in an HTTP-only cookie. The SteamID is derived from Steam's signed \
response and is never accepted from a client.\n\n\
**Errors** all share one envelope — see `ErrorBody`. The `code` field is \
stable and worth branching on; `message` is safe to show a user.",
        version = env!("CARGO_PKG_VERSION"),
        license(name = "Proprietary"),
    ),
    modifiers(&SessionCookie),
    tags(
        (name = "health", description = "Liveness and readiness probes. No session required."),
        (name = "auth", description = "Steam OpenID sign-in and session lifecycle."),
        (name = "players", description = "The linked Dota account and match synchronisation."),
        (name = "stats", description = "Aggregated performance, computed in SQL."),
        (name = "benchmark", description = "Player metrics against peers. Percentiles are withheld, never estimated, when the sample is too small."),
        (name = "heroes", description = "Hero pool, meta strength and fit-scored recommendations."),
        (name = "matches", description = "Stored match history and per-match metrics."),
        (name = "coaching", description = "Evidence, insights, the long-term player model and the current training focus. Generation is the only rate-limited verb in the API."),
        (name = "billing", description = "Trial, entitlement, subscription and payment callbacks. The server is the only source of truth for access."),
        (name = "events", description = "Client-reported analytics the backend cannot observe on its own."),
        (name = "admin", description = "Usage, trial and revenue reporting, and account moderation. Requires users.is_admin."),
    ),
    paths(
        crate::api::handlers::health::health,
        crate::api::handlers::health::liveness,
        crate::api::handlers::auth::login,
        crate::api::handlers::auth::callback,
        crate::api::handlers::auth::me,
        crate::api::handlers::auth::logout,
        crate::api::handlers::players::me,
        crate::api::handlers::players::sync,
        crate::api::handlers::stats::get,
        crate::api::handlers::benchmark::overview,
        crate::api::handlers::benchmark::metric,
        crate::api::handlers::calibration::get,
        crate::api::handlers::rank_distribution::get,
        crate::api::handlers::heroes::pool,
        crate::api::handlers::heroes::recommendations,
        crate::api::handlers::heroes::intelligence,
        crate::api::handlers::matches::list,
        crate::api::handlers::matches::get,
        crate::api::handlers::matches::comparison,
        crate::api::handlers::coach::get,
        crate::api::handlers::coach::roles,
        crate::api::handlers::coach::select_role,
        crate::api::handlers::coach::analyze,
        crate::api::handlers::coach::player_model,
        crate::api::handlers::coach::training_focus,
        crate::api::handlers::sessions::list,
        crate::api::handlers::sessions::get,
        crate::api::handlers::sessions::progress,
        crate::api::handlers::conversation::get,
        crate::api::handlers::conversation::ask,
        crate::api::handlers::coach::match_analysis,
        crate::api::handlers::coach::analyze_match,
        crate::api::handlers::billing::overview,
        crate::api::handlers::billing::plan,
        crate::api::handlers::billing::subscription,
        crate::api::handlers::billing::payments,
        crate::api::handlers::billing::checkout,
        crate::api::handlers::billing::webhook,
        crate::api::handlers::subscribe::redeem,
        crate::api::handlers::events::page_view,
        crate::api::handlers::admin::stats,
        crate::api::handlers::admin::list_users,
        crate::api::handlers::admin::get_user,
        crate::api::handlers::admin::extend,
        crate::api::handlers::admin::disable_user,
        crate::api::handlers::admin::enable_user,
        crate::api::handlers::admin::create_vouchers,
        crate::api::handlers::admin::list_vouchers,
        crate::api::handlers::admin::get_voucher,
        crate::api::handlers::admin::deactivate_voucher,
        crate::api::handlers::admin::list_audit_log,
    )
)]
pub struct ApiDoc;

/// Declares the session cookie as the API's one security scheme.
///
/// A modifier rather than an attribute because the cookie's name is a domain
/// constant, and duplicating it here is exactly the kind of drift this module
/// exists to prevent.
struct SessionCookie;

impl Modify for SessionCookie {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "session",
                SecurityScheme::ApiKey(ApiKey::Cookie(ApiKeyValue::with_description(
                    SESSION_COOKIE,
                    "Set by the Steam callback. HTTP-only, so it cannot be read \
                     or written from JavaScript — sign in in this browser and \
                     it is sent automatically.",
                ))),
            );
        }
    }
}

/// The Swagger UI router, serving the document beside it.
pub fn router<S>() -> SwaggerUi
where
    S: Clone + Send + Sync + 'static,
{
    SwaggerUi::new(UI_PATH).url(SPEC_PATH, ApiDoc::openapi())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The rendered document, as a client actually receives it.
    ///
    /// Asserting against the serialized JSON rather than utoipa's types keeps
    /// these tests about the contract we publish, not about the shape of the
    /// library that happens to build it.
    fn spec() -> serde_json::Value {
        serde_json::to_value(ApiDoc::openapi()).expect("the document serializes")
    }

    /// Every operation under a path, paired with its verb.
    fn operations(item: &serde_json::Value) -> Vec<(&str, &serde_json::Value)> {
        ["get", "post", "put", "patch", "delete"]
            .into_iter()
            .filter_map(|verb| item.get(verb).map(|op| (verb, op)))
            .collect()
    }

    fn demands_session(operation: &serde_json::Value) -> bool {
        operation
            .get("security")
            .and_then(|s| s.as_array())
            .is_some_and(|reqs| reqs.iter().any(|req| req.get("session").is_some()))
    }

    /// A path listed twice, or a handler that was never added to `paths(...)`,
    /// is silent otherwise.
    #[test]
    fn the_document_covers_every_route() {
        let spec = spec();
        let paths = spec["paths"].as_object().expect("paths object");

        // 51 handlers; `health` is mounted at two URLs but documented once,
        // and `/admin/vouchers` carries two handlers (GET and POST) under
        // one path — and `/coach/conversation` carries a GET and a POST — 49
        // unique path strings.
        assert_eq!(paths.len(), 49, "every routed handler is documented");
    }

    #[test]
    fn every_documented_operation_has_a_response_and_a_summary() {
        let spec = spec();

        for (path, item) in spec["paths"].as_object().unwrap() {
            let ops = operations(item);
            assert!(!ops.is_empty(), "{path} documents no operation");

            for (verb, operation) in ops {
                assert!(
                    operation["responses"]
                        .as_object()
                        .is_some_and(|r| !r.is_empty()),
                    "{verb} {path} documents no response"
                );
                assert!(
                    operation.get("summary").is_some(),
                    "{verb} {path} has no summary"
                );
            }
        }
    }

    #[test]
    fn the_session_scheme_matches_the_cookie_the_server_reads() {
        let spec = spec();
        let scheme = &spec["components"]["securitySchemes"]["session"];

        assert_eq!(scheme["type"], "apiKey");
        assert_eq!(scheme["in"], "cookie");
        assert_eq!(
            scheme["name"], SESSION_COOKIE,
            "the documented cookie is the one the server actually reads"
        );
    }

    /// A route that needs a session must say so, or a reader will believe it
    /// is public and a generated client will omit the credential.
    #[test]
    fn authenticated_routes_declare_the_session_requirement() {
        let spec = spec();

        for path in [
            "/api/stats",
            "/api/coach",
            "/api/coach/analyze",
            "/api/coach/player-model",
            "/api/players/me",
            "/api/players/me/sync",
            "/api/billing",
            "/api/matches",
            "/api/admin/stats",
            "/api/admin/users",
            "/api/admin/users/{id}",
        ] {
            let item = &spec["paths"][path];
            assert!(!item.is_null(), "{path} is not documented");

            for (verb, operation) in operations(item) {
                assert!(
                    demands_session(operation),
                    "{verb} {path} does not declare a session"
                );
            }
        }
    }

    /// The inverse, and the more dangerous direction to get wrong: a route a
    /// signed-out browser must reach cannot be documented as requiring one.
    #[test]
    fn public_routes_declare_no_session_requirement() {
        let spec = spec();

        for path in [
            "/health",
            "/health/live",
            "/api/auth/steam",
            "/api/auth/steam/callback",
            "/api/billing/plan",
            "/api/billing/webhook",
        ] {
            let item = &spec["paths"][path];
            assert!(!item.is_null(), "{path} is not documented");

            for (verb, operation) in operations(item) {
                assert!(
                    !demands_session(operation),
                    "{verb} {path} is public but demands a session"
                );
            }
        }
    }

    /// The error envelope is the one contract every client needs and the one
    /// most easily lost when a response is re-documented.
    #[test]
    fn the_error_envelope_is_published() {
        let spec = spec();
        let schemas = &spec["components"]["schemas"];

        assert!(!schemas["ErrorBody"].is_null(), "ErrorBody is published");
        assert!(
            !schemas["ErrorDetail"]["properties"]["code"].is_null(),
            "the stable `code` field is documented"
        );
    }

    /// A parameter with no example is a parameter a reader has to guess at,
    /// and "Try it out" opens with an empty box.
    #[test]
    fn every_parameter_is_described_and_carries_an_example() {
        let spec = spec();

        let mut seen = 0;
        for (path, item) in spec["paths"].as_object().unwrap() {
            for (verb, operation) in operations(item) {
                for parameter in operation
                    .get("parameters")
                    .and_then(|p| p.as_array())
                    .unwrap_or(&Vec::new())
                {
                    let name = parameter["name"].as_str().unwrap_or("?");
                    assert!(
                        parameter.get("description").is_some(),
                        "{verb} {path} parameter `{name}` has no description"
                    );
                    assert!(
                        parameter.get("example").is_some(),
                        "{verb} {path} parameter `{name}` has no example"
                    );
                    seen += 1;
                }
            }
        }

        assert_eq!(seen, 44, "every documented parameter was checked");
    }

    /// A path parameter the server requires must be required in the document,
    /// or a generated client will happily build a request to `/matches//`.
    #[test]
    fn path_parameters_are_required_and_query_parameters_are_not() {
        let spec = spec();

        for (path, item) in spec["paths"].as_object().unwrap() {
            for (verb, operation) in operations(item) {
                for parameter in operation
                    .get("parameters")
                    .and_then(|p| p.as_array())
                    .unwrap_or(&Vec::new())
                {
                    let name = parameter["name"].as_str().unwrap_or("?");
                    let required = parameter["required"].as_bool().unwrap_or(false);

                    match parameter["in"].as_str() {
                        Some("path") => assert!(
                            required,
                            "{verb} {path} path parameter `{name}` is optional"
                        ),
                        Some("query") => assert!(
                            !required,
                            "{verb} {path} query parameter `{name}` is required"
                        ),
                        other => panic!("{verb} {path} parameter `{name}` is in {other:?}"),
                    }
                }
            }
        }
    }

    /// Generation is rate limited and can fail upstream. A client that does
    /// not know 429 and 503 are possible will treat them as bugs.
    #[test]
    fn the_generating_routes_document_their_failure_modes() {
        let spec = spec();

        for path in ["/api/coach/analyze", "/api/matches/{id}/analyze"] {
            let responses = &spec["paths"][path]["post"]["responses"];
            for status in ["200", "429", "502", "503"] {
                assert!(
                    !responses[status].is_null(),
                    "{path} does not document {status}"
                );
            }
        }
    }
}

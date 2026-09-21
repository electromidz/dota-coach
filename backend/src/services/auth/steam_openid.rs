//! Steam OpenID 2.0 login.
//!
//! Steam speaks OpenID 2.0, not OIDC. The flow is:
//!
//! 1. Redirect the browser to Steam with `openid.mode=checkid_setup`.
//! 2. Steam redirects back to `return_to` with a signed assertion.
//! 3. We hand every received parameter back to Steam with
//!    `openid.mode=check_authentication`; Steam answers `is_valid:true`.
//!
//! Step 3 is not optional. The redirect parameters are attacker-controlled
//! until Steam confirms its own signature over them.

use std::collections::BTreeMap;

use async_trait::async_trait;
use reqwest::{Client, Url};

use crate::config::AuthConfig;

/// Path Steam is told to send the user back to.
pub const CALLBACK_PATH: &str = "/api/auth/steam/callback";

const OPENID_NS: &str = "http://specs.openid.net/auth/2.0";
const IDENTIFIER_SELECT: &str = "http://specs.openid.net/auth/2.0/identifier_select";
const CLAIMED_ID_PREFIX: &str = "https://steamcommunity.com/openid/id/";

#[derive(Debug, thiserror::Error)]
pub enum OpenIdError {
    #[error("steam rejected the assertion")]
    InvalidAssertion,
    #[error("assertion is malformed: {0}")]
    Malformed(&'static str),
    #[error("steam unreachable: {0}")]
    Unavailable(String),
}

/// Verifying an assertion is an outbound HTTP call, so it sits behind a trait:
/// tests inject a stub instead of reaching Valve.
#[async_trait]
pub trait SteamVerifier: Send + Sync {
    /// Confirm the assertion with Steam and return the proven SteamID64.
    async fn verify(&self, params: &BTreeMap<String, String>) -> Result<i64, OpenIdError>;
}

pub struct SteamOpenId {
    http: Client,
    openid_url: String,
    return_to: String,
    realm: String,
}

impl SteamOpenId {
    pub fn new(http: Client, config: &AuthConfig) -> Self {
        Self {
            http,
            openid_url: config.steam_openid_url.clone(),
            return_to: format!("{}{}", config.public_base_url, CALLBACK_PATH),
            realm: config.public_base_url.clone(),
        }
    }

    /// URL to send the browser to. `state` rides along on `return_to` so the
    /// callback can prove the login started here.
    pub fn authorization_url(&self, state: &str) -> Result<String, OpenIdError> {
        let return_to = format!("{}?state={}", self.return_to, urlencode(state));

        let url = Url::parse_with_params(
            &self.openid_url,
            &[
                ("openid.ns", OPENID_NS),
                ("openid.mode", "checkid_setup"),
                ("openid.return_to", &return_to),
                ("openid.realm", &self.realm),
                ("openid.identity", IDENTIFIER_SELECT),
                ("openid.claimed_id", IDENTIFIER_SELECT),
            ],
        )
        .map_err(|_| OpenIdError::Malformed("could not build the Steam login URL"))?;

        Ok(url.to_string())
    }

    /// The `return_to` this instance advertises, minus the state parameter.
    pub fn return_to_prefix(&self) -> &str {
        &self.return_to
    }
}

#[async_trait]
impl SteamVerifier for SteamOpenId {
    async fn verify(&self, params: &BTreeMap<String, String>) -> Result<i64, OpenIdError> {
        // Reject anything not addressed to us before spending a network call.
        let return_to = params
            .get("openid.return_to")
            .ok_or(OpenIdError::Malformed("missing openid.return_to"))?;
        if !return_to.starts_with(self.return_to_prefix()) {
            return Err(OpenIdError::Malformed("openid.return_to is not ours"));
        }

        let claimed_id = params
            .get("openid.claimed_id")
            .ok_or(OpenIdError::Malformed("missing openid.claimed_id"))?;
        let steam_id = parse_claimed_id(claimed_id)?;

        let form = check_authentication_form(params);

        let response = self
            .http
            .post(&self.openid_url)
            .form(&form)
            .send()
            .await
            .map_err(|e| OpenIdError::Unavailable(e.to_string()))?;

        if !response.status().is_success() {
            return Err(OpenIdError::Unavailable(format!(
                "HTTP {}",
                response.status()
            )));
        }

        let body = response
            .text()
            .await
            .map_err(|e| OpenIdError::Unavailable(e.to_string()))?;

        if is_valid_response(&body) {
            Ok(steam_id)
        } else {
            Err(OpenIdError::InvalidAssertion)
        }
    }
}

/// Echo the assertion back with the mode swapped, which is what OpenID 2.0
/// direct verification requires.
fn check_authentication_form(params: &BTreeMap<String, String>) -> Vec<(String, String)> {
    params
        .iter()
        .map(|(key, value)| {
            if key == "openid.mode" {
                (key.clone(), "check_authentication".to_string())
            } else {
                (key.clone(), value.clone())
            }
        })
        .collect()
}

/// Steam answers with a tiny key-value body; only `is_valid:true` counts.
fn is_valid_response(body: &str) -> bool {
    body.lines()
        .filter_map(|line| line.split_once(':'))
        .any(|(key, value)| key.trim() == "is_valid" && value.trim() == "true")
}

/// `https://steamcommunity.com/openid/id/76561198047011640` -> `76561198047011640`.
///
/// The prefix is pinned: a claimed id pointing anywhere else is not a Steam
/// identity, however well-formed it looks.
pub fn parse_claimed_id(claimed_id: &str) -> Result<i64, OpenIdError> {
    let digits = claimed_id
        .strip_prefix(CLAIMED_ID_PREFIX)
        .ok_or(OpenIdError::Malformed("claimed_id is not a Steam identity"))?;

    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err(OpenIdError::Malformed("claimed_id is not a SteamID64"));
    }

    digits
        .parse()
        .map_err(|_| OpenIdError::Malformed("claimed_id is not a SteamID64"))
}

fn urlencode(value: &str) -> String {
    // The state is hex, but encode anyway so the contract does not depend on that.
    value
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            other => format!("%{other:02X}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> AuthConfig {
        AuthConfig {
            public_base_url: "http://localhost:8080".into(),
            frontend_base_url: "http://localhost:3000".into(),
            steam_openid_url: "https://steamcommunity.com/openid/login".into(),
            session_ttl_hours: 720,
            cookie_secure: false,
            cookie_cross_site: false,
        }
    }

    fn provider() -> SteamOpenId {
        SteamOpenId::new(Client::new(), &config())
    }

    #[test]
    fn a_steam_claimed_id_yields_the_steam_id() {
        let steam_id =
            parse_claimed_id("https://steamcommunity.com/openid/id/76561198047011640").unwrap();
        assert_eq!(steam_id, 76_561_198_047_011_640);
    }

    #[test]
    fn a_claimed_id_from_another_host_is_rejected() {
        // The whole point of pinning the prefix.
        for hostile in [
            "https://evil.example/openid/id/76561198047011640",
            "http://steamcommunity.com/openid/id/76561198047011640",
            "https://steamcommunity.com.evil.example/openid/id/76561198047011640",
            "76561198047011640",
        ] {
            assert!(
                parse_claimed_id(hostile).is_err(),
                "should have rejected {hostile}"
            );
        }
    }

    #[test]
    fn a_non_numeric_claimed_id_is_rejected() {
        assert!(parse_claimed_id("https://steamcommunity.com/openid/id/").is_err());
        assert!(parse_claimed_id("https://steamcommunity.com/openid/id/abc").is_err());
        assert!(parse_claimed_id("https://steamcommunity.com/openid/id/123abc").is_err());
    }

    #[test]
    fn the_authorization_url_carries_the_state_and_our_callback() {
        let url = provider().authorization_url("abc123").unwrap();

        assert!(url.starts_with("https://steamcommunity.com/openid/login?"));
        assert!(url.contains("openid.mode=checkid_setup"));
        assert!(url.contains("identifier_select"));
        // return_to is percent-encoded inside the query string.
        assert!(url.contains("callback%3Fstate%3Dabc123"));
    }

    #[test]
    fn verification_echoes_every_parameter_with_the_mode_swapped() {
        let mut params = BTreeMap::new();
        params.insert("openid.mode".to_string(), "id_res".to_string());
        params.insert("openid.sig".to_string(), "abc".to_string());
        params.insert("openid.signed".to_string(), "mode,sig".to_string());

        let form = check_authentication_form(&params);

        assert!(form.contains(&("openid.mode".into(), "check_authentication".into())));
        assert!(form.contains(&("openid.sig".into(), "abc".into())));
        assert!(form.contains(&("openid.signed".into(), "mode,sig".into())));
        assert_eq!(form.len(), 3);
    }

    #[test]
    fn only_an_explicit_is_valid_true_passes() {
        assert!(is_valid_response(
            "ns:http://specs.openid.net/auth/2.0\nis_valid:true\n"
        ));
        assert!(!is_valid_response(
            "ns:http://specs.openid.net/auth/2.0\nis_valid:false\n"
        ));
        assert!(!is_valid_response(""));
        assert!(!is_valid_response("is_valid:maybe"));
        // Must not be fooled by the substring appearing elsewhere.
        assert!(!is_valid_response("error:is_valid:true is not set"));
    }

    #[tokio::test]
    async fn an_assertion_aimed_at_another_site_never_reaches_steam() {
        let mut params = BTreeMap::new();
        params.insert(
            "openid.return_to".to_string(),
            "https://evil.example/auth/steam/callback".to_string(),
        );
        params.insert(
            "openid.claimed_id".to_string(),
            "https://steamcommunity.com/openid/id/76561198047011640".to_string(),
        );

        let error = provider().verify(&params).await.unwrap_err();
        assert!(matches!(error, OpenIdError::Malformed(_)));
    }

    #[tokio::test]
    async fn a_missing_claimed_id_is_rejected_before_any_network_call() {
        let mut params = BTreeMap::new();
        params.insert(
            "openid.return_to".to_string(),
            "http://localhost:8080/api/auth/steam/callback?state=x".to_string(),
        );

        let error = provider().verify(&params).await.unwrap_err();
        assert!(matches!(error, OpenIdError::Malformed(_)));
    }
}

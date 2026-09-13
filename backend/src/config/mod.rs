use std::env;

/// Application configuration, sourced exclusively from environment variables.
///
/// Secrets (`llm_api_key`, database credentials) live here and are never
/// serialized or sent to the frontend.
// Provider fields are parsed and validated at startup but only consumed once
// the Dota and LLM services land in Phase 2/4.
#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub host: String,
    pub port: u16,
    /// Comma-separated list of origins allowed to call the API.
    pub cors_origins: Vec<String>,
    pub auth: AuthConfig,
    pub dota: DotaConfig,
    pub llm: LlmConfig,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct AuthConfig {
    /// Public origin of *this* API, used to build `openid.return_to`. Steam
    /// signs the value, so it must match what the browser actually reaches.
    pub public_base_url: String,
    /// Where the user lands after a completed or failed login.
    pub frontend_base_url: String,
    /// Steam's OpenID 2.0 endpoint. Overridable so tests never touch Valve.
    pub steam_openid_url: String,
    pub session_ttl_hours: i64,
    /// Set the `Secure` flag on cookies. Must be true in production; false
    /// allows plain-HTTP local development.
    pub cookie_secure: bool,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct DotaConfig {
    pub base_url: String,
    /// Optional OpenDota key. Raises the rate limit; the free tier works without it.
    pub api_key: Option<String>,
    /// How many recent matches a single sync pulls.
    pub sync_match_limit: u32,
    /// Minimum gap between two syncs of the same player, in seconds.
    pub sync_cooldown_seconds: i64,
    pub request_timeout_seconds: u64,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct LlmConfig {
    pub base_url: String,
    pub api_key: Option<String>,
    pub model: String,
}

impl LlmConfig {
    /// Whether the LLM provider has enough configuration to be usable.
    /// Phase 1 only reports this at startup; later phases gate analysis on it.
    pub fn is_configured(&self) -> bool {
        self.api_key.as_ref().is_some_and(|k| !k.is_empty())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("missing required environment variable: {0}")]
    Missing(&'static str),
    #[error("invalid value for environment variable {0}: {1}")]
    Invalid(&'static str, String),
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        Ok(Self {
            database_url: required("DATABASE_URL")?,
            host: optional("HOST", "0.0.0.0"),
            port: optional("PORT", "8080")
                .parse()
                .map_err(|e| ConfigError::Invalid("PORT", format!("{e}")))?,
            cors_origins: optional("CORS_ORIGINS", "http://localhost:3000")
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
            auth: AuthConfig {
                public_base_url: trim_trailing_slash(&optional(
                    "PUBLIC_BASE_URL",
                    "http://localhost:8080",
                )),
                frontend_base_url: trim_trailing_slash(&optional(
                    "FRONTEND_BASE_URL",
                    "http://localhost:3000",
                )),
                steam_openid_url: optional(
                    "STEAM_OPENID_URL",
                    "https://steamcommunity.com/openid/login",
                ),
                session_ttl_hours: parsed("SESSION_TTL_HOURS", 720)?,
                cookie_secure: parsed("COOKIE_SECURE", false)?,
            },
            dota: DotaConfig {
                base_url: optional("DOTA_API_BASE_URL", "https://api.opendota.com/api"),
                api_key: env::var("DOTA_API_KEY").ok().filter(|s| !s.is_empty()),
                sync_match_limit: parsed("SYNC_MATCH_LIMIT", 20)?.clamp(1, 100),
                sync_cooldown_seconds: parsed("SYNC_COOLDOWN_SECONDS", 30)?,
                request_timeout_seconds: parsed("DOTA_API_TIMEOUT_SECONDS", 10)?,
            },
            llm: LlmConfig {
                base_url: optional("LLM_BASE_URL", "https://api.openai.com/v1"),
                api_key: env::var("LLM_API_KEY").ok().filter(|s| !s.is_empty()),
                model: optional("LLM_MODEL", "gpt-4o-mini"),
            },
        })
    }

    pub fn bind_address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

fn required(key: &'static str) -> Result<String, ConfigError> {
    env::var(key)
        .ok()
        .filter(|v| !v.is_empty())
        .ok_or(ConfigError::Missing(key))
}

/// Parse an optional numeric variable, falling back to `default` when unset.
fn parsed<T>(key: &'static str, default: T) -> Result<T, ConfigError>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    match env::var(key).ok().filter(|v| !v.is_empty()) {
        None => Ok(default),
        Some(raw) => raw
            .trim()
            .parse()
            .map_err(|e| ConfigError::Invalid(key, format!("{e}"))),
    }
}

/// URLs are concatenated with paths throughout, so store them without a
/// trailing slash and never produce `//` in a redirect.
fn trim_trailing_slash(value: &str) -> String {
    value.trim_end_matches('/').to_string()
}

fn optional(key: &str, default: &str) -> String {
    env::var(key)
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| default.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn llm(key: Option<&str>) -> LlmConfig {
        LlmConfig {
            base_url: "https://example.test/v1".into(),
            api_key: key.map(str::to_string),
            model: "test-model".into(),
        }
    }

    #[test]
    fn llm_is_configured_only_with_a_non_empty_key() {
        assert!(!llm(None).is_configured());
        assert!(!llm(Some("")).is_configured());
        assert!(llm(Some("sk-test")).is_configured());
    }

    #[test]
    fn bind_address_joins_host_and_port() {
        let config = Config {
            database_url: "postgres://localhost/test".into(),
            host: "127.0.0.1".into(),
            port: 9000,
            cors_origins: vec![],
            auth: AuthConfig {
                public_base_url: "http://localhost:8080".into(),
                frontend_base_url: "http://localhost:3000".into(),
                steam_openid_url: "https://steamcommunity.com/openid/login".into(),
                session_ttl_hours: 720,
                cookie_secure: false,
            },
            dota: DotaConfig {
                base_url: "https://api.opendota.com/api".into(),
                api_key: None,
                sync_match_limit: 20,
                sync_cooldown_seconds: 30,
                request_timeout_seconds: 10,
            },
            llm: llm(None),
        };
        assert_eq!(config.bind_address(), "127.0.0.1:9000");
    }

    #[test]
    fn parsed_falls_back_when_unset_and_rejects_garbage() {
        // A name no test sets, so this exercises the unset path deterministically.
        assert_eq!(parsed::<u32>("DOTA_COACH_TEST_ABSENT_VAR", 20).unwrap(), 20);

        env::set_var("DOTA_COACH_TEST_BAD_VAR", "twenty");
        assert!(parsed::<u32>("DOTA_COACH_TEST_BAD_VAR", 20).is_err());
        env::remove_var("DOTA_COACH_TEST_BAD_VAR");
    }

    #[test]
    fn base_urls_never_keep_a_trailing_slash() {
        assert_eq!(trim_trailing_slash("http://x:3000/"), "http://x:3000");
        assert_eq!(trim_trailing_slash("http://x:3000"), "http://x:3000");
    }
}

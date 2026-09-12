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
    pub dota_api_base_url: String,
    pub llm: LlmConfig,
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
            dota_api_base_url: optional("DOTA_API_BASE_URL", "https://api.opendota.com/api"),
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
            dota_api_base_url: "https://api.opendota.com/api".into(),
            llm: llm(None),
        };
        assert_eq!(config.bind_address(), "127.0.0.1:9000");
    }
}

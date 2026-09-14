//! OpenAI-compatible implementation of [`LlmProvider`].
//!
//! "Compatible" rather than "OpenAI": the endpoint, key and model all come
//! from configuration, so any server speaking `POST /chat/completions` — a
//! self-hosted gateway, Azure, a local runtime — works without a code change.
//!
//! The API key is read from config, sent in an `Authorization` header, and is
//! never logged, serialized or returned. Provider error bodies are logged at
//! debug and never surfaced: they have been known to echo the request back.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use serde_json::json;

use super::{LlmCompletion, LlmError, LlmProvider, LlmRequest};
use crate::config::LlmConfig;

pub struct OpenAiLlmProvider {
    http: Client,
    base_url: String,
    api_key: Option<String>,
    model: String,
}

impl OpenAiLlmProvider {
    pub fn new(config: &LlmConfig, timeout: Duration) -> Result<Arc<Self>, reqwest::Error> {
        let http = Client::builder().timeout(timeout).build()?;

        Ok(Arc::new(Self {
            http,
            base_url: config.base_url.trim_end_matches('/').to_string(),
            api_key: config.api_key.clone().filter(|k| !k.is_empty()),
            model: config.model.clone(),
        }))
    }
}

#[async_trait]
impl LlmProvider for OpenAiLlmProvider {
    fn is_configured(&self) -> bool {
        self.api_key.is_some()
    }

    async fn generate(&self, request: &LlmRequest) -> Result<LlmCompletion, LlmError> {
        let Some(key) = &self.api_key else {
            return Err(LlmError::NotConfigured);
        };

        let mut body = json!({
            "model": self.model,
            "temperature": request.temperature,
            "max_tokens": request.max_output_tokens,
            "messages": [
                {"role": "system", "content": request.system},
                {"role": "user", "content": request.user},
            ],
        });
        if request.json_only {
            body["response_format"] = json!({"type": "json_object"});
        }

        let response = self
            .http
            .post(format!("{}/chat/completions", self.base_url))
            .bearer_auth(key)
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    LlmError::Timeout
                } else {
                    LlmError::Unavailable(e.to_string())
                }
            })?;

        let status = response.status();
        if status == StatusCode::TOO_MANY_REQUESTS {
            return Err(LlmError::RateLimited);
        }
        if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
            // Worth its own log line: this is a deployment problem, not a blip.
            tracing::error!(%status, "llm provider rejected our credentials");
            return Err(LlmError::NotConfigured);
        }
        if !status.is_success() {
            // Read the body before logging: holding the future inside a
            // tracing macro would make this whole call non-`Send`.
            let body = response.text().await.unwrap_or_default();
            tracing::debug!(%status, body, "llm provider error body");
            return Err(LlmError::Unavailable(format!("HTTP {status}")));
        }

        let payload: serde_json::Value = response
            .json()
            .await
            .map_err(|e| LlmError::InvalidResponse(e.to_string()))?;

        parse_completion(&payload, &self.model)
    }
}

#[derive(Debug, Deserialize)]
struct Usage {
    prompt_tokens: Option<u32>,
    completion_tokens: Option<u32>,
}

/// Pull the answer out of a chat-completions payload.
///
/// Tolerant about what it does not need — usage, the model echo, extra choices
/// — and strict about what it does: an empty or missing message is an invalid
/// response, never an empty analysis.
fn parse_completion(
    payload: &serde_json::Value,
    fallback_model: &str,
) -> Result<LlmCompletion, LlmError> {
    let choice = payload
        .get("choices")
        .and_then(|c| c.as_array())
        .and_then(|c| c.first())
        .ok_or_else(|| LlmError::InvalidResponse("no choices in response".into()))?;

    // A provider that stopped early produced a truncated object, which would
    // fail JSON parsing downstream with a far less useful message.
    if let Some(reason) = choice.get("finish_reason").and_then(|r| r.as_str()) {
        if reason == "length" {
            return Err(LlmError::InvalidResponse(
                "the model ran out of output tokens".into(),
            ));
        }
    }

    let text = choice
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .unwrap_or_default()
        .trim()
        .to_string();

    if text.is_empty() {
        return Err(LlmError::InvalidResponse("empty completion".into()));
    }

    let usage: Option<Usage> = payload
        .get("usage")
        .and_then(|u| serde_json::from_value(u.clone()).ok());

    Ok(LlmCompletion {
        text,
        model: payload
            .get("model")
            .and_then(|m| m.as_str())
            .unwrap_or(fallback_model)
            .to_string(),
        input_tokens: usage.as_ref().and_then(|u| u.prompt_tokens),
        output_tokens: usage.as_ref().and_then(|u| u.completion_tokens),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(key: Option<&str>) -> LlmConfig {
        LlmConfig {
            base_url: "https://llm.example/v1".into(),
            api_key: key.map(str::to_string),
            model: "test-model".into(),
        }
    }

    #[test]
    fn a_provider_without_a_key_reports_itself_unconfigured() {
        let provider = OpenAiLlmProvider::new(&config(None), Duration::from_secs(1)).unwrap();
        assert!(!provider.is_configured());

        let empty = OpenAiLlmProvider::new(&config(Some("")), Duration::from_secs(1)).unwrap();
        assert!(!empty.is_configured(), "an empty key is not a key");
    }

    #[tokio::test]
    async fn an_unconfigured_provider_never_reaches_the_network() {
        // The base URL below does not resolve; reaching it would be a failure
        // of a different shape than NotConfigured.
        let provider = OpenAiLlmProvider::new(&config(None), Duration::from_millis(50)).unwrap();
        let request = LlmRequest {
            system: "s".into(),
            user: "u".into(),
            max_output_tokens: 16,
            temperature: 0.2,
            json_only: true,
        };

        assert!(matches!(
            provider.generate(&request).await,
            Err(LlmError::NotConfigured)
        ));
    }

    #[test]
    fn a_normal_completion_parses_with_its_usage() {
        let payload = serde_json::json!({
            "model": "served-model",
            "choices": [{"finish_reason": "stop", "message": {"role": "assistant", "content": " {\"ok\":true} "}}],
            "usage": {"prompt_tokens": 120, "completion_tokens": 40},
        });

        let completion = parse_completion(&payload, "configured-model").unwrap();

        assert_eq!(completion.text, "{\"ok\":true}", "whitespace is trimmed");
        assert_eq!(completion.model, "served-model", "attribute what answered");
        assert_eq!(completion.input_tokens, Some(120));
        assert_eq!(completion.output_tokens, Some(40));
    }

    #[test]
    fn a_provider_that_omits_the_model_falls_back_to_the_configured_one() {
        let payload = serde_json::json!({
            "choices": [{"message": {"content": "hello"}}],
        });

        let completion = parse_completion(&payload, "configured-model").unwrap();
        assert_eq!(completion.model, "configured-model");
        assert_eq!(completion.input_tokens, None);
    }

    #[test]
    fn a_truncated_answer_is_an_invalid_response_not_a_short_one() {
        let payload = serde_json::json!({
            "choices": [{"finish_reason": "length", "message": {"content": "{\"insights\": [" }}],
        });

        let error = parse_completion(&payload, "m").unwrap_err();
        assert!(matches!(error, LlmError::InvalidResponse(_)));
        assert!(error.to_string().contains("output tokens"));
    }

    #[test]
    fn an_empty_completion_is_rejected_rather_than_treated_as_no_advice() {
        let payload = serde_json::json!({"choices": [{"message": {"content": "   "}}]});
        assert!(matches!(
            parse_completion(&payload, "m"),
            Err(LlmError::InvalidResponse(_))
        ));
    }

    #[test]
    fn a_response_with_no_choices_is_rejected() {
        let payload = serde_json::json!({"choices": []});
        assert!(matches!(
            parse_completion(&payload, "m"),
            Err(LlmError::InvalidResponse(_))
        ));
    }

    #[test]
    fn every_error_has_a_note_that_does_not_leak_the_cause() {
        let error = LlmError::Unavailable("dns failure for llm.internal.example".into());
        assert!(!error.user_note().contains("dns"));
        assert!(LlmError::NotConfigured
            .user_note()
            .contains("not configured"));
    }
}

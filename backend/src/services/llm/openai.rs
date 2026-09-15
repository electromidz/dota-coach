//! OpenAI-compatible implementation of [`LlmProvider`].
//!
//! "Compatible" rather than "OpenAI": the endpoint, key and model all come
//! from configuration, so any server speaking `POST /chat/completions` — a
//! self-hosted gateway, Azure, a local runtime — works without a code change.
//!
//! The API key is read from config, sent in an `Authorization` header, and is
//! never logged, serialized or returned. Provider error bodies are logged at
//! debug and never surfaced: they have been known to echo the request back.
//!
//! Calls are streamed. Not for latency — the answer is useless until it is
//! whole and validated — but because a reasoning model can think for over a
//! minute before its first visible character, and a gateway sitting between us
//! and it will abandon a request that has sent nothing back. `stream: true`
//! keeps frames flowing over that gap. The body is still accumulated in full
//! here, so everything above this module sees one complete answer as before.

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
            "stream": true,
            // Providers that support it report token counts in a final frame;
            // ones that do not ignore the key, and usage stays unknown.
            "stream_options": {"include_usage": true},
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

        // The stream is drained to completion before anything is parsed: a
        // partial answer is not a short answer, and the timeout on the client
        // bounds how long this can take.
        let body = response.text().await.map_err(|e| {
            if e.is_timeout() {
                LlmError::Timeout
            } else {
                LlmError::InvalidResponse(e.to_string())
            }
        })?;

        parse_stream(&body, &self.model)
    }
}

#[derive(Debug, Deserialize)]
struct Usage {
    prompt_tokens: Option<u32>,
    completion_tokens: Option<u32>,
}

/// Reassemble the answer from a server-sent-events body.
///
/// Tolerant about what it does not need — usage, the model echo, keep-alive
/// comments, a reasoning channel it never reads — and strict about what it
/// does: a truncated or empty answer is an invalid response, never an empty
/// analysis.
fn parse_stream(body: &str, fallback_model: &str) -> Result<LlmCompletion, LlmError> {
    let mut content = String::new();
    let mut finish_reason: Option<String> = None;
    let mut model: Option<String> = None;
    let mut input_tokens: Option<u32> = None;
    let mut output_tokens: Option<u32> = None;
    let mut frames = 0usize;

    for line in body.lines() {
        // Anything that is not a data frame — blank separators, `event:` lines,
        // `:` keep-alive comments — carries nothing we need.
        let Some(data) = line.strip_prefix("data:") else {
            continue;
        };
        let data = data.trim();
        if data.is_empty() || data == "[DONE]" {
            continue;
        }

        // One unreadable frame is not worth discarding an otherwise complete
        // answer for; a wholly broken stream still fails the checks below.
        let Ok(frame) = serde_json::from_str::<serde_json::Value>(data) else {
            continue;
        };
        frames += 1;

        if model.is_none() {
            model = frame
                .get("model")
                .and_then(|m| m.as_str())
                .map(str::to_string);
        }

        if let Some(usage) = frame
            .get("usage")
            .and_then(|u| serde_json::from_value::<Usage>(u.clone()).ok())
        {
            input_tokens = usage.prompt_tokens.or(input_tokens);
            output_tokens = usage.completion_tokens.or(output_tokens);
        }

        let Some(choice) = frame
            .get("choices")
            .and_then(|c| c.as_array())
            .and_then(|c| c.first())
        else {
            continue;
        };

        if let Some(reason) = choice.get("finish_reason").and_then(|r| r.as_str()) {
            finish_reason = Some(reason.to_string());
        }

        // `delta` while streaming; `message` covers a provider that answers a
        // stream request with one complete frame instead.
        if let Some(piece) = choice
            .get("delta")
            .or_else(|| choice.get("message"))
            .and_then(|d| d.get("content"))
            .and_then(|c| c.as_str())
        {
            content.push_str(piece);
        }
    }

    if frames == 0 {
        return Err(LlmError::InvalidResponse(
            "the response carried no stream frames".into(),
        ));
    }

    // A model that stopped early produced a truncated object, which would fail
    // JSON parsing downstream with a far less useful message. For a reasoning
    // model this is the usual shape of "the output budget was too small": the
    // thinking consumed it before any content was emitted.
    if finish_reason.as_deref() == Some("length") {
        return Err(LlmError::InvalidResponse(
            "the model ran out of output tokens".into(),
        ));
    }

    let text = content.trim().to_string();
    if text.is_empty() {
        return Err(LlmError::InvalidResponse("empty completion".into()));
    }

    Ok(LlmCompletion {
        text,
        model: model.unwrap_or_else(|| fallback_model.to_string()),
        input_tokens,
        output_tokens,
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

    /// One SSE data frame, as a provider writes it.
    fn frame(value: serde_json::Value) -> String {
        format!("data: {value}\n\n")
    }

    #[test]
    fn a_streamed_completion_is_reassembled_with_its_usage() {
        let body = [
            frame(serde_json::json!({
                "model": "served-model",
                "choices": [{"delta": {"role": "assistant", "content": " {\"ok\":"}}],
            })),
            frame(serde_json::json!({
                "choices": [{"delta": {"content": "true} "}, "finish_reason": "stop"}],
            })),
            frame(serde_json::json!({
                "choices": [],
                "usage": {"prompt_tokens": 120, "completion_tokens": 40},
            })),
            "data: [DONE]\n\n".to_string(),
        ]
        .concat();

        let completion = parse_stream(&body, "configured-model").unwrap();

        assert_eq!(completion.text, "{\"ok\":true}", "whitespace is trimmed");
        assert_eq!(completion.model, "served-model", "attribute what answered");
        assert_eq!(completion.input_tokens, Some(120));
        assert_eq!(completion.output_tokens, Some(40));
    }

    #[test]
    fn a_reasoning_channel_is_ignored_and_only_content_is_kept() {
        // The shape that broke coaching in production: the model thinks for
        // thousands of tokens in a channel we must not read, then answers.
        let body = [
            frame(serde_json::json!({
                "choices": [{"delta": {"reasoning_content": "Let me weigh the evidence..."}}],
            })),
            frame(serde_json::json!({
                "choices": [{"delta": {"content": "{\"summary\":\"x\"}"}, "finish_reason": "stop"}],
            })),
        ]
        .concat();

        let completion = parse_stream(&body, "m").unwrap();
        assert_eq!(completion.text, "{\"summary\":\"x\"}");
    }

    #[test]
    fn a_provider_that_omits_the_model_falls_back_to_the_configured_one() {
        let body = frame(serde_json::json!({
            "choices": [{"delta": {"content": "hello"}}],
        }));

        let completion = parse_stream(&body, "configured-model").unwrap();
        assert_eq!(completion.model, "configured-model");
        assert_eq!(completion.input_tokens, None);
    }

    #[test]
    fn a_provider_that_answers_a_stream_request_in_one_whole_frame_is_read() {
        let body = frame(serde_json::json!({
            "choices": [{"message": {"content": "hello"}, "finish_reason": "stop"}],
        }));

        assert_eq!(parse_stream(&body, "m").unwrap().text, "hello");
    }

    #[test]
    fn keep_alives_and_unreadable_frames_do_not_discard_a_good_answer() {
        let body = format!(
            ": keep-alive\n\nevent: message\n{}data: not json at all\n\n{}",
            frame(serde_json::json!({"choices": [{"delta": {"content": "hel"}}]})),
            frame(serde_json::json!({
                "choices": [{"delta": {"content": "lo"}, "finish_reason": "stop"}],
            })),
        );

        assert_eq!(parse_stream(&body, "m").unwrap().text, "hello");
    }

    #[test]
    fn a_truncated_answer_is_an_invalid_response_not_a_short_one() {
        let body = frame(serde_json::json!({
            "choices": [{"delta": {"content": "{\"insights\": ["}, "finish_reason": "length"}],
        }));

        let error = parse_stream(&body, "m").unwrap_err();
        assert!(matches!(error, LlmError::InvalidResponse(_)));
        assert!(error.to_string().contains("output tokens"));
    }

    #[test]
    fn a_budget_spent_entirely_on_thinking_is_reported_as_a_truncation() {
        // 900 tokens of reasoning and no content: the exact failure that
        // produced "upstream service unavailable" for every analysis.
        let body = frame(serde_json::json!({
            "choices": [{"delta": {"reasoning_content": "..."}, "finish_reason": "length"}],
        }));

        let error = parse_stream(&body, "m").unwrap_err();
        assert!(error.to_string().contains("output tokens"));
    }

    #[test]
    fn an_empty_completion_is_rejected_rather_than_treated_as_no_advice() {
        let body = frame(serde_json::json!({
            "choices": [{"delta": {"content": "   "}, "finish_reason": "stop"}],
        }));
        assert!(matches!(
            parse_stream(&body, "m"),
            Err(LlmError::InvalidResponse(_))
        ));
    }

    #[test]
    fn a_body_with_no_frames_is_rejected() {
        assert!(matches!(
            parse_stream("", "m"),
            Err(LlmError::InvalidResponse(_))
        ));
        assert!(matches!(
            parse_stream("data: [DONE]\n\n", "m"),
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

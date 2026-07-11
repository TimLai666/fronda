//! Concrete Anthropic Messages API transport for the agent loop.
//!
//! Implements `agent_contract::LlmTransport` with a blocking reqwest client
//! (rustls TLS, no OpenSSL — portable across macOS/Windows/Linux). The agent
//! loop stays synchronous and runs on a background thread, matching the app's
//! existing threading model.
//!
//! The live HTTP round-trip needs a real API key and network, so it is not
//! covered by automated tests; the request preparation (URL, headers, body)
//! is factored out and unit-tested.

use agent_contract::LlmTransport;
use serde_json::Value;
use std::time::Duration;

const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";
const DEFAULT_ANTHROPIC_VERSION: &str = "2023-06-01";

/// Endpoint + credential configuration for the Anthropic API.
#[derive(Debug, Clone)]
pub struct AnthropicConfig {
    pub api_key: String,
    pub base_url: String,
    pub anthropic_version: String,
    pub timeout: Duration,
}

/// Upstream #36: resolve the base URL from an `ANTHROPIC_BASE_URL`-style
/// value — blank/whitespace falls back to the public API.
fn resolve_base_url(env_value: Option<String>) -> String {
    env_value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| DEFAULT_BASE_URL.to_string())
}

impl AnthropicConfig {
    /// Config with the public API base URL and current API version.
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: DEFAULT_BASE_URL.to_string(),
            anthropic_version: DEFAULT_ANTHROPIC_VERSION.to_string(),
            timeout: Duration::from_secs(120),
        }
    }

    /// Config honouring the `ANTHROPIC_BASE_URL` environment variable
    /// (proxy/gateway, upstream #36); unset or blank keeps the public API.
    pub fn from_env(api_key: impl Into<String>) -> Self {
        let base = resolve_base_url(std::env::var("ANTHROPIC_BASE_URL").ok());
        Self::new(api_key).with_base_url(base)
    }

    /// Override the base URL (e.g. a proxy or gateway). Trailing slash tolerated.
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    /// The Messages API endpoint for this config.
    pub fn messages_url(&self) -> String {
        format!("{}/v1/messages", self.base_url.trim_end_matches('/'))
    }
}

/// Blocking Anthropic transport. Construct once and reuse across turns.
pub struct AnthropicTransport {
    client: reqwest::blocking::Client,
    config: AnthropicConfig,
}

impl AnthropicTransport {
    pub fn new(config: AnthropicConfig) -> Result<Self, String> {
        let client = reqwest::blocking::Client::builder()
            .timeout(config.timeout)
            .build()
            .map_err(|e| format!("build http client: {e}"))?;
        Ok(Self { client, config })
    }

    /// Extract a human-readable message from an Anthropic error body, falling
    /// back to the raw JSON when the shape is unexpected.
    fn error_message(status: u16, body: &Value) -> String {
        let msg = body
            .get("error")
            .and_then(|e| e.get("message"))
            .and_then(Value::as_str)
            .unwrap_or_else(|| body.get("error").and_then(Value::as_str).unwrap_or(""));
        if msg.is_empty() {
            format!("anthropic API error {status}: {body}")
        } else {
            format!("anthropic API error {status}: {msg}")
        }
    }
}

impl LlmTransport for AnthropicTransport {
    fn send(&mut self, request: &Value) -> Result<Value, String> {
        let response = self
            .client
            .post(self.config.messages_url())
            .header("x-api-key", &self.config.api_key)
            .header("anthropic-version", &self.config.anthropic_version)
            .header("content-type", "application/json")
            .json(request)
            .send()
            .map_err(|e| format!("request failed: {e}"))?;

        let status = response.status();
        let body: Value = response
            .json()
            .map_err(|e| format!("decode response: {e}"))?;

        if status.is_success() {
            Ok(body)
        } else {
            Err(Self::error_message(status.as_u16(), &body))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_targets_public_api() {
        let config = AnthropicConfig::new("sk-test");
        assert_eq!(
            config.messages_url(),
            "https://api.anthropic.com/v1/messages"
        );
        assert_eq!(config.anthropic_version, "2023-06-01");
    }

    #[test]
    fn base_url_override_trims_trailing_slash() {
        let config = AnthropicConfig::new("k").with_base_url("http://localhost:8080/");
        assert_eq!(config.messages_url(), "http://localhost:8080/v1/messages");
    }

    #[test]
    fn error_message_prefers_structured_field() {
        let body =
            serde_json::json!({ "error": { "type": "invalid_request", "message": "bad model" } });
        assert_eq!(
            AnthropicTransport::error_message(400, &body),
            "anthropic API error 400: bad model"
        );
    }

    #[test]
    fn error_message_falls_back_to_raw_body() {
        let body = serde_json::json!({ "unexpected": true });
        let msg = AnthropicTransport::error_message(500, &body);
        assert!(msg.starts_with("anthropic API error 500:"));
    }

    #[test]
    fn transport_constructs() {
        assert!(AnthropicTransport::new(AnthropicConfig::new("sk-test")).is_ok());
    }

    // ─── Upstream #36: ANTHROPIC_BASE_URL override ───

    #[test]
    fn resolve_base_url_default_when_unset_or_blank() {
        assert_eq!(resolve_base_url(None), DEFAULT_BASE_URL);
        assert_eq!(resolve_base_url(Some(String::new())), DEFAULT_BASE_URL);
        assert_eq!(resolve_base_url(Some("   ".into())), DEFAULT_BASE_URL);
    }

    #[test]
    fn resolve_base_url_uses_override_trimmed() {
        assert_eq!(
            resolve_base_url(Some("  https://gateway.example.com  ".into())),
            "https://gateway.example.com"
        );
        let config = AnthropicConfig::new("k")
            .with_base_url(resolve_base_url(Some("http://proxy:9/".into())));
        assert_eq!(config.messages_url(), "http://proxy:9/v1/messages");
    }
}

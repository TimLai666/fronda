//! First real provider: Gemini image generation over the Google
//! `generateContent` REST surface, bring-your-own-key.
//!
//! `POST {base}/{apiVersion}/models/{model}:generateContent` with header
//! `x-goog-api-key: <key>` and body
//! `{"contents":[{"parts":[{"text":<prompt>}]}],
//!   "generationConfig":{"responseModalities":["TEXT","IMAGE"]}}`.
//! A 200 carries `candidates[0].content.parts[]`; the first part with an
//! `inlineData` object holds the base64 image (`data`) and its `mimeType`.
//! That is decoded into the `ResultStore` and the job succeeds with a
//! `{public_base}/v1/results/{id}` URL; any non-image response (safety block,
//! text-only) or transport/HTTP error fails the job with a reason.
//!
//! The provider trait is synchronous: `submit` opens a job, marks it Running,
//! `tokio::spawn`s the HTTP call (the axum handler runs inside the runtime),
//! and returns immediately; the spawned task writes the terminal state; `poll`
//! just reads the shared job store.

use std::sync::Arc;

use base64::Engine;
use serde_json::{json, Value};

use crate::config::GatewayConfig;
use crate::jobs::JobStore;
use crate::protocol::GenerateRequest;
use crate::provider::{
    provider_http_client, GenerationProvider, ProviderJob, ProviderKind, ProviderStatus,
};
use crate::results::ResultStore;

/// The name the Gemini provider registers under (also its BYO-key lookup key).
pub const GEMINI_NAME: &str = "gemini";
pub const DEFAULT_GEMINI_MODEL: &str = "gemini-2.5-flash-image";
pub const DEFAULT_GEMINI_BASE: &str = "https://generativelanguage.googleapis.com";
pub const DEFAULT_GEMINI_API_VERSION: &str = "v1beta";

/// Resolved Gemini connection settings. `base`/`api_version`/`model` are
/// overridable (env/config); `api_key` is required — no key, no provider.
#[derive(Debug, Clone)]
pub struct GeminiConfig {
    pub api_key: String,
    pub model: String,
    pub base: String,
    pub api_version: String,
}

impl GeminiConfig {
    /// Build from the gateway config. Returns `None` when no Gemini key is set,
    /// so the provider is registered only with a key.
    pub fn from_gateway(config: &GatewayConfig) -> Option<Self> {
        let api_key = config.provider_key(GEMINI_NAME)?.to_string();
        Some(Self {
            api_key,
            model: config
                .gemini_model
                .clone()
                .unwrap_or_else(|| DEFAULT_GEMINI_MODEL.to_string()),
            base: config
                .gemini_base
                .clone()
                .unwrap_or_else(|| DEFAULT_GEMINI_BASE.to_string()),
            api_version: config
                .gemini_api_version
                .clone()
                .unwrap_or_else(|| DEFAULT_GEMINI_API_VERSION.to_string()),
        })
    }

    fn generate_content_url(&self) -> String {
        format!(
            "{}/{}/models/{}:generateContent",
            self.base.trim_end_matches('/'),
            self.api_version,
            self.model
        )
    }
}

/// Gemini image provider (kind = Image). Holds a shared job store and result
/// store so the spawned generation task can publish bytes and terminal state.
pub struct GeminiImageProvider {
    config: GeminiConfig,
    store: Arc<JobStore>,
    results: Arc<ResultStore>,
    public_base: String,
    client: reqwest::Client,
}

impl GeminiImageProvider {
    pub fn new(
        config: GeminiConfig,
        store: Arc<JobStore>,
        results: Arc<ResultStore>,
        public_base: impl Into<String>,
    ) -> Self {
        Self {
            config,
            store,
            results,
            public_base: public_base.into(),
            client: provider_http_client(),
        }
    }

    /// Construct from the gateway config; `None` when no Gemini key is set.
    pub fn from_config(
        config: &GatewayConfig,
        store: Arc<JobStore>,
        results: Arc<ResultStore>,
        public_base: impl Into<String>,
    ) -> Option<Self> {
        let gemini = GeminiConfig::from_gateway(config)?;
        Some(Self::new(gemini, store, results, public_base))
    }
}

impl GenerationProvider for GeminiImageProvider {
    fn name(&self) -> &str {
        GEMINI_NAME
    }

    fn kind(&self) -> ProviderKind {
        ProviderKind::Image
    }

    fn models(&self) -> Vec<String> {
        vec![self.config.model.clone()]
    }

    fn submit(&self, req: &GenerateRequest) -> Result<ProviderJob, String> {
        let job_id = self.store.create(self.kind(), self.name());
        // A task is in flight immediately, so poll reports running until it lands.
        self.store
            .update(&job_id, |rec| rec.status = ProviderStatus::Running);

        let client = self.client.clone();
        let config = self.config.clone();
        let store = self.store.clone();
        let results = self.results.clone();
        let public_base = self.public_base.clone();
        let prompt = req.prompt.clone();
        let task_job_id = job_id.clone();

        tokio::spawn(async move {
            match run_gemini_generation(&client, &config, &prompt).await {
                Ok((bytes, content_type)) => {
                    let result_id = results.put(bytes, content_type);
                    let url = format!(
                        "{}/v1/results/{}",
                        public_base.trim_end_matches('/'),
                        result_id
                    );
                    store.update(&task_job_id, |rec| {
                        rec.result_urls = vec![url.clone()];
                        rec.status = ProviderStatus::Succeeded {
                            urls: vec![url.clone()],
                        };
                    });
                }
                Err(reason) => {
                    store.update(&task_job_id, |rec| {
                        rec.status = ProviderStatus::Failed { reason };
                    });
                }
            }
        });

        Ok(ProviderJob { job_id })
    }

    fn poll(&self, job_id: &str) -> Result<ProviderStatus, String> {
        self.store
            .get(job_id)
            .map(|rec| rec.status)
            .ok_or_else(|| format!("unknown job: {job_id}"))
    }
}

/// Call `generateContent` and reduce the response to `(image_bytes, content_type)`
/// or an explicit failure reason.
async fn run_gemini_generation(
    client: &reqwest::Client,
    config: &GeminiConfig,
    prompt: &str,
) -> Result<(Vec<u8>, String), String> {
    let body = build_generate_content_body(prompt);
    let resp = client
        .post(config.generate_content_url())
        .header("x-goog-api-key", &config.api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("gemini: request failed: {e}"))?;
    let status = resp.status().as_u16();
    let text = resp
        .text()
        .await
        .map_err(|e| format!("gemini: reading response body failed: {e}"))?;
    parse_generate_content(status, &text)
}

/// Build the `generateContent` request body: a single text prompt part with
/// image + text response modalities enabled.
pub fn build_generate_content_body(prompt: &str) -> Value {
    json!({
        "contents": [{ "parts": [{ "text": prompt }] }],
        "generationConfig": { "responseModalities": ["TEXT", "IMAGE"] }
    })
}

/// Turn a raw (status, body) pair into image bytes or a failure reason. Non-2xx
/// is a failure carrying the status and any error message; a 2xx is parsed for
/// the first `inlineData` image part.
pub fn parse_generate_content(status: u16, body: &str) -> Result<(Vec<u8>, String), String> {
    if !(200..300).contains(&status) {
        return Err(gemini_http_error(status, body));
    }
    let value: Value = serde_json::from_str(body)
        .map_err(|e| format!("gemini: invalid JSON response (HTTP {status}): {e}"))?;
    extract_inline_image(&value)
}

/// Extract `(bytes, content_type)` from the first `inlineData` part of a
/// successful response. Errors when the response carries no image (safety block
/// or text-only), naming the finish/block reason when the API supplies one.
pub fn extract_inline_image(body: &Value) -> Result<(Vec<u8>, String), String> {
    if let Some(candidate) = body
        .get("candidates")
        .and_then(|c| c.as_array())
        .and_then(|c| c.first())
    {
        if let Some(parts) = candidate
            .pointer("/content/parts")
            .and_then(|p| p.as_array())
        {
            for part in parts {
                if let Some(inline) = part.get("inlineData") {
                    let data = inline
                        .get("data")
                        .and_then(|d| d.as_str())
                        .ok_or_else(|| "gemini: inlineData part missing 'data'".to_string())?;
                    let mime = inline
                        .get("mimeType")
                        .and_then(|m| m.as_str())
                        .unwrap_or("image/png")
                        .to_string();
                    let bytes = base64::engine::general_purpose::STANDARD
                        .decode(data)
                        .map_err(|e| format!("gemini: base64 decode failed: {e}"))?;
                    return Ok((bytes, mime));
                }
            }
        }
        if let Some(reason) = candidate.get("finishReason").and_then(|r| r.as_str()) {
            return Err(format!("gemini: no image in response (finishReason: {reason})"));
        }
    }
    if let Some(block) = body
        .pointer("/promptFeedback/blockReason")
        .and_then(|b| b.as_str())
    {
        return Err(format!("gemini: prompt blocked (blockReason: {block})"));
    }
    Err("gemini: response contained no image (text-only or blocked)".to_string())
}

/// Best-effort error message for a non-2xx response: HTTP status plus the API's
/// `error.message` when present, else a body snippet.
fn gemini_http_error(status: u16, body: &str) -> String {
    if let Ok(value) = serde_json::from_str::<Value>(body) {
        if let Some(message) = value.pointer("/error/message").and_then(|m| m.as_str()) {
            return format!("gemini: HTTP {status}: {message}");
        }
    }
    let snippet: String = body.chars().take(200).collect();
    if snippet.trim().is_empty() {
        format!("gemini: HTTP {status}")
    } else {
        format!("gemini: HTTP {status}: {snippet}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A canonical 1x1 transparent PNG, base64-encoded — a real, decodable image.
    const PNG_1X1_B64: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAAC0lEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==";

    fn decoded_png() -> Vec<u8> {
        base64::engine::general_purpose::STANDARD
            .decode(PNG_1X1_B64)
            .unwrap()
    }

    #[test]
    fn build_body_has_prompt_and_image_modality() {
        let body = build_generate_content_body("a red logo");
        assert_eq!(body["contents"][0]["parts"][0]["text"], "a red logo");
        assert_eq!(
            body["generationConfig"]["responseModalities"],
            json!(["TEXT", "IMAGE"])
        );
    }

    #[test]
    fn generate_content_url_is_well_formed() {
        let config = GeminiConfig {
            api_key: "k".into(),
            model: "gemini-2.5-flash-image".into(),
            base: "https://generativelanguage.googleapis.com".into(),
            api_version: "v1beta".into(),
        };
        assert_eq!(
            config.generate_content_url(),
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash-image:generateContent"
        );
        // Trailing slash on base is tolerated.
        let config = GeminiConfig {
            base: "http://localhost:9/".into(),
            ..config
        };
        assert_eq!(
            config.generate_content_url(),
            "http://localhost:9/v1beta/models/gemini-2.5-flash-image:generateContent"
        );
    }

    #[test]
    fn parse_extracts_inline_image_bytes_and_mime() {
        let body = json!({
            "candidates": [{
                "content": { "parts": [
                    { "text": "here is your image" },
                    { "inlineData": { "mimeType": "image/png", "data": PNG_1X1_B64 } }
                ]}
            }]
        })
        .to_string();
        let (bytes, ct) = parse_generate_content(200, &body).unwrap();
        assert_eq!(bytes, decoded_png());
        assert_eq!(ct, "image/png");
    }

    #[test]
    fn parse_defaults_mime_when_absent() {
        let body = json!({
            "candidates": [{
                "content": { "parts": [ { "inlineData": { "data": PNG_1X1_B64 } } ] }
            }]
        })
        .to_string();
        let (_bytes, ct) = parse_generate_content(200, &body).unwrap();
        assert_eq!(ct, "image/png");
    }

    #[test]
    fn parse_text_only_response_is_failure() {
        let body = json!({
            "candidates": [{
                "content": { "parts": [ { "text": "I can't create that image." } ] },
                "finishReason": "STOP"
            }]
        })
        .to_string();
        let err = parse_generate_content(200, &body).unwrap_err();
        assert!(err.contains("no image"), "err was: {err}");
        assert!(err.contains("STOP"), "err was: {err}");
    }

    #[test]
    fn parse_prompt_block_is_failure_with_reason() {
        let body = json!({ "promptFeedback": { "blockReason": "SAFETY" } }).to_string();
        let err = parse_generate_content(200, &body).unwrap_err();
        assert!(err.contains("blocked"), "err was: {err}");
        assert!(err.contains("SAFETY"), "err was: {err}");
    }

    #[test]
    fn parse_non_200_carries_status_and_api_message() {
        let body = json!({ "error": { "message": "API key not valid" } }).to_string();
        let err = parse_generate_content(400, &body).unwrap_err();
        assert!(err.contains("400"), "err was: {err}");
        assert!(err.contains("API key not valid"), "err was: {err}");
    }

    #[test]
    fn parse_non_200_without_json_carries_snippet() {
        let err = parse_generate_content(503, "service unavailable").unwrap_err();
        assert!(err.contains("503"), "err was: {err}");
        assert!(err.contains("service unavailable"), "err was: {err}");
    }

    #[test]
    fn from_gateway_requires_a_key() {
        let mut config = GatewayConfig::default();
        assert!(GeminiConfig::from_gateway(&config).is_none());
        config
            .provider_keys
            .insert("gemini".into(), "secret-key".into());
        let gemini = GeminiConfig::from_gateway(&config).unwrap();
        assert_eq!(gemini.api_key, "secret-key");
        assert_eq!(gemini.model, DEFAULT_GEMINI_MODEL);
        assert_eq!(gemini.base, DEFAULT_GEMINI_BASE);
        assert_eq!(gemini.api_version, DEFAULT_GEMINI_API_VERSION);
    }

    #[test]
    fn from_gateway_applies_overrides() {
        let mut config = GatewayConfig::default();
        config
            .provider_keys
            .insert("gemini".into(), "k".into());
        config.gemini_model = Some("gemini-3-image".into());
        config.gemini_base = Some("http://127.0.0.1:1234".into());
        config.gemini_api_version = Some("v1".into());
        let gemini = GeminiConfig::from_gateway(&config).unwrap();
        assert_eq!(gemini.model, "gemini-3-image");
        assert_eq!(gemini.base, "http://127.0.0.1:1234");
        assert_eq!(gemini.api_version, "v1");
    }

    /// Gated live test: only runs with a real key set. Hits the real Gemini API,
    /// asserts a non-empty image comes back. Skipped (with a note) otherwise.
    #[tokio::test]
    async fn live_gemini_returns_real_image_bytes() {
        let Ok(api_key) = std::env::var("FRONDA_GEMINI_API_KEY") else {
            eprintln!("skipping live_gemini_returns_real_image_bytes: FRONDA_GEMINI_API_KEY unset");
            return;
        };
        let config = GeminiConfig {
            api_key,
            model: std::env::var("FRONDA_GEN_GEMINI_MODEL")
                .unwrap_or_else(|_| DEFAULT_GEMINI_MODEL.to_string()),
            base: DEFAULT_GEMINI_BASE.to_string(),
            api_version: DEFAULT_GEMINI_API_VERSION.to_string(),
        };
        let client = reqwest::Client::new();
        let (bytes, content_type) =
            run_gemini_generation(&client, &config, "a small red circle on white")
                .await
                .expect("live gemini generation should succeed");
        assert!(!bytes.is_empty(), "expected non-empty image bytes");
        assert!(
            content_type.starts_with("image/"),
            "expected image content-type, got {content_type}"
        );
    }
}

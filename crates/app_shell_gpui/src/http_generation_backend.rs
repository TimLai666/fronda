//! Concrete HTTP generation backend for Fronda Generation Protocol v1.
//!
//! Implements `agent_contract::GenerationBackend` (submit + resume_job) with a
//! blocking reqwest client (rustls TLS, no OpenSSL — portable across
//! macOS/Windows/Linux). Mirrors `anthropic_transport`: request building and
//! response parsing are factored into pure functions and unit-tested; the live
//! HTTP round-trip needs a configured endpoint and network, so it is not
//! covered by automated tests.
//!
//! Protocol v1 (see `specs/rust-rewrite/98-generation-protocol.md`):
//!   POST {base}/v1/generate  → { jobId, status }
//!   GET  {base}/v1/jobs/{id}  → { status, resultUrls?, error? }
//! Both use bearer auth. `queued`/`running` poll results are returned as `Err`
//! so the manifest entry stays pending for a later recovery pass (#216).

use std::time::Duration;

use agent_contract::GenerationBackend;
use generation_core::{GenerationOutcome, GenerationRequest, GenerationSubmission, ModelKind};
use serde_json::Value;

const GENERATION_URL_ENV: &str = "FRONDA_GENERATION_URL";
const GENERATION_TOKEN_ENV: &str = "FRONDA_GENERATION_TOKEN";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);

/// Endpoint + credential configuration for a Protocol v1 generation service.
#[derive(Debug, Clone)]
pub struct GenerationBackendConfig {
    pub base_url: String,
    pub token: String,
    pub timeout: Duration,
}

impl GenerationBackendConfig {
    pub fn new(base_url: impl Into<String>, token: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            token: token.into(),
            timeout: DEFAULT_TIMEOUT,
        }
    }

    /// Resolve from the environment: `FRONDA_GENERATION_URL` +
    /// `FRONDA_GENERATION_TOKEN`. Either missing or blank/whitespace yields
    /// `None` so the generate tools keep their honest "requires a remote API"
    /// error (no backend installed).
    pub fn from_env() -> Option<Self> {
        resolve_config(
            std::env::var(GENERATION_URL_ENV).ok(),
            std::env::var(GENERATION_TOKEN_ENV).ok(),
        )
    }

    /// The submit endpoint for this config. Trailing slash tolerated.
    pub fn generate_url(&self) -> String {
        format!("{}/v1/generate", self.base_url.trim_end_matches('/'))
    }

    /// The poll endpoint for a job. Trailing slash tolerated.
    pub fn job_url(&self, job_id: &str) -> String {
        format!("{}/v1/jobs/{job_id}", self.base_url.trim_end_matches('/'))
    }
}

/// Pure config resolution (env read factored out so tests avoid process-global
/// env mutation): both values must be present and non-blank.
fn resolve_config(url: Option<String>, token: Option<String>) -> Option<GenerationBackendConfig> {
    let base_url = non_blank(url)?;
    let token = non_blank(token)?;
    Some(GenerationBackendConfig::new(base_url, token))
}

fn non_blank(value: Option<String>) -> Option<String> {
    value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

/// Blocking HTTP generation backend. Construct once and reuse across jobs.
pub struct HttpGenerationBackend {
    client: reqwest::blocking::Client,
    config: GenerationBackendConfig,
}

impl HttpGenerationBackend {
    pub fn new(config: GenerationBackendConfig) -> Result<Self, String> {
        let client = reqwest::blocking::Client::builder()
            .timeout(config.timeout)
            .build()
            .map_err(|e| format!("build http client: {e}"))?;
        Ok(Self { client, config })
    }

    /// Build from the environment; missing/blank config → `None` (no backend
    /// installed, honest error preserved).
    pub fn from_config() -> Option<Self> {
        let config = GenerationBackendConfig::from_env()?;
        Self::new(config).ok()
    }
}

/// Lowercase protocol token for a model kind.
fn kind_token(kind: &ModelKind) -> &'static str {
    match kind {
        ModelKind::Video => "video",
        ModelKind::Image => "image",
        ModelKind::Audio => "audio",
        ModelKind::Upscale => "upscale",
    }
}

/// Protocol v1 `/v1/generate` request body. Optional fields are omitted when
/// absent; `params` is included only when it is a non-null JSON value.
pub fn build_submit_body(req: &GenerationRequest) -> Value {
    let mut body = serde_json::Map::new();
    body.insert("kind".into(), Value::String(kind_token(&req.kind).into()));
    body.insert("model".into(), Value::String(req.model.clone()));
    body.insert("prompt".into(), Value::String(req.prompt.clone()));
    if let Some(duration) = req.duration_seconds {
        body.insert("durationSeconds".into(), serde_json::json!(duration));
    }
    if let Some(source_url) = &req.source_url {
        body.insert("sourceUrl".into(), Value::String(source_url.clone()));
    }
    if let Some(language) = &req.target_language {
        body.insert("targetLanguage".into(), Value::String(language.clone()));
    }
    if !req.params.is_null() {
        body.insert("params".into(), req.params.clone());
    }
    Value::Object(body)
}

/// Parse a `/v1/generate` response. A 2xx with a non-empty `jobId` string is a
/// submission; any other status, or a body without a usable job id, is an error
/// carrying the status code and any server message.
pub fn parse_submit_response(status: u16, body: &Value) -> Result<GenerationSubmission, String> {
    if !is_success(status) {
        return Err(error_message(status, body));
    }
    let job_id = body
        .get("jobId")
        .and_then(Value::as_str)
        .map(str::to_string)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("generation submit: response missing jobId: {body}"))?;
    Ok(GenerationSubmission {
        backend_job_id: job_id,
    })
}

/// Map a `/v1/jobs/{id}` response to an outcome. `succeeded` → Success with
/// resultUrls; `failed` → Failure with the error reason; `queued`/`running`
/// (or any not-yet-terminal status) → Err so the manifest entry stays pending
/// for a later recovery pass (#216: "Err = no verdict, retry"). A non-2xx or a
/// body without a `status` field is likewise an Err.
pub fn parse_poll_response(status: u16, body: &Value) -> Result<GenerationOutcome, String> {
    if !is_success(status) {
        return Err(error_message(status, body));
    }
    let job_status = body
        .get("status")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("generation poll: response missing status: {body}"))?;
    match job_status {
        "succeeded" => {
            let result_urls: Vec<String> = body
                .get("resultUrls")
                .and_then(Value::as_array)
                .map(|urls| {
                    urls.iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default();
            // A "succeeded" job with no URLs delivered nothing: applying it as
            // Success flips the asset to ready (generation_status "none") with
            // no media, leaving a dangling done-but-empty entry. Treat it as a
            // failure so the asset stays honest and retryable.
            if result_urls.is_empty() {
                return Ok(GenerationOutcome::Failure {
                    reason: "generation reported success but returned no result URLs".to_string(),
                });
            }
            Ok(GenerationOutcome::Success { result_urls })
        }
        "failed" => {
            let reason = body
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or("generation failed")
                .to_string();
            Ok(GenerationOutcome::Failure { reason })
        }
        other => Err(format!("still {other}")),
    }
}

fn is_success(status: u16) -> bool {
    (200..300).contains(&status)
}

/// Human-readable message from an error body, falling back to the raw JSON.
fn error_message(status: u16, body: &Value) -> String {
    let msg = body
        .get("error")
        .and_then(Value::as_str)
        .or_else(|| body.get("message").and_then(Value::as_str))
        .unwrap_or("");
    if msg.is_empty() {
        format!("generation backend error {status}: {body}")
    } else {
        format!("generation backend error {status}: {msg}")
    }
}

impl GenerationBackend for HttpGenerationBackend {
    fn submit(&self, req: &GenerationRequest) -> Result<GenerationSubmission, String> {
        let body = build_submit_body(req);
        let response = self
            .client
            .post(self.config.generate_url())
            .bearer_auth(&self.config.token)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .map_err(|e| format!("generation submit request failed: {e}"))?;
        let status = response.status().as_u16();
        let body: Value = response
            .json()
            .map_err(|e| format!("generation submit: decode response: {e}"))?;
        parse_submit_response(status, &body)
    }

    fn resume_job(&self, job_id: &str) -> Result<GenerationOutcome, String> {
        let response = self
            .client
            .get(self.config.job_url(job_id))
            .bearer_auth(&self.config.token)
            .send()
            .map_err(|e| format!("generation poll request failed: {e}"))?;
        let status = response.status().as_u16();
        let body: Value = response
            .json()
            .map_err(|e| format!("generation poll: decode response: {e}"))?;
        parse_poll_response(status, &body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_request() -> GenerationRequest {
        GenerationRequest {
            kind: ModelKind::Video,
            model: "veo-3".into(),
            prompt: "a cat surfing".into(),
            duration_seconds: Some(5.0),
            source_url: Some("https://cdn/in.png".into()),
            target_language: Some("en".into()),
            params: serde_json::json!({ "aspectRatio": "16:9" }),
        }
    }

    // ─── config resolution ───

    #[test]
    fn resolve_config_needs_both_url_and_token() {
        assert!(resolve_config(None, Some("t".into())).is_none());
        assert!(resolve_config(Some("https://gen".into()), None).is_none());
        assert!(resolve_config(None, None).is_none());
        let cfg = resolve_config(Some("https://gen".into()), Some("t".into())).unwrap();
        assert_eq!(cfg.base_url, "https://gen");
        assert_eq!(cfg.token, "t");
    }

    #[test]
    fn resolve_config_rejects_blank_and_trims() {
        assert!(resolve_config(Some("   ".into()), Some("t".into())).is_none());
        assert!(resolve_config(Some("https://gen".into()), Some("  ".into())).is_none());
        let cfg = resolve_config(Some("  https://gen  ".into()), Some("  tok  ".into())).unwrap();
        assert_eq!(cfg.base_url, "https://gen");
        assert_eq!(cfg.token, "tok");
    }

    #[test]
    fn endpoints_tolerate_trailing_slash() {
        let cfg = GenerationBackendConfig::new("http://localhost:8080/", "tok");
        assert_eq!(cfg.generate_url(), "http://localhost:8080/v1/generate");
        assert_eq!(cfg.job_url("abc"), "http://localhost:8080/v1/jobs/abc");
    }

    // ─── request building ───

    #[test]
    fn submit_body_includes_all_present_fields() {
        let body = build_submit_body(&sample_request());
        assert_eq!(body["kind"], "video");
        assert_eq!(body["model"], "veo-3");
        assert_eq!(body["prompt"], "a cat surfing");
        assert_eq!(body["durationSeconds"], 5.0);
        assert_eq!(body["sourceUrl"], "https://cdn/in.png");
        assert_eq!(body["targetLanguage"], "en");
        assert_eq!(body["params"]["aspectRatio"], "16:9");
    }

    #[test]
    fn submit_body_omits_absent_optionals_and_null_params() {
        let req = GenerationRequest {
            kind: ModelKind::Image,
            model: "flux".into(),
            prompt: "a logo".into(),
            duration_seconds: None,
            source_url: None,
            target_language: None,
            params: Value::Null,
        };
        let body = build_submit_body(&req);
        assert_eq!(body["kind"], "image");
        let obj = body.as_object().unwrap();
        assert!(!obj.contains_key("durationSeconds"));
        assert!(!obj.contains_key("sourceUrl"));
        assert!(!obj.contains_key("targetLanguage"));
        assert!(!obj.contains_key("params"));
    }

    #[test]
    fn kind_tokens_are_lowercase() {
        assert_eq!(kind_token(&ModelKind::Video), "video");
        assert_eq!(kind_token(&ModelKind::Image), "image");
        assert_eq!(kind_token(&ModelKind::Audio), "audio");
        assert_eq!(kind_token(&ModelKind::Upscale), "upscale");
    }

    // ─── submit response parsing ───

    #[test]
    fn submit_2xx_yields_submission() {
        let body = serde_json::json!({ "jobId": "job-123", "status": "queued" });
        let sub = parse_submit_response(200, &body).unwrap();
        assert_eq!(sub.backend_job_id, "job-123");
    }

    #[test]
    fn submit_4xx_is_error_with_status_and_message() {
        let body = serde_json::json!({ "error": "unknown model" });
        let err = parse_submit_response(400, &body).unwrap_err();
        assert!(err.contains("400"), "{err}");
        assert!(err.contains("unknown model"), "{err}");
    }

    #[test]
    fn submit_2xx_bad_body_is_error() {
        // 2xx but no usable jobId → error (a malformed-but-JSON success body).
        let missing = serde_json::json!({ "status": "queued" });
        assert!(parse_submit_response(200, &missing).is_err());
        let empty_id = serde_json::json!({ "jobId": "" });
        assert!(parse_submit_response(200, &empty_id).is_err());
        let wrong_type = serde_json::json!({ "jobId": 7 });
        assert!(parse_submit_response(200, &wrong_type).is_err());
    }

    // ─── poll response parsing ───

    #[test]
    fn poll_succeeded_maps_to_success_with_urls() {
        let body = serde_json::json!({
            "status": "succeeded",
            "resultUrls": ["https://cdn/a.mp4", "https://cdn/b.mp4"]
        });
        assert_eq!(
            parse_poll_response(200, &body).unwrap(),
            GenerationOutcome::Success {
                result_urls: vec![
                    "https://cdn/a.mp4".to_string(),
                    "https://cdn/b.mp4".to_string()
                ]
            }
        );
    }

    #[test]
    fn poll_failed_maps_to_failure_with_reason() {
        let body = serde_json::json!({ "status": "failed", "error": "content policy" });
        assert_eq!(
            parse_poll_response(200, &body).unwrap(),
            GenerationOutcome::Failure {
                reason: "content policy".to_string()
            }
        );
    }

    #[test]
    fn poll_queued_and_running_stay_pending_as_err() {
        let queued = serde_json::json!({ "status": "queued" });
        assert_eq!(parse_poll_response(200, &queued).unwrap_err(), "still queued");
        let running = serde_json::json!({ "status": "running" });
        assert_eq!(
            parse_poll_response(200, &running).unwrap_err(),
            "still running"
        );
    }

    #[test]
    fn poll_bad_body_and_non_2xx_are_err() {
        // 2xx without a status field.
        let no_status = serde_json::json!({ "resultUrls": [] });
        assert!(parse_poll_response(200, &no_status).is_err());
        // Non-2xx transport-level status.
        let server_err = serde_json::json!({ "error": "boom" });
        let err = parse_poll_response(500, &server_err).unwrap_err();
        assert!(err.contains("500"), "{err}");
        assert!(err.contains("boom"), "{err}");
    }

    #[test]
    fn poll_succeeded_without_urls_is_a_failure_not_empty_success() {
        // A success carrying no URLs would flip the asset to ready-with-no-media
        // (a dangling done-but-empty entry); it must map to Failure instead.
        for body in [
            serde_json::json!({ "status": "succeeded" }),
            serde_json::json!({ "status": "succeeded", "resultUrls": [] }),
        ] {
            match parse_poll_response(200, &body).unwrap() {
                GenerationOutcome::Failure { reason } => {
                    assert!(reason.contains("no result URLs"), "{reason}");
                }
                other => panic!("expected Failure, got {other:?}"),
            }
        }
    }

    #[test]
    fn backend_constructs_from_config() {
        let cfg = GenerationBackendConfig::new("https://gen.example", "tok");
        assert!(HttpGenerationBackend::new(cfg).is_ok());
    }
}

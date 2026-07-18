//! Fronda Generation Protocol v1.1 wire types.
//!
//! v1 (see `specs/rust-rewrite/98-generation-protocol.md`):
//!   POST /v1/generate  → { jobId, status }
//!   GET  /v1/jobs/{id}  → { status, resultUrls?, error? }
//! v1.1 (additive, backward compatible):
//!   - generate request gains an optional `provider` field (defaults per kind).
//!   - GET /v1/providers → { video: [...], image: [...], audio: [...] } catalog.
//!
//! Field names mirror Fronda's client exactly (`http_generation_backend`):
//! `build_submit_body` emits camelCase keys, `parse_submit_response` reads
//! `jobId`, `parse_poll_response` reads `status`/`resultUrls`/`error`.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::provider::{ProviderKind, ProviderStatus};

/// `POST /v1/generate` request body. Deserializes the client's submit body
/// (kind/model/prompt + optional durationSeconds/sourceUrl/targetLanguage/params)
/// and the v1.1 optional `provider`. A v1 client that omits `provider` routes to
/// the kind's default.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateRequest {
    pub kind: ProviderKind,
    pub model: String,
    pub prompt: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_seconds: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_language: Option<String>,
    /// v1.1: explicit provider name; `None` → the kind's default provider.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    /// Opaque model-specific passthrough; omitted from the wire when null.
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub params: Value,
}

/// `POST /v1/generate` success response: `{ jobId, status }`. `jobId` is what the
/// client persists and polls; `status` is informational at submit time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmitResponse {
    pub job_id: String,
    pub status: String,
}

/// `GET /v1/jobs/{id}` response: `{ status, resultUrls?, error? }`. `resultUrls`
/// is present (non-empty) on `succeeded`; `error` on `failed`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobStatusResponse {
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_urls: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl JobStatusResponse {
    /// Project a provider status onto the poll wire shape.
    pub fn from_status(status: &ProviderStatus) -> Self {
        match status {
            ProviderStatus::Queued => Self {
                status: "queued".into(),
                result_urls: None,
                error: None,
            },
            ProviderStatus::Running => Self {
                status: "running".into(),
                result_urls: None,
                error: None,
            },
            ProviderStatus::Succeeded { urls } => Self {
                status: "succeeded".into(),
                result_urls: Some(urls.clone()),
                error: None,
            },
            ProviderStatus::Failed { reason } => Self {
                status: "failed".into(),
                result_urls: None,
                error: Some(reason.clone()),
            },
        }
    }
}

/// One provider entry in the `/v1/providers` catalog.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderCatalogEntry {
    pub name: String,
    pub models: Vec<String>,
}

/// `GET /v1/providers` response, grouped by kind so Fronda can build a picker.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ProvidersCatalog {
    pub video: Vec<ProviderCatalogEntry>,
    pub image: Vec<ProviderCatalogEntry>,
    pub audio: Vec<ProviderCatalogEntry>,
}

impl ProvidersCatalog {
    /// Mutable bucket for a kind — keeps `registry::catalog` grouping in one place.
    pub fn bucket_mut(&mut self, kind: ProviderKind) -> &mut Vec<ProviderCatalogEntry> {
        match kind {
            ProviderKind::Video => &mut self.video,
            ProviderKind::Image => &mut self.image,
            ProviderKind::Audio => &mut self.audio,
        }
    }
}

/// Error body returned for 4xx/401/5xx: `{ error }` — matches the client's
/// `error_message` extraction (`parse_submit_response` reads `error`/`message`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn generate_request_deserializes_client_wire_format() {
        // The exact object Fronda's build_submit_body produces (v1, no provider).
        let body = json!({
            "kind": "video",
            "model": "veo-3",
            "prompt": "a cat surfing",
            "durationSeconds": 5.0,
            "sourceUrl": "https://cdn/in.png",
            "targetLanguage": "en",
            "params": { "aspectRatio": "16:9" }
        });
        let req: GenerateRequest = serde_json::from_value(body).unwrap();
        assert_eq!(req.kind, ProviderKind::Video);
        assert_eq!(req.model, "veo-3");
        assert_eq!(req.prompt, "a cat surfing");
        assert_eq!(req.duration_seconds, Some(5.0));
        assert_eq!(req.source_url.as_deref(), Some("https://cdn/in.png"));
        assert_eq!(req.target_language.as_deref(), Some("en"));
        assert_eq!(req.provider, None);
        assert_eq!(req.params["aspectRatio"], "16:9");
    }

    #[test]
    fn generate_request_reads_v11_provider() {
        let body = json!({ "kind": "image", "model": "flux", "prompt": "logo", "provider": "gemini" });
        let req: GenerateRequest = serde_json::from_value(body).unwrap();
        assert_eq!(req.provider.as_deref(), Some("gemini"));
    }

    #[test]
    fn generate_request_minimal_defaults_optionals() {
        let body = json!({ "kind": "audio", "model": "m", "prompt": "p" });
        let req: GenerateRequest = serde_json::from_value(body).unwrap();
        assert_eq!(req.duration_seconds, None);
        assert_eq!(req.source_url, None);
        assert_eq!(req.target_language, None);
        assert_eq!(req.provider, None);
        assert!(req.params.is_null());
    }

    #[test]
    fn submit_response_serializes_camel_case() {
        let out = serde_json::to_value(SubmitResponse {
            job_id: "job-1".into(),
            status: "queued".into(),
        })
        .unwrap();
        assert_eq!(out, json!({ "jobId": "job-1", "status": "queued" }));
    }

    #[test]
    fn job_status_succeeded_emits_result_urls_only() {
        let out = serde_json::to_value(JobStatusResponse::from_status(
            &ProviderStatus::Succeeded {
                urls: vec!["stub://video/job-1".into()],
            },
        ))
        .unwrap();
        assert_eq!(
            out,
            json!({ "status": "succeeded", "resultUrls": ["stub://video/job-1"] })
        );
        assert!(out.as_object().unwrap().get("error").is_none());
    }

    #[test]
    fn job_status_failed_emits_error_only() {
        let out = serde_json::to_value(JobStatusResponse::from_status(
            &ProviderStatus::Failed {
                reason: "content policy".into(),
            },
        ))
        .unwrap();
        assert_eq!(out, json!({ "status": "failed", "error": "content policy" }));
        assert!(out.as_object().unwrap().get("resultUrls").is_none());
    }

    #[test]
    fn job_status_running_is_bare_status() {
        let out =
            serde_json::to_value(JobStatusResponse::from_status(&ProviderStatus::Running)).unwrap();
        assert_eq!(out, json!({ "status": "running" }));
    }

    #[test]
    fn providers_catalog_serializes_kind_buckets() {
        let mut cat = ProvidersCatalog::default();
        cat.bucket_mut(ProviderKind::Video).push(ProviderCatalogEntry {
            name: "stub".into(),
            models: vec!["stub-video".into()],
        });
        let out = serde_json::to_value(&cat).unwrap();
        assert_eq!(
            out,
            json!({
                "video": [{ "name": "stub", "models": ["stub-video"] }],
                "image": [],
                "audio": []
            })
        );
    }
}

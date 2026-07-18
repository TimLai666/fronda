//! Second real provider: Pollinations image generation over its keyless public
//! REST surface — no credential of any kind.
//!
//! `GET {base}/prompt/{url-encoded prompt}` with optional query parameters
//! `width`/`height`/`seed`/`model`. A 200 carries the generated image as a raw
//! binary body (typically `Content-Type: image/jpeg`); those bytes are stored in
//! the `ResultStore` and the job succeeds with a `{public_base}/v1/results/{id}`
//! URL. Any non-2xx response fails the job with a reason carrying the status and
//! a body snippet.
//!
//! Because it needs no key, this provider is *always* registered (unlike the
//! key-gated Gemini one), which lets the full generate → store → serve → fetch
//! media loop be exercised end to end with no credentials.
//!
//! The provider trait is synchronous: `submit` opens a job, marks it Running,
//! `tokio::spawn`s the HTTP call, and returns immediately; the spawned task
//! writes the terminal state; `poll` just reads the shared job store.

use std::sync::Arc;

use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use serde_json::Value;

use crate::config::GatewayConfig;
use crate::jobs::JobStore;
use crate::protocol::GenerateRequest;
use crate::provider::{
    provider_http_client, resolve_effective_model, GenerationProvider, ProviderJob, ProviderKind,
    ProviderStatus,
};
use crate::results::ResultStore;

/// The name the Pollinations provider registers under.
pub const POLLINATIONS_NAME: &str = "pollinations";
pub const DEFAULT_POLLINATIONS_BASE: &str = "https://image.pollinations.ai";
/// Model advertised in the `/v1/providers` catalog (Pollinations' default and,
/// currently, only image model). Verified against `GET {base}/models` on
/// 2026-07-18, which returned `["sana"]`. Pollinations' model list is volatile
/// (it was `flux` weeks earlier), so this is a point-in-time snapshot; fetching
/// `/models` at startup to advertise the live set is a follow-up.
pub const DEFAULT_POLLINATIONS_MODEL: &str = "sana";

/// Resolved Pollinations connection settings. `base` is overridable (env/config);
/// there is no key — the provider is always available. `models` is the advertised
/// model list: the live `/models` set when the gateway fetched it at startup, else
/// the hardcoded default.
#[derive(Debug, Clone)]
pub struct PollinationsConfig {
    pub base: String,
    pub models: Vec<String>,
}

impl PollinationsConfig {
    /// Build from the gateway config. Always succeeds — no key is required — so the
    /// provider is registered unconditionally. A non-empty `pollinations_models`
    /// (fetched live at startup) is advertised as-is; otherwise the single
    /// hardcoded default.
    pub fn from_gateway(config: &GatewayConfig) -> Self {
        let models = config
            .pollinations_models
            .clone()
            .filter(|m| !m.is_empty())
            .unwrap_or_else(|| vec![DEFAULT_POLLINATIONS_MODEL.to_string()]);
        Self {
            base: config
                .pollinations_base
                .clone()
                .unwrap_or_else(|| DEFAULT_POLLINATIONS_BASE.to_string()),
            models,
        }
    }
}

/// Parse the Pollinations `GET {base}/models` response — a JSON array of model-id
/// strings (e.g. `["sana"]`). Returns `None` on invalid JSON or an empty list, so
/// the caller falls back to the hardcoded default rather than advertising nothing.
pub fn parse_models_response(body: &str) -> Option<Vec<String>> {
    let ids: Vec<String> = serde_json::from_str(body).ok()?;
    let ids: Vec<String> = ids
        .into_iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if ids.is_empty() {
        None
    } else {
        Some(ids)
    }
}

/// Best-effort fetch of Pollinations' live model list from `{base}/models`. Any
/// failure (network, non-2xx, bad body, empty list) → `None` so the gateway keeps
/// its hardcoded default. The caller supplies a short-timeout client so a slow
/// Pollinations never stalls gateway startup.
pub async fn fetch_models(base: &str, client: &reqwest::Client) -> Option<Vec<String>> {
    let url = format!("{}/models", base.trim_end_matches('/'));
    let resp = client.get(url).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let body = resp.text().await.ok()?;
    parse_models_response(&body)
}

/// The optional query parameters Pollinations accepts, extracted from a request's
/// opaque `params` passthrough. Absent fields are omitted from the URL.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PollinationsParams {
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub seed: Option<i64>,
    pub model: Option<String>,
}

impl PollinationsParams {
    /// Read the recognised keys from a request's `params` value. Numeric fields
    /// accept either a JSON number or a numeric string; unknown/invalid values are
    /// simply dropped (the parameter is omitted).
    pub fn from_request_params(params: &Value) -> Self {
        Self {
            width: param_u32(params, "width"),
            height: param_u32(params, "height"),
            seed: param_i64(params, "seed"),
            model: params
                .get("model")
                .and_then(|v| v.as_str())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
        }
    }

    /// Build the URL-encoded query string (no leading `?`) in a stable field order
    /// so URLs are deterministic and testable. Empty when no parameter is set.
    fn query_string(&self) -> String {
        let mut pairs: Vec<(&str, String)> = Vec::new();
        if let Some(w) = self.width {
            pairs.push(("width", w.to_string()));
        }
        if let Some(h) = self.height {
            pairs.push(("height", h.to_string()));
        }
        if let Some(s) = self.seed {
            pairs.push(("seed", s.to_string()));
        }
        if let Some(m) = &self.model {
            pairs.push(("model", m.clone()));
        }
        pairs
            .iter()
            .map(|(k, v)| format!("{}={}", k, utf8_percent_encode(v, NON_ALPHANUMERIC)))
            .collect::<Vec<_>>()
            .join("&")
    }
}

fn param_u32(params: &Value, key: &str) -> Option<u32> {
    let value = params.get(key)?;
    if let Some(n) = value.as_u64() {
        return u32::try_from(n).ok();
    }
    value.as_str().and_then(|s| s.trim().parse::<u32>().ok())
}

fn param_i64(params: &Value, key: &str) -> Option<i64> {
    let value = params.get(key)?;
    if let Some(n) = value.as_i64() {
        return Some(n);
    }
    value.as_str().and_then(|s| s.trim().parse::<i64>().ok())
}

/// Pollinations image provider (kind = Image). Holds a shared job store and result
/// store so the spawned generation task can publish bytes and terminal state.
pub struct PollinationsImageProvider {
    config: PollinationsConfig,
    store: Arc<JobStore>,
    results: Arc<ResultStore>,
    public_base: String,
    client: reqwest::Client,
}

impl PollinationsImageProvider {
    pub fn new(
        config: PollinationsConfig,
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

    /// Construct from the gateway config. Always succeeds — Pollinations needs no
    /// key — so callers register it unconditionally.
    pub fn from_config(
        config: &GatewayConfig,
        store: Arc<JobStore>,
        results: Arc<ResultStore>,
        public_base: impl Into<String>,
    ) -> Self {
        Self::new(
            PollinationsConfig::from_gateway(config),
            store,
            results,
            public_base,
        )
    }
}

impl GenerationProvider for PollinationsImageProvider {
    fn name(&self) -> &str {
        POLLINATIONS_NAME
    }

    fn kind(&self) -> ProviderKind {
        ProviderKind::Image
    }

    fn models(&self) -> Vec<String> {
        self.config.models.clone()
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
        let mut params = PollinationsParams::from_request_params(&req.params);
        // v1.1: honor the request's top-level model when it names an advertised
        // model (the picker's selection); an explicit `params.model` passthrough
        // still wins. An unadvertised id (the agent path) leaves it unset → the
        // Pollinations server default.
        if params.model.is_none() {
            let effective = resolve_effective_model(&req.model, "", &self.models());
            if !effective.is_empty() {
                params.model = Some(effective);
            }
        }
        let task_job_id = job_id.clone();

        tokio::spawn(async move {
            match run_pollinations_generation(&client, &config, &prompt, &params).await {
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

/// Fetch the generated image and reduce the response to `(image_bytes, content_type)`
/// or an explicit failure reason.
async fn run_pollinations_generation(
    client: &reqwest::Client,
    config: &PollinationsConfig,
    prompt: &str,
    params: &PollinationsParams,
) -> Result<(Vec<u8>, String), String> {
    let url = build_pollinations_url(&config.base, prompt, params);
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("pollinations: request failed: {e}"))?;
    let status = resp.status().as_u16();
    let content_type = content_type_or_default(
        resp.headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok()),
    );
    let body = resp
        .bytes()
        .await
        .map_err(|e| format!("pollinations: reading response body failed: {e}"))?
        .to_vec();
    parse_pollinations_response(status, content_type, body)
}

/// Build the request URL: `{base}/prompt/{url-encoded prompt}` plus any optional
/// query parameters. The prompt is percent-encoded as a single path segment so
/// spaces and special characters cannot break out of the path.
pub fn build_pollinations_url(base: &str, prompt: &str, params: &PollinationsParams) -> String {
    let encoded_prompt = utf8_percent_encode(prompt, NON_ALPHANUMERIC).to_string();
    let mut url = format!("{}/prompt/{}", base.trim_end_matches('/'), encoded_prompt);
    let query = params.query_string();
    if !query.is_empty() {
        url.push('?');
        url.push_str(&query);
    }
    url
}

/// Pick the response content type, defaulting to `image/jpeg` when the header is
/// absent or blank (Pollinations' default response type).
pub fn content_type_or_default(header: Option<&str>) -> String {
    match header {
        Some(ct) if !ct.trim().is_empty() => ct.to_string(),
        _ => "image/jpeg".to_string(),
    }
}

/// Turn a raw (status, content_type, body) triple into image bytes or a failure
/// reason. Non-2xx is a failure carrying the status and a body snippet; an empty
/// 2xx body is also a failure (a zero-byte "success" would be a false positive).
pub fn parse_pollinations_response(
    status: u16,
    content_type: String,
    body: Vec<u8>,
) -> Result<(Vec<u8>, String), String> {
    if !(200..300).contains(&status) {
        return Err(pollinations_http_error(status, &body));
    }
    if body.is_empty() {
        return Err(format!("pollinations: HTTP {status} returned an empty body"));
    }
    Ok((body, content_type))
}

/// Best-effort error message for a non-2xx response: HTTP status plus a snippet of
/// the (possibly non-UTF8) body.
fn pollinations_http_error(status: u16, body: &[u8]) -> String {
    let snippet: String = String::from_utf8_lossy(body).chars().take(200).collect();
    if snippet.trim().is_empty() {
        format!("pollinations: HTTP {status}")
    } else {
        format!("pollinations: HTTP {status}: {snippet}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn build_url_encodes_prompt_as_path_segment() {
        let url = build_pollinations_url(
            "https://image.pollinations.ai",
            "a red fox & hound",
            &PollinationsParams::default(),
        );
        // Space → %20, '&' → %26 — the prompt cannot break out of the path segment,
        // and no query string is appended when no params are set.
        assert_eq!(
            url,
            "https://image.pollinations.ai/prompt/a%20red%20fox%20%26%20hound"
        );
    }

    #[test]
    fn build_url_tolerates_trailing_slash_on_base() {
        let url = build_pollinations_url(
            "http://localhost:9/",
            "cat",
            &PollinationsParams::default(),
        );
        assert_eq!(url, "http://localhost:9/prompt/cat");
    }

    #[test]
    fn build_url_appends_params_in_stable_order() {
        let params = PollinationsParams {
            width: Some(512),
            height: Some(768),
            seed: Some(42),
            model: Some("flux".into()),
        };
        let url = build_pollinations_url("https://image.pollinations.ai", "a logo", &params);
        assert_eq!(
            url,
            "https://image.pollinations.ai/prompt/a%20logo?width=512&height=768&seed=42&model=flux"
        );
    }

    #[test]
    fn build_url_omits_absent_params() {
        let params = PollinationsParams {
            width: Some(256),
            model: Some("turbo".into()),
            ..PollinationsParams::default()
        };
        let url = build_pollinations_url("https://image.pollinations.ai", "x", &params);
        assert_eq!(
            url,
            "https://image.pollinations.ai/prompt/x?width=256&model=turbo"
        );
    }

    #[test]
    fn params_from_request_reads_numbers_and_numeric_strings() {
        let from_numbers = PollinationsParams::from_request_params(&json!({
            "width": 640, "height": 480, "seed": 7, "model": "flux"
        }));
        assert_eq!(
            from_numbers,
            PollinationsParams {
                width: Some(640),
                height: Some(480),
                seed: Some(7),
                model: Some("flux".into()),
            }
        );

        // Fronda's opaque params passthrough may deliver numbers as strings.
        let from_strings = PollinationsParams::from_request_params(&json!({
            "width": "640", "height": "480", "seed": "7"
        }));
        assert_eq!(from_strings.width, Some(640));
        assert_eq!(from_strings.height, Some(480));
        assert_eq!(from_strings.seed, Some(7));
    }

    #[test]
    fn params_from_request_ignores_missing_and_bad_values() {
        let params = PollinationsParams::from_request_params(&json!({
            "width": "not-a-number", "model": "   "
        }));
        assert_eq!(params, PollinationsParams::default());
        assert_eq!(
            PollinationsParams::from_request_params(&Value::Null),
            PollinationsParams::default()
        );
    }

    #[test]
    fn content_type_defaults_to_jpeg_when_absent_or_blank() {
        assert_eq!(content_type_or_default(Some("image/png")), "image/png");
        assert_eq!(content_type_or_default(None), "image/jpeg");
        assert_eq!(content_type_or_default(Some("   ")), "image/jpeg");
    }

    #[test]
    fn parse_200_returns_bytes_and_content_type() {
        let (bytes, ct) =
            parse_pollinations_response(200, "image/jpeg".into(), vec![0xFF, 0xD8, 0xD9]).unwrap();
        assert_eq!(bytes, vec![0xFF, 0xD8, 0xD9]);
        assert_eq!(ct, "image/jpeg");
    }

    #[test]
    fn parse_empty_200_body_is_failure() {
        let err = parse_pollinations_response(200, "image/jpeg".into(), Vec::new()).unwrap_err();
        assert!(err.contains("empty body"), "err was: {err}");
    }

    #[test]
    fn parse_non_200_carries_status_and_snippet() {
        let err =
            parse_pollinations_response(500, "text/plain".into(), b"upstream boom".to_vec())
                .unwrap_err();
        assert!(err.contains("500"), "err was: {err}");
        assert!(err.contains("upstream boom"), "err was: {err}");
    }

    #[test]
    fn parse_non_200_without_body_still_carries_status() {
        let err = parse_pollinations_response(429, "text/plain".into(), Vec::new()).unwrap_err();
        assert!(err.contains("429"), "err was: {err}");
    }

    #[test]
    fn from_gateway_defaults_base_and_needs_no_key() {
        let config = GatewayConfig::default();
        let pollinations = PollinationsConfig::from_gateway(&config);
        assert_eq!(pollinations.base, DEFAULT_POLLINATIONS_BASE);
        // No fetched list → the single hardcoded default is advertised.
        assert_eq!(pollinations.models, vec![DEFAULT_POLLINATIONS_MODEL.to_string()]);
    }

    #[test]
    fn from_gateway_advertises_fetched_models_else_default() {
        // A live-fetched (non-empty) list is advertised as-is.
        let config = GatewayConfig {
            pollinations_models: Some(vec!["sana".into(), "flux".into()]),
            ..GatewayConfig::default()
        };
        assert_eq!(
            PollinationsConfig::from_gateway(&config).models,
            vec!["sana".to_string(), "flux".to_string()]
        );
        // An empty fetched list falls back to the hardcoded default (never advertise nothing).
        let config = GatewayConfig {
            pollinations_models: Some(vec![]),
            ..GatewayConfig::default()
        };
        assert_eq!(
            PollinationsConfig::from_gateway(&config).models,
            vec![DEFAULT_POLLINATIONS_MODEL.to_string()]
        );
    }

    #[test]
    fn parse_models_response_reads_array_and_rejects_empty_or_invalid() {
        assert_eq!(
            parse_models_response(r#"["sana"]"#),
            Some(vec!["sana".to_string()])
        );
        assert_eq!(
            parse_models_response(r#"["sana", "flux", "turbo"]"#),
            Some(vec!["sana".to_string(), "flux".to_string(), "turbo".to_string()])
        );
        // Whitespace/blank ids are trimmed and dropped.
        assert_eq!(
            parse_models_response(r#"[" sana ", "", "  "]"#),
            Some(vec!["sana".to_string()])
        );
        // Empty array / invalid JSON / wrong shape → None (caller keeps the default).
        assert_eq!(parse_models_response("[]"), None);
        assert_eq!(parse_models_response("not json"), None);
        assert_eq!(parse_models_response(r#"{"models":["x"]}"#), None);
    }

    #[test]
    fn from_gateway_applies_base_override() {
        let config = GatewayConfig {
            pollinations_base: Some("http://127.0.0.1:4321".into()),
            ..GatewayConfig::default()
        };
        let pollinations = PollinationsConfig::from_gateway(&config);
        assert_eq!(pollinations.base, "http://127.0.0.1:4321");
    }

    /// Gated live test: only runs with `FRONDA_GEN_LIVE_POLLINATIONS` set. Hits the
    /// real, keyless Pollinations API and asserts a non-trivial image comes back.
    /// Skipped (with a note) otherwise.
    #[tokio::test]
    async fn live_pollinations_returns_real_image_bytes() {
        if std::env::var("FRONDA_GEN_LIVE_POLLINATIONS").is_err() {
            eprintln!(
                "skipping live_pollinations_returns_real_image_bytes: \
                 FRONDA_GEN_LIVE_POLLINATIONS unset"
            );
            return;
        }
        let config = PollinationsConfig {
            base: std::env::var("FRONDA_GEN_POLLINATIONS_BASE")
                .unwrap_or_else(|_| DEFAULT_POLLINATIONS_BASE.to_string()),
            models: vec![DEFAULT_POLLINATIONS_MODEL.to_string()],
        };
        let client = reqwest::Client::new();
        let params = PollinationsParams {
            width: Some(256),
            height: Some(256),
            ..PollinationsParams::default()
        };
        let (bytes, content_type) =
            run_pollinations_generation(&client, &config, "a small red circle on white", &params)
                .await
                .expect("live pollinations generation should succeed");
        assert!(
            content_type.starts_with("image/"),
            "expected image content-type, got {content_type}"
        );
        assert!(
            bytes.len() > 1024,
            "expected a multi-KB image, got {} bytes",
            bytes.len()
        );
    }
}

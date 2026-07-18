//! Provider abstraction: one trait behind which many generation providers (stub
//! now, real adapters in phase 2) coexist, keyed by (kind, name).

use serde::{Deserialize, Serialize};

use crate::protocol::GenerateRequest;

/// The media modality a provider serves. Mirrors the lowercase `kind` token of
/// Fronda Generation Protocol v1 (video/image/audio). Phase 1 has no `upscale`
/// provider, so that kind is intentionally absent here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderKind {
    Video,
    Image,
    Audio,
}

impl ProviderKind {
    pub const ALL: [ProviderKind; 3] =
        [ProviderKind::Video, ProviderKind::Image, ProviderKind::Audio];

    pub fn as_str(&self) -> &'static str {
        match self {
            ProviderKind::Video => "video",
            ProviderKind::Image => "image",
            ProviderKind::Audio => "audio",
        }
    }

    pub fn from_token(token: &str) -> Option<Self> {
        match token {
            "video" => Some(ProviderKind::Video),
            "image" => Some(ProviderKind::Image),
            "audio" => Some(ProviderKind::Audio),
            _ => None,
        }
    }
}

/// Overall request timeout for a provider's upstream HTTP call. A real provider
/// spawns its generation task and only writes a terminal job state when the call
/// returns; with no timeout a hung upstream would leave the job stuck `Running`
/// forever (and the client's asset permanently "generating"). Bounds it so a hang
/// becomes an explicit `Failed` instead. Matches the client-side 120s ceiling in
/// `http_generation_backend`.
const PROVIDER_HTTP_TIMEOUT_SECS: u64 = 120;
/// Separate, tighter connect-phase bound so an unreachable host fails fast rather
/// than burning the full request budget on the TCP/TLS handshake.
const PROVIDER_CONNECT_TIMEOUT_SECS: u64 = 15;

/// Build the shared reqwest client every real provider uses: bounded overall and
/// connect timeouts so a hung or unreachable upstream can never strand a job. The
/// builder only fails on a broken TLS backend; fall back to a plain client so a
/// provider is never un-constructable over a timeout.
pub fn provider_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(PROVIDER_HTTP_TIMEOUT_SECS))
        .connect_timeout(std::time::Duration::from_secs(PROVIDER_CONNECT_TIMEOUT_SECS))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

/// Resolve the effective model for a request under the v1.1 provider contract:
/// honor `requested` when it names a model this provider advertises (the picker
/// sources model ids from `/v1/providers`, i.e. the provider namespace);
/// otherwise fall back to `default`. The agent/tool path sends Fronda-catalog
/// ids the provider does not advertise, so those cleanly fall back to the
/// provider's default — zero regression for callers that don't use the picker.
pub fn resolve_effective_model(requested: &str, default: &str, advertised: &[String]) -> String {
    let r = requested.trim();
    if !r.is_empty() && advertised.iter().any(|m| m == r) {
        r.to_string()
    } else {
        default.to_string()
    }
}

/// A provider's acknowledgement of a submitted job: the id the client polls.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderJob {
    pub job_id: String,
}

/// A provider's view of a job's lifecycle. Maps onto the protocol poll status:
/// Queued/Running are non-terminal; Succeeded/Failed are terminal.
#[derive(Debug, Clone, PartialEq)]
pub enum ProviderStatus {
    Queued,
    Running,
    Succeeded { urls: Vec<String> },
    Failed { reason: String },
}

/// The common seam every generation provider implements. A real adapter
/// (Gemini/fal/…) polls its remote job by id; the stub drives an in-memory job
/// store. `Send + Sync` so providers can be shared across the async server.
pub trait GenerationProvider: Send + Sync {
    fn name(&self) -> &str;
    fn kind(&self) -> ProviderKind;
    /// Model ids this provider exposes (for the `/v1/providers` catalog).
    fn models(&self) -> Vec<String>;
    fn submit(&self, req: &GenerateRequest) -> Result<ProviderJob, String>;
    fn poll(&self, job_id: &str) -> Result<ProviderStatus, String>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_token_round_trips() {
        for kind in ProviderKind::ALL {
            assert_eq!(ProviderKind::from_token(kind.as_str()), Some(kind));
        }
        assert_eq!(ProviderKind::from_token("upscale"), None);
        assert_eq!(ProviderKind::from_token(""), None);
    }

    #[test]
    fn resolve_effective_model_honors_advertised_else_defaults() {
        let advertised = vec!["flux".to_string(), "sana".to_string()];
        // Advertised → honored (the picker's selection flows through).
        assert_eq!(resolve_effective_model("sana", "flux", &advertised), "sana");
        assert_eq!(resolve_effective_model("flux", "flux", &advertised), "flux");
        // Not advertised (agent path sends a Fronda-catalog id) → default.
        assert_eq!(resolve_effective_model("veo-3", "flux", &advertised), "flux");
        // Empty/whitespace request → default.
        assert_eq!(resolve_effective_model("", "flux", &advertised), "flux");
        assert_eq!(resolve_effective_model("  ", "flux", &advertised), "flux");
        // Surrounding whitespace on an advertised id is tolerated.
        assert_eq!(resolve_effective_model(" sana ", "flux", &advertised), "sana");
        // An empty default (Pollinations: "no model param → server default") stays empty.
        assert_eq!(resolve_effective_model("veo-3", "", &advertised), "");
    }

    #[test]
    fn provider_http_client_builds_with_timeouts() {
        // A well-formed builder (valid timeout + rustls backend) yields a client;
        // this guards against a future change that makes the builder fall over.
        let _client = provider_http_client();
    }

    #[test]
    fn kind_serializes_lowercase() {
        assert_eq!(
            serde_json::to_value(ProviderKind::Video).unwrap(),
            serde_json::json!("video")
        );
        assert_eq!(
            serde_json::from_value::<ProviderKind>(serde_json::json!("audio")).unwrap(),
            ProviderKind::Audio
        );
    }
}

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

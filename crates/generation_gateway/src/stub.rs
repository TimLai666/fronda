//! Phase-1 stub provider. Drives the full submit → poll → succeeded loop with no
//! external key so Fronda → gateway → result works end to end. A real adapter
//! (Gemini/fal/…) implements the same `GenerationProvider` trait in phase 2.

use std::sync::Arc;

use crate::jobs::JobStore;
use crate::protocol::GenerateRequest;
use crate::provider::{GenerationProvider, ProviderJob, ProviderKind, ProviderStatus};

/// The single name every stub provider registers under (one stub per kind).
pub const STUB_NAME: &str = "stub";

/// Default placeholder URL scheme; phase 1 delivers a non-fetchable marker URL.
pub const DEFAULT_URL_BASE: &str = "stub://";

/// A stub provider for one kind. Ignores any BYO key. `submit` opens a job in the
/// shared store; `poll` reports `running` on the first poll and `succeeded` (with
/// a `stub://{kind}/{jobid}` URL) on the second, so the client's poll loop runs.
pub struct StubProvider {
    kind: ProviderKind,
    name: String,
    model: String,
    store: Arc<JobStore>,
    url_base: String,
}

impl StubProvider {
    pub fn new(kind: ProviderKind, store: Arc<JobStore>) -> Self {
        Self::with_url_base(kind, store, DEFAULT_URL_BASE)
    }

    pub fn with_url_base(kind: ProviderKind, store: Arc<JobStore>, url_base: &str) -> Self {
        Self {
            kind,
            name: STUB_NAME.to_string(),
            model: format!("stub-{}", kind.as_str()),
            store,
            url_base: url_base.to_string(),
        }
    }

    fn result_url(&self, job_id: &str) -> String {
        format!("{}{}/{}", self.url_base, self.kind.as_str(), job_id)
    }
}

impl GenerationProvider for StubProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn kind(&self) -> ProviderKind {
        self.kind
    }

    fn models(&self) -> Vec<String> {
        vec![self.model.clone()]
    }

    fn submit(&self, _req: &GenerateRequest) -> Result<ProviderJob, String> {
        // BYO-key fields are intentionally ignored by the stub.
        let job_id = self.store.create(self.kind, &self.name);
        Ok(ProviderJob { job_id })
    }

    fn poll(&self, job_id: &str) -> Result<ProviderStatus, String> {
        let url = self.result_url(job_id);
        let updated = self.store.update(job_id, |rec| {
            rec.poll_count += 1;
            if rec.poll_count >= 2 {
                rec.result_urls = vec![url.clone()];
                rec.status = ProviderStatus::Succeeded {
                    urls: vec![url.clone()],
                };
            } else {
                rec.status = ProviderStatus::Running;
            }
        });
        match updated {
            Some(rec) => Ok(rec.status),
            None => Err(format!("unknown job: {job_id}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn request(kind: ProviderKind) -> GenerateRequest {
        GenerateRequest {
            kind,
            model: "stub".into(),
            prompt: "p".into(),
            duration_seconds: None,
            source_url: None,
            target_language: None,
            provider: None,
            params: Value::Null,
        }
    }

    #[test]
    fn identity_and_models() {
        let store = Arc::new(JobStore::new());
        let stub = StubProvider::new(ProviderKind::Video, store);
        assert_eq!(stub.name(), "stub");
        assert_eq!(stub.kind(), ProviderKind::Video);
        assert_eq!(stub.models(), vec!["stub-video".to_string()]);
    }

    #[test]
    fn submit_opens_a_queued_job() {
        let store = Arc::new(JobStore::new());
        let stub = StubProvider::new(ProviderKind::Image, store.clone());
        let job = stub.submit(&request(ProviderKind::Image)).unwrap();
        assert!(!job.job_id.is_empty());
        let rec = store.get(&job.job_id).unwrap();
        assert_eq!(rec.status, ProviderStatus::Queued);
        assert_eq!(rec.poll_count, 0);
    }

    #[test]
    fn poll_transitions_running_then_succeeded() {
        let store = Arc::new(JobStore::new());
        let stub = StubProvider::new(ProviderKind::Video, store);
        let job = stub.submit(&request(ProviderKind::Video)).unwrap();

        assert_eq!(stub.poll(&job.job_id).unwrap(), ProviderStatus::Running);

        match stub.poll(&job.job_id).unwrap() {
            ProviderStatus::Succeeded { urls } => {
                assert_eq!(urls.len(), 1);
                assert_eq!(urls[0], format!("stub://video/{}", job.job_id));
            }
            other => panic!("expected Succeeded, got {other:?}"),
        }
    }

    #[test]
    fn poll_stays_succeeded_after_second_transition() {
        let store = Arc::new(JobStore::new());
        let stub = StubProvider::new(ProviderKind::Audio, store);
        let job = stub.submit(&request(ProviderKind::Audio)).unwrap();
        let _ = stub.poll(&job.job_id).unwrap(); // running
        let _ = stub.poll(&job.job_id).unwrap(); // succeeded
        assert!(matches!(
            stub.poll(&job.job_id).unwrap(),
            ProviderStatus::Succeeded { .. }
        ));
    }

    #[test]
    fn poll_unknown_job_errs() {
        let store = Arc::new(JobStore::new());
        let stub = StubProvider::new(ProviderKind::Video, store);
        assert!(stub.poll("job-does-not-exist").is_err());
    }
}

//! In-memory job store shared by the server (for poll routing) and the stub
//! providers (for lifecycle state). A `JobRecord` remembers which provider owns
//! a job so `GET /v1/jobs/{id}` — which carries only the id — can route the poll
//! back to the right provider.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use crate::provider::{ProviderKind, ProviderStatus};

/// Insertion-order cap on live job records. A safety-net against unbounded
/// memory growth in a long-running / network-exposed gateway (the network bind
/// is a supported mode — see `GatewayConfig::validate`). Generous so realistic
/// single-user use never evicts a job before its terminal poll; the oldest
/// (most likely already-terminal-and-polled) records are dropped first.
pub const DEFAULT_JOB_CAP: usize = 4096;

/// A single job's record: owning provider, current status, how many times it has
/// been polled (the stub uses this to advance queued → running → succeeded), and
/// any delivered result URLs.
#[derive(Debug, Clone)]
pub struct JobRecord {
    pub provider_kind: ProviderKind,
    pub provider_name: String,
    pub status: ProviderStatus,
    pub poll_count: u32,
    pub result_urls: Vec<String>,
}

/// The map plus an insertion-order queue so the store can evict oldest-first
/// once it exceeds its cap, under a single lock (map and order never diverge).
#[derive(Default)]
struct JobMap {
    map: HashMap<String, JobRecord>,
    order: VecDeque<String>,
}

/// Thread-safe job store. `seq` gives deterministic, monotonic job ids
/// (`job-1`, `job-2`, …) across all providers. `cap` bounds live records.
pub struct JobStore {
    inner: Mutex<JobMap>,
    seq: AtomicU64,
    cap: usize,
}

impl Default for JobStore {
    fn default() -> Self {
        Self::new()
    }
}

impl JobStore {
    pub fn new() -> Self {
        Self::with_cap(DEFAULT_JOB_CAP)
    }

    /// Store with an explicit cap (≥1). Used by tests to force eviction.
    pub fn with_cap(cap: usize) -> Self {
        Self {
            inner: Mutex::new(JobMap::default()),
            seq: AtomicU64::new(0),
            cap: cap.max(1),
        }
    }

    /// Allocate a fresh id and insert a `Queued` record owned by `(kind, name)`.
    /// Evicts the oldest record(s) if this pushes the store past its cap.
    /// Returns the new job id.
    pub fn create(&self, kind: ProviderKind, name: &str) -> String {
        let n = self.seq.fetch_add(1, Ordering::Relaxed) + 1;
        let id = format!("job-{n}");
        let record = JobRecord {
            provider_kind: kind,
            provider_name: name.to_string(),
            status: ProviderStatus::Queued,
            poll_count: 0,
            result_urls: Vec::new(),
        };
        let mut guard = self.inner.lock().unwrap();
        guard.map.insert(id.clone(), record);
        guard.order.push_back(id.clone());
        while guard.order.len() > self.cap {
            if let Some(old) = guard.order.pop_front() {
                guard.map.remove(&old);
            }
        }
        id
    }

    pub fn get(&self, id: &str) -> Option<JobRecord> {
        self.inner.lock().unwrap().map.get(id).cloned()
    }

    /// The provider that owns a job, for routing a poll request.
    pub fn provider_of(&self, id: &str) -> Option<(ProviderKind, String)> {
        self.inner
            .lock()
            .unwrap()
            .map
            .get(id)
            .map(|r| (r.provider_kind, r.provider_name.clone()))
    }

    /// Mutate a record under lock; returns the updated clone, or `None` if the id
    /// is unknown. Transition logic lives in the caller (the provider), keeping
    /// this store generic.
    pub fn update<F: FnOnce(&mut JobRecord)>(&self, id: &str, mutate: F) -> Option<JobRecord> {
        let mut guard = self.inner.lock().unwrap();
        let record = guard.map.get_mut(id)?;
        mutate(record);
        Some(record.clone())
    }

    pub fn len(&self) -> usize {
        self.inner.lock().unwrap().map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_inserts_queued_record_with_monotonic_ids() {
        let store = JobStore::new();
        let a = store.create(ProviderKind::Video, "stub");
        let b = store.create(ProviderKind::Image, "stub");
        assert_eq!(a, "job-1");
        assert_eq!(b, "job-2");
        assert_eq!(store.len(), 2);
        let rec = store.get(&a).unwrap();
        assert_eq!(rec.provider_kind, ProviderKind::Video);
        assert_eq!(rec.provider_name, "stub");
        assert_eq!(rec.status, ProviderStatus::Queued);
        assert_eq!(rec.poll_count, 0);
    }

    #[test]
    fn provider_of_routes_by_id() {
        let store = JobStore::new();
        let id = store.create(ProviderKind::Audio, "stub");
        assert_eq!(
            store.provider_of(&id),
            Some((ProviderKind::Audio, "stub".to_string()))
        );
        assert_eq!(store.provider_of("job-999"), None);
    }

    #[test]
    fn create_evicts_oldest_past_cap() {
        let store = JobStore::with_cap(3);
        let ids: Vec<String> = (0..5)
            .map(|_| store.create(ProviderKind::Image, "stub"))
            .collect();
        // Bounded at the cap; the two oldest (job-1, job-2) were evicted.
        assert_eq!(store.len(), 3);
        assert!(store.get(&ids[0]).is_none(), "oldest evicted");
        assert!(store.get(&ids[1]).is_none(), "2nd-oldest evicted");
        assert!(store.get(&ids[2]).is_some());
        assert!(store.get(&ids[4]).is_some(), "newest retained");
        // Ids stay monotonic across eviction (seq is never reused).
        assert_eq!(ids[4], "job-5");
    }

    #[test]
    fn update_mutates_and_returns_clone() {
        let store = JobStore::new();
        let id = store.create(ProviderKind::Video, "stub");
        let updated = store
            .update(&id, |rec| {
                rec.poll_count += 1;
                rec.status = ProviderStatus::Running;
            })
            .unwrap();
        assert_eq!(updated.poll_count, 1);
        assert_eq!(updated.status, ProviderStatus::Running);
        assert!(store.update("missing", |_| {}).is_none());
    }
}

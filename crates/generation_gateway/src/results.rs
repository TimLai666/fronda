//! In-memory result store for generated media. A provider decodes real bytes
//! (e.g. a Gemini `inlineData` image), `put`s them here, and hands back a
//! `{public_base}/v1/results/{id}` URL on the succeeded job. `GET /v1/results/{id}`
//! serves those exact bytes with the stored content-type, so a succeeded job's
//! resultUrls are fetchable real media rather than placeholder schemes.

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

/// Insertion-order cap on stored results. Results are large (whole images), so a
/// long-running / network-exposed gateway must not grow this without bound.
/// Generous, since the normal flow fetches a result seconds after it is stored
/// (and Fronda then caches it, #135), so the oldest — most likely already
/// fetched — are the ones dropped. Tunable; a busy multi-user deployment that
/// serves results lazily may want it higher (or provider-hosted URLs instead).
pub const DEFAULT_RESULT_CAP: usize = 256;

/// The map plus an insertion-order queue so eviction drops oldest-first under a
/// single lock (map and order never diverge).
#[derive(Default)]
struct ResultMap {
    map: HashMap<String, (Vec<u8>, String)>,
    order: VecDeque<String>,
}

/// Thread-safe store of `id → (bytes, content_type)`. Ids are unguessable random
/// tokens (UUID v4): `/v1/results/{id}` is an unauthenticated **capability URL**
/// so Fronda's generic media downloader can fetch it without the gateway token,
/// while the id's unguessability is what gates access. Bounded by `cap`.
pub struct ResultStore {
    inner: Mutex<ResultMap>,
    cap: usize,
}

impl Default for ResultStore {
    fn default() -> Self {
        Self::new()
    }
}

impl ResultStore {
    pub fn new() -> Self {
        Self::with_cap(DEFAULT_RESULT_CAP)
    }

    /// Store with an explicit cap (≥1). Used by tests to force eviction.
    pub fn with_cap(cap: usize) -> Self {
        Self {
            inner: Mutex::new(ResultMap::default()),
            cap: cap.max(1),
        }
    }

    /// Store bytes under a fresh unguessable id and return that id. Evicts the
    /// oldest result(s) if this pushes the store past its cap.
    pub fn put(&self, bytes: Vec<u8>, content_type: impl Into<String>) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        let mut guard = self.inner.lock().unwrap();
        guard.map.insert(id.clone(), (bytes, content_type.into()));
        guard.order.push_back(id.clone());
        while guard.order.len() > self.cap {
            if let Some(old) = guard.order.pop_front() {
                guard.map.remove(&old);
            }
        }
        id
    }

    /// Fetch `(bytes, content_type)` for an id, or `None` if unknown.
    pub fn get(&self, id: &str) -> Option<(Vec<u8>, String)> {
        self.inner.lock().unwrap().map.get(id).cloned()
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
    fn put_returns_unguessable_unique_ids_and_get_round_trips_bytes() {
        let store = ResultStore::new();
        let a = store.put(vec![1, 2, 3], "image/png");
        let b = store.put(vec![9, 9], "image/jpeg");
        // Capability-URL model: ids are unguessable UUIDs, distinct, not a
        // predictable "result-N" sequence an attacker could enumerate.
        assert_ne!(a, b);
        assert!(!a.contains("result-"));
        assert_eq!(a.len(), 36, "uuid v4 hyphenated form");
        assert_eq!(store.len(), 2);

        let (bytes, ct) = store.get(&a).unwrap();
        assert_eq!(bytes, vec![1, 2, 3]);
        assert_eq!(ct, "image/png");

        let (bytes, ct) = store.get(&b).unwrap();
        assert_eq!(bytes, vec![9, 9]);
        assert_eq!(ct, "image/jpeg");
    }

    #[test]
    fn get_unknown_id_is_none() {
        let store = ResultStore::new();
        assert!(store.get("00000000-0000-0000-0000-000000000000").is_none());
        assert!(store.is_empty());
    }

    #[test]
    fn put_evicts_oldest_past_cap() {
        let store = ResultStore::with_cap(2);
        let a = store.put(vec![1], "image/png");
        let b = store.put(vec![2], "image/png");
        let c = store.put(vec![3], "image/png");
        // Bounded at the cap; the oldest (a) was evicted, newest two retained.
        assert_eq!(store.len(), 2);
        assert!(store.get(&a).is_none(), "oldest result evicted");
        assert_eq!(store.get(&b).unwrap().0, vec![2]);
        assert_eq!(store.get(&c).unwrap().0, vec![3]);
    }
}

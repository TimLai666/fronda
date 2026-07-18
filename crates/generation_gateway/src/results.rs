//! In-memory result store for generated media. A provider decodes real bytes
//! (e.g. a Gemini `inlineData` image), `put`s them here, and hands back a
//! `{public_base}/v1/results/{id}` URL on the succeeded job. `GET /v1/results/{id}`
//! serves those exact bytes with the stored content-type, so a succeeded job's
//! resultUrls are fetchable real media rather than placeholder schemes.

use std::collections::HashMap;
use std::sync::Mutex;

/// Thread-safe store of `id → (bytes, content_type)`. Ids are unguessable random
/// tokens (UUID v4): `/v1/results/{id}` is an unauthenticated **capability URL**
/// so Fronda's generic media downloader can fetch it without the gateway token,
/// while the id's unguessability is what gates access.
#[derive(Default)]
pub struct ResultStore {
    inner: Mutex<HashMap<String, (Vec<u8>, String)>>,
}

impl ResultStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Store bytes under a fresh unguessable id and return that id.
    pub fn put(&self, bytes: Vec<u8>, content_type: impl Into<String>) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        self.inner
            .lock()
            .unwrap()
            .insert(id.clone(), (bytes, content_type.into()));
        id
    }

    /// Fetch `(bytes, content_type)` for an id, or `None` if unknown.
    pub fn get(&self, id: &str) -> Option<(Vec<u8>, String)> {
        self.inner.lock().unwrap().get(id).cloned()
    }

    pub fn len(&self) -> usize {
        self.inner.lock().unwrap().len()
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
}

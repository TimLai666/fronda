//! EmbeddingStore — persists and queries VisualEmbedding vectors.
//!
//! Mirrors Swift `EmbeddingStore.swift` + `VisualSearch.swift`.
//!
//! Storage: in-memory flat index (linear scan).  For production, callers can
//! swap in an HNSW or SQLite-vec backend via the `SearchIndex` trait.

use crate::embedder::VisualEmbedding;
use serde::{Deserialize, Serialize};

/// A ranked result from a visual search query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// Media entry ID.
    pub entry_id: String,
    /// Best cosine similarity score across all frames in this entry (0.0..=1.0).
    pub score: f32,
    /// Sample index of the best-matching frame.
    pub best_frame_index: usize,
}

/// In-memory flat embedding store with cosine-similarity search.
///
/// Thread-safety: not `Sync` — wrap in `Mutex` or `RwLock` at the call site.
#[derive(Debug, Default)]
pub struct EmbeddingStore {
    entries: Vec<VisualEmbedding>,
}

impl EmbeddingStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert or replace all embeddings for `entry_id`.
    pub fn upsert(&mut self, embeddings: Vec<VisualEmbedding>) {
        if let Some(first) = embeddings.first() {
            let id = &first.entry_id;
            self.entries.retain(|e| &e.entry_id != id);
        }
        self.entries.extend(embeddings);
    }

    /// Remove all embeddings for `entry_id`.
    pub fn remove(&mut self, entry_id: &str) {
        self.entries.retain(|e| e.entry_id != entry_id);
    }

    /// Number of frame embeddings currently stored.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Query with a text/image embedding vector, return top-k results by entry.
    pub fn search(&self, query: &[f32], top_k: usize) -> Vec<SearchResult> {
        // Score each frame, then aggregate by entry_id (max-pool).
        let mut best: std::collections::HashMap<&str, (f32, usize)> =
            std::collections::HashMap::new();

        for emb in &self.entries {
            if emb.vector.len() != query.len() {
                continue;
            }
            let score: f32 = emb.vector.iter().zip(query).map(|(a, b)| a * b).sum();
            let entry = best.entry(&emb.entry_id).or_insert((f32::NEG_INFINITY, 0));
            if score > entry.0 {
                *entry = (score, emb.sample_index);
            }
        }

        let mut results: Vec<SearchResult> = best
            .into_iter()
            .map(|(id, (score, frame))| SearchResult {
                entry_id: id.to_string(),
                score,
                best_frame_index: frame,
            })
            .collect();

        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(top_k);
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embedder::VisualEmbedding;

    fn emb(id: &str, idx: usize, v: Vec<f32>) -> VisualEmbedding {
        VisualEmbedding { entry_id: id.to_string(), sample_index: idx, vector: v }
    }

    #[test]
    fn upsert_and_search_basic() {
        let mut store = EmbeddingStore::new();
        store.upsert(vec![emb("a", 0, vec![1.0, 0.0]), emb("a", 1, vec![0.0, 1.0])]);
        store.upsert(vec![emb("b", 0, vec![0.707, 0.707])]);

        let results = store.search(&[1.0, 0.0], 5);
        assert_eq!(results[0].entry_id, "a");
    }

    #[test]
    fn upsert_replaces_existing() {
        let mut store = EmbeddingStore::new();
        store.upsert(vec![emb("a", 0, vec![0.0, 1.0])]);
        store.upsert(vec![emb("a", 0, vec![1.0, 0.0])]);
        assert_eq!(store.len(), 1);
        let r = store.search(&[1.0, 0.0], 1);
        assert!((r[0].score - 1.0).abs() < 1e-5);
    }

    #[test]
    fn remove_clears_entry() {
        let mut store = EmbeddingStore::new();
        store.upsert(vec![emb("a", 0, vec![1.0])]);
        store.remove("a");
        assert!(store.is_empty());
    }

    #[test]
    fn top_k_limits_results() {
        let mut store = EmbeddingStore::new();
        for i in 0..10u32 {
            store.upsert(vec![emb(&i.to_string(), 0, vec![1.0])]);
        }
        let r = store.search(&[1.0], 3);
        assert_eq!(r.len(), 3);
    }
}

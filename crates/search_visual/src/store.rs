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

        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(top_k);
        results
    }

    /// Serialize the store to a self-describing binary blob (magic + count +
    /// rows). Vectors are little-endian f32 for an exact round-trip. Mirrors the
    /// intent of Swift `EmbeddingStore`'s on-disk index (we use f32, not f16,
    /// since the index is per-app and f32 is lossless).
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(STORE_MAGIC);
        out.extend_from_slice(&(self.entries.len() as u32).to_le_bytes());
        for e in &self.entries {
            let id = e.entry_id.as_bytes();
            out.extend_from_slice(&(id.len() as u32).to_le_bytes());
            out.extend_from_slice(id);
            out.extend_from_slice(&(e.sample_index as u32).to_le_bytes());
            out.extend_from_slice(&(e.vector.len() as u32).to_le_bytes());
            for v in &e.vector {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
        out
    }

    /// Parse a store from bytes produced by [`EmbeddingStore::to_bytes`]. Errors
    /// on a bad magic or truncated/corrupt input; never panics or over-allocates
    /// on a hostile length field.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        let mut r = ByteReader::new(bytes);
        if r.take(STORE_MAGIC.len())? != STORE_MAGIC {
            return Err("EmbeddingStore: bad magic".to_string());
        }
        let count = r.u32()? as usize;
        let mut entries = Vec::with_capacity(count.min(1 << 20));
        for _ in 0..count {
            let id_len = r.u32()? as usize;
            let id = std::str::from_utf8(r.take(id_len)?)
                .map_err(|_| "EmbeddingStore: invalid utf-8 id".to_string())?
                .to_string();
            let sample_index = r.u32()? as usize;
            let dim = r.u32()? as usize;
            let mut vector = Vec::with_capacity(dim.min(1 << 16));
            for _ in 0..dim {
                let b = r.take(4)?;
                vector.push(f32::from_le_bytes([b[0], b[1], b[2], b[3]]));
            }
            entries.push(VisualEmbedding {
                entry_id: id,
                sample_index,
                vector,
            });
        }
        Ok(Self { entries })
    }
}

const STORE_MAGIC: &[u8; 8] = b"FRVIDX01";

/// Bounds-checked forward reader over a byte slice.
struct ByteReader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> ByteReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end = self
            .pos
            .checked_add(n)
            .ok_or_else(|| "EmbeddingStore: length overflow".to_string())?;
        if end > self.data.len() {
            return Err("EmbeddingStore: unexpected end of data".to_string());
        }
        let s = &self.data[self.pos..end];
        self.pos = end;
        Ok(s)
    }

    fn u32(&mut self) -> Result<u32, String> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embedder::VisualEmbedding;

    fn emb(id: &str, idx: usize, v: Vec<f32>) -> VisualEmbedding {
        VisualEmbedding {
            entry_id: id.to_string(),
            sample_index: idx,
            vector: v,
        }
    }

    #[test]
    fn upsert_and_search_basic() {
        let mut store = EmbeddingStore::new();
        store.upsert(vec![
            emb("a", 0, vec![1.0, 0.0]),
            emb("a", 1, vec![0.0, 1.0]),
        ]);
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

    #[test]
    fn binary_round_trip_is_exact() {
        let mut store = EmbeddingStore::new();
        store.upsert(vec![
            emb("a", 0, vec![1.0, 0.0, 0.5]),
            emb("a", 1, vec![0.0, 1.0, 0.25]),
        ]);
        store.upsert(vec![emb("b", 2, vec![0.707, 0.707, 0.0])]);

        let bytes = store.to_bytes();
        let loaded = EmbeddingStore::from_bytes(&bytes).unwrap();
        assert_eq!(loaded.len(), 3);
        // Re-serializing the loaded store reproduces the exact bytes.
        assert_eq!(loaded.to_bytes(), bytes);
        // And search behaves identically.
        let r1 = store.search(&[1.0, 0.0, 0.0], 5);
        let r2 = loaded.search(&[1.0, 0.0, 0.0], 5);
        assert_eq!(r1[0].entry_id, r2[0].entry_id);
        assert!((r1[0].score - r2[0].score).abs() < 1e-9);
    }

    #[test]
    fn empty_store_round_trips() {
        let store = EmbeddingStore::new();
        let loaded = EmbeddingStore::from_bytes(&store.to_bytes()).unwrap();
        assert!(loaded.is_empty());
    }

    #[test]
    fn from_bytes_rejects_bad_magic_and_truncation() {
        assert!(EmbeddingStore::from_bytes(b"nope").is_err());
        assert!(EmbeddingStore::from_bytes(&[]).is_err());
        let mut store = EmbeddingStore::new();
        store.upsert(vec![emb("a", 0, vec![1.0, 2.0])]);
        let bytes = store.to_bytes();
        assert!(EmbeddingStore::from_bytes(&bytes[..bytes.len() - 3]).is_err());
    }
}

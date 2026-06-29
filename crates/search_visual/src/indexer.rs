//! VisualIndexer — orchestrates frame sampling → embedding → storing.
//!
//! Mirrors Swift `VisualIndexer.swift`.
//!
//! Callers supply a concrete `Embedder` impl and a pixel-fetching closure.
//! The indexer is synchronous; async wrappers live in the platform adapter layer.

use crate::embedder::{Embedder, VisualEmbedding};
use crate::frame_sampler::{compute_sample_timestamps, FrameSamplerConfig};
use crate::store::EmbeddingStore;

/// Orchestrates frame sampling, embedding, and storage for one media entry.
pub struct VisualIndexer<'a, E: Embedder> {
    pub embedder: &'a E,
    pub config: FrameSamplerConfig,
}

impl<'a, E: Embedder> VisualIndexer<'a, E> {
    pub fn new(embedder: &'a E) -> Self {
        Self {
            embedder,
            config: FrameSamplerConfig::default(),
        }
    }

    pub fn with_config(mut self, config: FrameSamplerConfig) -> Self {
        self.config = config;
        self
    }

    /// Index one media entry.
    ///
    /// `fetch_frame` receives a timestamp in seconds and must return raw RGBA8
    /// pixel data plus (width, height). Return `Err` to skip a frame.
    pub fn index_entry(
        &self,
        entry_id: &str,
        duration_secs: f64,
        store: &mut EmbeddingStore,
        fetch_frame: &dyn Fn(f64) -> anyhow::Result<(Vec<u8>, u32, u32)>,
    ) -> anyhow::Result<usize> {
        let samples = compute_sample_timestamps(duration_secs, &self.config);
        let mut embeddings: Vec<VisualEmbedding> = Vec::with_capacity(samples.len());

        for sample in &samples {
            if let Ok((pixels, w, h)) = fetch_frame(sample.timestamp_secs) {
                match self.embedder.embed(entry_id, sample.index, &pixels, w, h) {
                    Ok(emb) => embeddings.push(emb),
                    Err(e) => {
                        // Skip frame on embed error, log at debug level only.
                        let _ = e;
                    }
                }
            }
        }

        let count = embeddings.len();
        if !embeddings.is_empty() {
            store.upsert(embeddings);
        }
        Ok(count)
    }

    /// Remove a previously indexed entry.
    pub fn remove_entry(&self, entry_id: &str, store: &mut EmbeddingStore) {
        store.remove(entry_id);
    }
}

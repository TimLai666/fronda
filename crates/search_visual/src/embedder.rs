//! Embedder trait and VisualEmbedding type.
//!
//! Mirrors Swift `VisualEmbedder.swift` + `VisualModelLoader.swift`.
//!
//! The actual embedding computation (ONNX / CoreML / tch) lives in platform
//! adapters that implement `Embedder`. This crate defines the contract only.

use serde::{Deserialize, Serialize};

/// A dense float vector representing a single frame's visual content.
/// Dimension depends on the model (e.g. 512 for CLIP ViT-B/32).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualEmbedding {
    /// Media entry ID this embedding belongs to.
    pub entry_id: String,
    /// Zero-based sample index within the clip.
    pub sample_index: usize,
    /// Embedding vector (L2-normalised).
    pub vector: Vec<f32>,
}

impl VisualEmbedding {
    /// Cosine similarity with another embedding (assumes L2-normalised vectors).
    pub fn cosine_similarity(&self, other: &VisualEmbedding) -> f32 {
        if self.vector.len() != other.vector.len() {
            return 0.0;
        }
        self.vector
            .iter()
            .zip(other.vector.iter())
            .map(|(a, b)| a * b)
            .sum()
    }
}

/// Trait for converting raw pixel data to a `VisualEmbedding`.
///
/// Implementors supply the ML runtime. The crate itself has no runtime dep.
pub trait Embedder: Send + Sync {
    /// Embedding vector dimension.
    fn dimension(&self) -> usize;

    /// Embed a single frame.
    ///
    /// `pixel_data` is RGBA8, `width` × `height` pixels.
    fn embed(
        &self,
        entry_id: &str,
        sample_index: usize,
        pixel_data: &[u8],
        width: u32,
        height: u32,
    ) -> anyhow::Result<VisualEmbedding>;

    /// Embed a text query for cross-modal search.
    fn embed_text(&self, text: &str) -> anyhow::Result<Vec<f32>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_emb(v: Vec<f32>) -> VisualEmbedding {
        VisualEmbedding {
            entry_id: "x".into(),
            sample_index: 0,
            vector: v,
        }
    }

    #[test]
    fn cosine_identical_is_one() {
        let e = make_emb(vec![1.0, 0.0]);
        assert!((e.cosine_similarity(&e) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn cosine_orthogonal_is_zero() {
        let a = make_emb(vec![1.0, 0.0]);
        let b = make_emb(vec![0.0, 1.0]);
        assert!(a.cosine_similarity(&b).abs() < 1e-6);
    }

    #[test]
    fn cosine_dimension_mismatch_returns_zero() {
        let a = make_emb(vec![1.0]);
        let b = make_emb(vec![1.0, 0.0]);
        assert_eq!(a.cosine_similarity(&b), 0.0);
    }
}

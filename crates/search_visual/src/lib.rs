//! Visual media search — Rust port of Swift `Sources/PalmierPro/Search/`.
//!
//! Architecture:
//!   FrameSampler  — extracts representative frames from a media file
//!   Embedder      — converts a frame into a float vector (trait, no ML runtime dep here)
//!   EmbeddingStore — persists and retrieves embeddings by media entry ID
//!   VisualIndexer  — orchestrates sampling → embedding → storing
//!   VisualSearch   — queries the store and returns ranked results
//!
//! No ML inference runtime is bundled in this crate. Callers supply a concrete
//! `Embedder` implementation (ONNX, CoreML, etc.) via the trait.

pub mod embedder;
pub mod frame_sampler;
pub mod indexer;
pub mod store;

pub use embedder::{Embedder, VisualEmbedding};
pub use frame_sampler::{FrameSample, FrameSamplerConfig};
pub use indexer::VisualIndexer;
pub use store::{EmbeddingStore, SearchResult};

/// Re-export error type.
pub use anyhow::Error;

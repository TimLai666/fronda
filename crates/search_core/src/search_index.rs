use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// Identity key for a search/transcript cache entry.
/// SRCH-015, TRN-001: identity depends on path + modification time + file size.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CacheIdentity {
    pub path: String,
    pub modification_time: i64,
    pub file_size: u64,
}

impl CacheIdentity {
    /// Creates identity with the current timestamp and file size 0.
    /// This is a placeholder until real file stat is available.
    pub fn from_path(path: &str) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        Self {
            path: path.to_string(),
            modification_time: now,
            file_size: 0,
        }
    }
}

/// Single embedding row at a frame offset.
/// SRCH-016: still-image indexes contain exactly one row at time zero.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EmbeddingRow {
    pub frame: i64,
    pub embedding: Vec<f32>,
}

/// Visual search index for one media asset.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VisualIndex {
    pub identity: CacheIdentity,
    pub rows: Vec<EmbeddingRow>,
}

impl VisualIndex {
    pub fn new(identity: CacheIdentity, rows: Vec<EmbeddingRow>) -> Self {
        Self { identity, rows }
    }

    /// Convenience for still images (SRCH-016): exactly one row at frame 0.
    pub fn single_frame(identity: CacheIdentity, embedding: Vec<f32>) -> Self {
        Self {
            identity,
            rows: vec![EmbeddingRow {
                frame: 0,
                embedding,
            }],
        }
    }
}

/// Search result hit.
/// SRCH-023: sorted by descending score.
#[derive(Debug, Clone)]
pub struct SearchHit {
    pub media_id: String,
    pub frame: i64,
    pub score: f64,
    pub kind: HitKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum HitKind {
    Visual,
    Spoken,
    File,
}

/// Search result groups.
/// SRCH-026: search UI keeps Moments, Spoken, and Files as separate result groups.
#[derive(Debug, Clone, Default)]
pub struct SearchResults {
    pub moments: Vec<SearchHit>,
    pub spoken: Vec<SearchHit>,
    pub files: Vec<SearchHit>,
}

impl SearchResults {
    pub fn is_empty(&self) -> bool {
        self.moments.is_empty() && self.spoken.is_empty() && self.files.is_empty()
    }

    pub fn total_hits(&self) -> usize {
        self.moments.len() + self.spoken.len() + self.files.len()
    }

    /// SRCH-027: Clear visual (moments) and spoken results immediately.
    /// File results are preserved.
    pub fn clear_query_results(&mut self) {
        self.moments.clear();
        self.spoken.clear();
    }

    /// Clear all results.
    pub fn clear_all(&mut self) {
        self.moments.clear();
        self.spoken.clear();
        self.files.clear();
    }
}

/// Configuration for visual search scoring (SRCH-024).
#[derive(Debug, Clone, Copy)]
pub struct VisualSearchConfig {
    /// Absolute minimum score threshold. Hits below this are discarded.
    pub min_score: f64,
    /// Relative cutoff ratio relative to the top score (e.g. 0.5 means
    /// keep only hits with score >= top_score * 0.5).
    pub cutoff_ratio: f64,
}

impl Default for VisualSearchConfig {
    fn default() -> Self {
        Self {
            min_score: 0.0,
            cutoff_ratio: 0.0, // 0 = no relative cutoff
        }
    }
}

/// SRCH-028: Drag payload kind for search results.
#[derive(Debug, Clone, PartialEq)]
pub enum SearchDragPayload {
    /// Plain asset drag (still-image moment hit).
    PlainAsset { media_id: String },
    /// Segmented drag with time range (video/spoken hit).
    Segmented {
        media_id: String,
        start_frame: i64,
        end_frame: i64,
    },
}

/// Determine the drag payload for a search hit (SRCH-028).
pub fn search_hit_drag_payload(hit: &SearchHit) -> SearchDragPayload {
    match hit.kind {
        HitKind::Visual => {
            // still-image moment → plain asset; video/spoken → segmented
            // For visual hits, frame is the best-frame. Treat it as a point hit.
            // In practice, Swift code checks if it's a still image or video.
            // We use frame == 0 as heuristic for still image (SRCH-016).
            if hit.frame == 0 {
                SearchDragPayload::PlainAsset {
                    media_id: hit.media_id.clone(),
                }
            } else {
                SearchDragPayload::Segmented {
                    media_id: hit.media_id.clone(),
                    start_frame: hit.frame,
                    end_frame: hit.frame + 1,
                }
            }
        }
        HitKind::Spoken => SearchDragPayload::Segmented {
            media_id: hit.media_id.clone(),
            start_frame: hit.frame,
            end_frame: hit.frame + 1,
        },
        HitKind::File => SearchDragPayload::PlainAsset {
            media_id: hit.media_id.clone(),
        },
    }
}

/// Check whether visual search is available (SRCH-021).
/// Requires the model to be ready and the trimmed query to be non-empty.
pub fn is_visual_search_available(model_ready: bool, trimmed_query: &str) -> bool {
    model_ready && !trimmed_query.is_empty()
}

/// Rank visual search hits by keeping the best frame per asset before
/// cross-asset ranking (SRCH-022), applying score cutoffs (SRCH-024),
/// and checking for non-positive top score (SRCH-025).
///
/// Input: per-frame hits from multiple assets.
/// Output: sorted, filtered hits with at most one hit per asset (best frame).
pub fn rank_visual_search(hits: Vec<SearchHit>, config: &VisualSearchConfig) -> Vec<SearchHit> {
    if hits.is_empty() {
        return Vec::new();
    }

    // SRCH-022: Keep the best frame per asset (highest score).
    let mut best_per_asset: std::collections::HashMap<String, SearchHit> =
        std::collections::HashMap::new();
    for hit in hits {
        let entry = best_per_asset
            .entry(hit.media_id.clone())
            .or_insert_with(|| hit.clone());
        if hit.score > entry.score {
            *entry = hit;
        }
    }

    let mut ranked: Vec<SearchHit> = best_per_asset.into_values().collect();

    // SRCH-023: Sort by descending score.
    ranked.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());

    // SRCH-025: If the top score is non-positive, return no hits.
    if ranked.is_empty() || ranked[0].score <= 0.0 {
        return Vec::new();
    }

    let top_score = ranked[0].score;

    // SRCH-024: Apply absolute minimum score and relative cutoff.
    ranked.retain(|hit| {
        if hit.score < config.min_score {
            return false;
        }
        if config.cutoff_ratio > 0.0 && hit.score < top_score * config.cutoff_ratio {
            return false;
        }
        true
    });

    ranked
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn srch_015_cache_identity_equality() {
        let a = CacheIdentity {
            path: "/movies/clip.mov".into(),
            modification_time: 1_700_000_000,
            file_size: 42_000,
        };
        let b = CacheIdentity {
            path: "/movies/clip.mov".into(),
            modification_time: 1_700_000_000,
            file_size: 42_000,
        };
        assert_eq!(a, b);

        let c = CacheIdentity {
            path: "/movies/other.mov".into(),
            modification_time: 1_700_000_000,
            file_size: 42_000,
        };
        assert_ne!(a, c);
    }

    #[test]
    fn srch_016_single_frame_index() {
        let identity = CacheIdentity::from_path("/images/photo.jpg");
        let index = VisualIndex::single_frame(identity, vec![0.1, 0.2, 0.3]);
        assert_eq!(index.rows.len(), 1);
        assert_eq!(index.rows[0].frame, 0);
        assert_eq!(index.rows[0].embedding, vec![0.1, 0.2, 0.3]);
    }

    #[test]
    fn srch_023_search_hit_sorting() {
        let mut hits = vec![
            SearchHit {
                media_id: "a".into(),
                frame: 0,
                score: 0.5,
                kind: HitKind::Visual,
            },
            SearchHit {
                media_id: "b".into(),
                frame: 0,
                score: 0.9,
                kind: HitKind::Visual,
            },
            SearchHit {
                media_id: "c".into(),
                frame: 0,
                score: 0.1,
                kind: HitKind::Visual,
            },
        ];
        hits.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        assert_eq!(hits[0].media_id, "b");
        assert_eq!(hits[1].media_id, "a");
        assert_eq!(hits[2].media_id, "c");
    }

    #[test]
    fn srch_026_separate_result_groups() {
        let results = SearchResults {
            moments: vec![SearchHit {
                media_id: "m1".into(),
                frame: 10,
                score: 0.8,
                kind: HitKind::Visual,
            }],
            spoken: vec![SearchHit {
                media_id: "s1".into(),
                frame: 20,
                score: 0.7,
                kind: HitKind::Spoken,
            }],
            files: vec![SearchHit {
                media_id: "f1".into(),
                frame: 0,
                score: 0.6,
                kind: HitKind::File,
            }],
        };
        assert_eq!(results.moments.len(), 1);
        assert_eq!(results.spoken.len(), 1);
        assert_eq!(results.files.len(), 1);
        assert_eq!(results.total_hits(), 3);
    }

    #[test]
    fn search_results_empty() {
        let results = SearchResults::default();
        assert!(results.is_empty());
        assert_eq!(results.total_hits(), 0);
    }

    #[test]
    fn embedding_row_serde() {
        let row = EmbeddingRow {
            frame: 42,
            embedding: vec![0.1, 0.2, 0.3, 0.4],
        };
        let json = serde_json::to_string(&row).unwrap();
        let deserialized: EmbeddingRow = serde_json::from_str(&json).unwrap();
        assert_eq!(row, deserialized);
    }
}

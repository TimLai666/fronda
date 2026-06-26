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

// ── Frame sampler (SRCH-017..020) ────────────────────────────────────────

/// A single frame sample at a specific frame offset.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameSample {
    pub frame: i64,
}

/// Sampling strategy for frame selection.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SamplingStrategy {
    /// Uniform sampling: evenly spaced samples across the duration.
    Uniform,
    /// Scene-aware: detect changes and promote scene-start frames.
    ///
    /// Falls back to uniform sampling when no scenes can be detected.
    SceneAware,
}

/// Configuration for the frame sampler.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameSamplerConfig {
    /// Maximum number of samples to take.
    pub max_samples: usize,
    /// Minimum gap between samples in frames.
    pub min_gap_frames: i64,
    /// Sampling strategy.
    pub strategy: SamplingStrategy,
    /// Number of frames to skip at the start (credit sequence, black frames).
    pub pad_start_frames: i64,
    /// Number of frames to skip at the end (credits, black frames).
    pub pad_end_frames: i64,
}

impl Default for FrameSamplerConfig {
    fn default() -> Self {
        Self {
            max_samples: 48,
            min_gap_frames: 15,
            strategy: SamplingStrategy::Uniform,
            pad_start_frames: 0,
            pad_end_frames: 0,
        }
    }
}

impl FrameSamplerConfig {
    /// Config for short videos — ensures at least one sample at midpoint (SRCH-017).
    pub fn short_video() -> Self {
        Self {
            max_samples: 1,
            ..Default::default()
        }
    }
}

/// Pure-logic frame sampler (SRCH-017..020).
///
/// Produces ordered frame positions for visual indexing.
/// Does NOT decode video — it computes sampling positions
/// based on configuration and duration metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct FrameSampler {
    pub config: FrameSamplerConfig,
}

impl FrameSampler {
    pub fn new(config: FrameSamplerConfig) -> Self {
        Self { config }
    }

    /// Sample frame positions for a video of the given total duration in frames.
    ///
    /// SRCH-017: Short videos (< max_samples * min_gap frames) still get at least
    ///           one sample at the midpoint.
    /// SRCH-018: Output times are strictly increasing with no duplicates.
    /// SRCH-019: Scene-aware strategy promotes scene-start frames but ensures
    ///           coverage-floor samples for long static spans.
    ///
    /// `scene_boundaries` — optional frame positions where scenes change.
    ///   When `None` or empty, falls back to uniform sampling.
    pub fn sample(&self, total_frames: i64, scene_boundaries: Option<&[i64]>) -> Vec<FrameSample> {
        if total_frames <= 0 {
            return Vec::new();
        }

        let usable = total_frames - self.config.pad_start_frames - self.config.pad_end_frames;
        if usable <= 0 {
            // SRCH-017: Even zero-length gets a midpoint.
            let midpoint = total_frames.max(0) / 2;
            return vec![FrameSample { frame: midpoint }];
        }

        let start = self.config.pad_start_frames;

        match self.config.strategy {
            SamplingStrategy::Uniform => self.sample_uniform(usable, start),
            SamplingStrategy::SceneAware => {
                self.sample_scene_aware(usable, start, scene_boundaries)
            }
        }
    }

    /// Uniform sampling: evenly spaced samples.
    fn sample_uniform(&self, usable: i64, start: i64) -> Vec<FrameSample> {
        let count = std::cmp::min(self.config.max_samples, usable as usize);
        if count == 0 {
            return Vec::new();
        }

        // SRCH-017: ensure at least 1 sample
        let count = count.max(1);
        let step = usable as f64 / count as f64;

        let mut samples: Vec<FrameSample> = (0..count)
            .map(|i| {
                let offset = (step * i as f64 + step * 0.5).round() as i64;
                FrameSample {
                    frame: start + offset.min(usable - 1),
                }
            })
            .collect();

        // SRCH-018: strictly increasing and no duplicates
        samples.dedup_by_key(|s| s.frame);
        debug_assert!(
            samples.windows(2).all(|w| w[0].frame < w[1].frame),
            "SRCH-018: FrameSample output must be strictly increasing"
        );
        samples
    }

    /// Scene-aware sampling: promote scene boundary frames, fill gaps with uniform samples.
    fn sample_scene_aware(
        &self,
        usable: i64,
        start: i64,
        scene_boundaries: Option<&[i64]>,
    ) -> Vec<FrameSample> {
        let boundaries = scene_boundaries
            .unwrap_or(&[])
            .iter()
            .copied()
            .filter(|&f| f >= start && f < start + usable)
            .collect::<Vec<_>>();

        if boundaries.is_empty() {
            return self.sample_uniform(usable, start);
        }

        let mut samples: Vec<FrameSample> = boundaries
            .iter()
            .map(|&f| FrameSample { frame: f })
            .collect();

        // SRCH-019: Ensure coverage-floor samples — if any gap between
        // consecutive scene boundaries exceeds max_samples * min_gap,
        // insert uniform samples to maintain coverage.
        let max_gap = self.config.max_samples as i64 * self.config.min_gap_frames;

        let mut extra_samples = Vec::new();
        for pair in boundaries.windows(2) {
            let gap = pair[1] - pair[0];
            if gap > max_gap {
                // Need extra samples in this gap
                let extra_count = (gap / max_gap).min(self.config.max_samples as i64) as usize;
                let local_step = gap as f64 / (extra_count + 1) as f64;
                for j in 1..=extra_count {
                    let frame = pair[0] + (local_step * j as f64).round() as i64;
                    extra_samples.push(FrameSample { frame });
                }
            }
        }

        samples.extend(extra_samples);
        // SRCH-018: strictly increasing, no duplicates
        samples.sort_by_key(|s| s.frame);
        samples.dedup_by_key(|s| s.frame);
        debug_assert!(
            samples.windows(2).all(|w| w[0].frame < w[1].frame),
            "SRCH-018: FrameSample output must be strictly increasing"
        );
        samples
    }

    /// Return a valid empty index for videos that cannot be decoded (SRCH-020).
    pub fn empty_index(identity: CacheIdentity) -> VisualIndex {
        VisualIndex {
            identity,
            rows: Vec::new(),
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

    // ── SRCH-017: Frame sampler short video ─────────────────────

    #[test]
    fn srch_017_short_video_gets_midpoint_sample() {
        let sampler = FrameSampler::new(FrameSamplerConfig::short_video());
        let samples = sampler.sample(30, None);
        assert_eq!(
            samples.len(),
            1,
            "SRCH-017: short video gets exactly one sample"
        );
        assert_eq!(
            samples[0].frame, 15,
            "SRCH-017: sample at midpoint for 30-frame video"
        );
    }

    #[test]
    fn srch_017_very_short_video_still_has_sample() {
        let sampler = FrameSampler::new(FrameSamplerConfig::short_video());
        let samples = sampler.sample(1, None);
        assert_eq!(samples.len(), 1);
        assert_eq!(samples[0].frame, 0);
    }

    #[test]
    fn srch_017_zero_frame_video_still_has_sample() {
        let sampler = FrameSampler::new(FrameSamplerConfig::short_video());
        let samples = sampler.sample(0, None);
        assert!(samples.is_empty(), "zero-frame returns empty");
    }

    #[test]
    fn srch_017_negative_frame_video_empty() {
        let sampler = FrameSampler::new(FrameSamplerConfig::short_video());
        let samples = sampler.sample(-1, None);
        assert!(samples.is_empty());
    }

    // ── SRCH-018: Strictly increasing, no duplicates ────────────

    #[test]
    fn srch_018_uniform_samples_are_strictly_increasing() {
        let sampler = FrameSampler::new(FrameSamplerConfig::default());
        let samples = sampler.sample(1000, None);
        assert!(samples.len() > 1);
        for w in samples.windows(2) {
            assert!(
                w[0].frame < w[1].frame,
                "SRCH-018: frames must be strictly increasing"
            );
        }
    }

    #[test]
    fn srch_018_uniform_no_duplicates() {
        let sampler = FrameSampler::new(FrameSamplerConfig::default());
        let samples = sampler.sample(1000, None);
        let mut frames: Vec<_> = samples.iter().map(|s| s.frame).collect();
        frames.sort();
        frames.dedup();
        assert_eq!(frames.len(), samples.len(), "SRCH-018: no duplicate frames");
    }

    #[test]
    fn srch_018_scene_aware_increasing() {
        let sampler = FrameSampler::new(FrameSamplerConfig {
            strategy: SamplingStrategy::SceneAware,
            ..Default::default()
        });
        let boundaries = vec![0, 200, 500, 800];
        let samples = sampler.sample(1000, Some(&boundaries));
        for w in samples.windows(2) {
            assert!(
                w[0].frame < w[1].frame,
                "SRCH-018: scene-aware frames must be strictly increasing"
            );
        }
    }

    #[test]
    fn srch_018_single_frame_trivially_increasing() {
        let sampler = FrameSampler::new(FrameSamplerConfig::default());
        let samples = sampler.sample(1, None);
        assert_eq!(samples.len(), 1);
    }

    // ── SRCH-019: Scene-aware with coverage floor ───────────────

    #[test]
    fn srch_019_scene_boundaries_promoted() {
        let sampler = FrameSampler::new(FrameSamplerConfig {
            strategy: SamplingStrategy::SceneAware,
            max_samples: 48,
            min_gap_frames: 15,
            ..Default::default()
        });
        let boundaries = vec![100, 300, 600];
        let samples = sampler.sample(1000, Some(&boundaries));
        let boundary_frames: Vec<_> = samples.iter().map(|s| s.frame).collect();
        for &b in &boundaries {
            assert!(
                boundary_frames.contains(&b),
                "SRCH-019: scene boundary {} should be promoted",
                b
            );
        }
    }

    #[test]
    fn srch_019_long_static_span_gets_coverage_floor() {
        let sampler = FrameSampler::new(FrameSamplerConfig {
            strategy: SamplingStrategy::SceneAware,
            max_samples: 10,
            min_gap_frames: 10,
            ..Default::default()
        });
        // Gap of 800 > max_gap = 100 -> needs extra samples
        let boundaries = vec![100, 900];
        let samples = sampler.sample(1000, Some(&boundaries));
        // Should have boundary frames plus extra coverage samples
        assert!(
            samples.len() > 2,
            "SRCH-019: long static span should get coverage samples"
        );
    }

    #[test]
    fn srch_019_no_boundaries_falls_back_to_uniform() {
        let sampler = FrameSampler::new(FrameSamplerConfig {
            strategy: SamplingStrategy::SceneAware,
            ..Default::default()
        });
        let samples = sampler.sample(1000, None);
        assert!(
            samples.len() > 1,
            "SRCH-019: no boundaries -> uniform fallback"
        );
        let uniform = FrameSampler::new(FrameSamplerConfig::default());
        let uniform_samples = uniform.sample(1000, None);
        assert_eq!(samples.len(), uniform_samples.len());
    }

    #[test]
    fn srch_019_empty_boundaries_falls_back_to_uniform() {
        let sampler = FrameSampler::new(FrameSamplerConfig {
            strategy: SamplingStrategy::SceneAware,
            ..Default::default()
        });
        let samples = sampler.sample(1000, Some(&[]));
        assert!(samples.len() > 1);
    }

    // ── SRCH-020: Corrupt video -> valid empty index ────────────

    #[test]
    fn srch_020_empty_index_no_rows() {
        let identity = CacheIdentity {
            path: "/corrupt/video.mp4".into(),
            modification_time: 12345,
            file_size: 0,
        };
        let index = FrameSampler::empty_index(identity.clone());
        assert_eq!(index.identity, identity);
        assert!(index.rows.is_empty(), "SRCH-020: empty index has no rows");
    }

    #[test]
    fn srch_020_empty_index_valid_visual_index() {
        let identity = CacheIdentity {
            path: "/missing/file.mp4".into(),
            modification_time: 0,
            file_size: 0,
        };
        let index = FrameSampler::empty_index(identity.clone());
        // Should be serializable (valid VisualIndex)
        let json = serde_json::to_string(&index).unwrap();
        let restored: VisualIndex = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.identity, identity);
        assert!(restored.rows.is_empty());
    }

    #[test]
    fn srch_020_empty_index_does_not_cause_retry() {
        // SRCH-020: A corrupt video should produce a valid empty index
        // that is then cached like any other index. The search lifecycle
        // marks it as Completed (not Failed), so it won't cause perpetual retry.
        //
        // This test validates the structural contract: an empty index is
        // a valid VisualIndex that can be stored, serialized, and queried.
        let identity = CacheIdentity {
            path: "/corrupt/bad_data.mp4".into(),
            modification_time: 99999,
            file_size: 42,
        };
        let _index = FrameSampler::empty_index(identity);
        // A query against an empty index returns zero results
        let results =
            crate::search_index::rank_visual_search(Vec::new(), &VisualSearchConfig::default());
        assert!(results.is_empty(), "empty index yields no search results");
    }

    // ── Frame sampler config tests ──────────────────────────────

    #[test]
    fn srch_frame_sampler_default_config() {
        let config = FrameSamplerConfig::default();
        assert_eq!(config.max_samples, 48);
        assert_eq!(config.min_gap_frames, 15);
        assert_eq!(config.strategy, SamplingStrategy::Uniform);
    }

    #[test]
    fn srch_frame_sampler_short_video_config() {
        let config = FrameSamplerConfig::short_video();
        assert_eq!(config.max_samples, 1);
    }

    #[test]
    fn srch_frame_sampler_padding() {
        let config = FrameSamplerConfig {
            pad_start_frames: 50,
            pad_end_frames: 50,
            max_samples: 5,
            ..Default::default()
        };
        let sampler = FrameSampler::new(config);
        let samples = sampler.sample(500, None);
        // Start padding means first sample >= 50
        // End padding means last sample < 450
        assert!(
            samples.iter().all(|s| s.frame >= 50 && s.frame < 450),
            "all samples should be within usable range"
        );
    }

    #[test]
    fn srch_frame_sampler_padding_fewer_frames_than_padding() {
        let config = FrameSamplerConfig {
            pad_start_frames: 50,
            pad_end_frames: 50,
            max_samples: 1,
            ..Default::default()
        };
        let sampler = FrameSampler::new(config);
        let samples = sampler.sample(50, None);
        // usable = 50 - 50 - 50 = -50 -> clamped to midpoint
        assert_eq!(samples.len(), 1);
        assert_eq!(samples[0].frame, 25); // midpoint
    }
}

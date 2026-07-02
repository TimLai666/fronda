//! FrameSampler — selects representative frames from a media item.
//!
//! Mirrors Swift `FrameSampler.swift`: divides the clip into N equal
//! intervals and records the timestamp of the frame at each interval midpoint.

use serde::{Deserialize, Serialize};

/// Configuration for the frame sampler.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameSamplerConfig {
    /// Number of frames to sample per media item.
    pub frames_per_clip: usize,
    /// Maximum clip duration in seconds to sample (longer clips are capped).
    pub max_sample_duration_secs: f64,
}

impl Default for FrameSamplerConfig {
    fn default() -> Self {
        Self {
            frames_per_clip: 8,
            max_sample_duration_secs: 300.0,
        }
    }
}

/// A single sampled frame: its timestamp (in seconds) within the source clip.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameSample {
    /// Zero-based index of this sample within the sampling run.
    pub index: usize,
    /// Timestamp in seconds from the start of the clip.
    pub timestamp_secs: f64,
}

/// Computes sample timestamps for a clip of `duration_secs` length.
///
/// Returns `config.frames_per_clip` evenly spaced timestamps, capped at
/// `config.max_sample_duration_secs`.
pub fn compute_sample_timestamps(
    duration_secs: f64,
    config: &FrameSamplerConfig,
) -> Vec<FrameSample> {
    let effective = duration_secs.min(config.max_sample_duration_secs);
    let n = config.frames_per_clip;
    if n == 0 || effective <= 0.0 {
        return Vec::new();
    }
    let interval = effective / n as f64;
    (0..n)
        .map(|i| FrameSample {
            index: i,
            timestamp_secs: interval * (i as f64 + 0.5),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_count_matches_config() {
        let cfg = FrameSamplerConfig {
            frames_per_clip: 4,
            max_sample_duration_secs: 60.0,
        };
        let samples = compute_sample_timestamps(30.0, &cfg);
        assert_eq!(samples.len(), 4);
    }

    #[test]
    fn timestamps_are_ordered() {
        let cfg = FrameSamplerConfig::default();
        let samples = compute_sample_timestamps(120.0, &cfg);
        for w in samples.windows(2) {
            assert!(w[0].timestamp_secs < w[1].timestamp_secs);
        }
    }

    #[test]
    fn caps_at_max_duration() {
        let cfg = FrameSamplerConfig {
            frames_per_clip: 2,
            max_sample_duration_secs: 10.0,
        };
        let samples = compute_sample_timestamps(9999.0, &cfg);
        assert!(samples.last().unwrap().timestamp_secs < 10.0);
    }
}

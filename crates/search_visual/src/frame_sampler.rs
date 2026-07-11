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

/// An 8x8 grid of per-cell mean luma, each value in `0.0..=1.0`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LumaGrid {
    /// 64 row-major cell means.
    pub cells: Vec<f32>,
}

/// Reduces a single-channel luma frame to an 8x8 grid of per-cell mean luma.
///
/// `luma` is a row-major buffer of length `width * height`. Cell bounds use
/// integer division; the last row/column of cells absorb any remainder when
/// dimensions are not divisible by 8. Returns 64 zeros for degenerate input.
pub fn luma_grid(luma: &[u8], width: usize, height: usize) -> LumaGrid {
    if width == 0 || height == 0 || luma.len() < width * height {
        return LumaGrid {
            cells: vec![0.0; 64],
        };
    }
    let mut cells = Vec::with_capacity(64);
    for gy in 0..8 {
        let y0 = gy * height / 8;
        let y1 = if gy == 7 {
            height
        } else {
            (gy + 1) * height / 8
        };
        for gx in 0..8 {
            let x0 = gx * width / 8;
            let x1 = if gx == 7 { width } else { (gx + 1) * width / 8 };
            let mut sum: u64 = 0;
            for y in y0..y1 {
                let row = y * width;
                for x in x0..x1 {
                    sum += luma[row + x] as u64;
                }
            }
            let count = ((x1 - x0) * (y1 - y0)) as u64;
            let mean = sum as f32 / count as f32 / 255.0;
            cells.push(mean);
        }
    }
    LumaGrid { cells }
}

/// True when the mean absolute per-cell difference between two grids exceeds
/// `threshold`. Mismatched lengths are treated as a scene change.
pub fn scene_changed(a: &LumaGrid, b: &LumaGrid, threshold: f32) -> bool {
    if a.cells.len() != b.cells.len() {
        return true;
    }
    if a.cells.is_empty() {
        return false;
    }
    let sum: f32 = a
        .cells
        .iter()
        .zip(&b.cells)
        .map(|(x, y)| (x - y).abs())
        .sum();
    let mad = sum / a.cells.len() as f32;
    mad > threshold
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

    #[test]
    fn identical_uniform_frames_no_scene_change() {
        let frame = vec![100u8; 16 * 16];
        let a = luma_grid(&frame, 16, 16);
        let b = luma_grid(&frame, 16, 16);
        assert!(!scene_changed(&a, &b, 0.01));
    }

    #[test]
    fn black_vs_white_is_scene_change() {
        let black = luma_grid(&vec![0u8; 16 * 16], 16, 16);
        let white = luma_grid(&vec![255u8; 16 * 16], 16, 16);
        assert!(scene_changed(&black, &white, 0.5));
    }

    #[test]
    fn luma_grid_uniform_128_is_half() {
        let grid = luma_grid(&[128u8; 8 * 8], 8, 8);
        assert_eq!(grid.cells.len(), 64);
        for c in &grid.cells {
            assert!((c - 0.502).abs() < 0.001, "cell was {c}");
        }
    }

    #[test]
    fn degenerate_input_returns_zeros() {
        let zeros = luma_grid(&[], 0, 0);
        assert_eq!(zeros.cells.len(), 64);
        assert!(zeros.cells.iter().all(|c| *c == 0.0));
        let short = luma_grid(&[1, 2, 3], 4, 4);
        assert!(short.cells.iter().all(|c| *c == 0.0));
    }

    #[test]
    fn non_divisible_dims_cover_all_pixels() {
        let grid = luma_grid(&[200u8; 10 * 10], 10, 10);
        assert_eq!(grid.cells.len(), 64);
        for c in &grid.cells {
            assert!((c - 200.0 / 255.0).abs() < 1e-6);
        }
    }

    #[test]
    fn mismatched_grid_lengths_is_scene_change() {
        let a = LumaGrid {
            cells: vec![0.0; 64],
        };
        let b = LumaGrid {
            cells: vec![0.0; 32],
        };
        assert!(scene_changed(&a, &b, 0.9));
    }
}

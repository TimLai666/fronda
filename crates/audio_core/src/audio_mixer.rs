//! Pure timeline audio mixer.
//!
//! Sums placed audio sources into one interleaved PCM buffer, applying per-clip
//! linear gain and linear fade in/out. Decoding is the caller's job (a platform
//! adapter fills each [`AudioPlacement`] with PCM at the mix rate/channels), so
//! this module is dependency-free and fully unit-tested. It is the audio
//! counterpart to the video frame compositor.

/// Per-channel sample count for one project frame at `sample_rate`/`fps`.
pub fn samples_per_frame(sample_rate: u32, fps: i64) -> usize {
    if fps <= 0 {
        return 0;
    }
    (sample_rate as f64 / fps as f64).round() as usize
}

/// One audio source placed on the timeline. `samples` is interleaved PCM already
/// resampled to the mix rate and channel count; `start_sample` is the per-channel
/// output offset where it begins.
#[derive(Debug, Clone)]
pub struct AudioPlacement {
    pub start_sample: usize,
    pub samples: Vec<f32>,
    pub volume: f32,
    pub fade_in_samples: usize,
    pub fade_out_samples: usize,
}

impl AudioPlacement {
    /// Per-channel frame count of this placement's PCM.
    fn frames(&self, channels: usize) -> usize {
        if channels == 0 {
            0
        } else {
            self.samples.len() / channels
        }
    }
}

/// Linear fade envelope at per-channel frame `i` of a clip `frames` long.
fn fade_gain(i: usize, frames: usize, fade_in: usize, fade_out: usize) -> f32 {
    let mut g = 1.0f32;
    if fade_in > 0 && i < fade_in {
        g *= (i as f32 + 0.5) / fade_in as f32;
    }
    if fade_out > 0 && frames >= fade_out {
        let out_start = frames - fade_out;
        if i >= out_start {
            let into = i - out_start;
            g *= 1.0 - (into as f32 + 0.5) / fade_out as f32;
        }
    }
    g.clamp(0.0, 1.0)
}

/// Mix `placements` into one interleaved buffer of `channels`. Its per-channel
/// length is the furthest placement end, or `min_frames` if larger (trailing
/// silence). Overlapping placements sum; the result is clamped to `[-1, 1]`.
pub fn mix(placements: &[AudioPlacement], channels: usize, min_frames: usize) -> Vec<f32> {
    if channels == 0 {
        return Vec::new();
    }
    let total_frames = placements
        .iter()
        .map(|p| p.start_sample + p.frames(channels))
        .max()
        .unwrap_or(0)
        .max(min_frames);

    let mut out = vec![0.0f32; total_frames * channels];
    for p in placements {
        let frames = p.frames(channels);
        for i in 0..frames {
            let gain = p.volume * fade_gain(i, frames, p.fade_in_samples, p.fade_out_samples);
            let out_frame = p.start_sample + i;
            let out_base = out_frame * channels;
            let in_base = i * channels;
            for c in 0..channels {
                out[out_base + c] += p.samples[in_base + c] * gain;
            }
        }
    }
    for s in &mut out {
        *s = s.clamp(-1.0, 1.0);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn placement(start: usize, samples: Vec<f32>, volume: f32) -> AudioPlacement {
        AudioPlacement {
            start_sample: start,
            samples,
            volume,
            fade_in_samples: 0,
            fade_out_samples: 0,
        }
    }

    #[test]
    fn samples_per_frame_rounds() {
        assert_eq!(samples_per_frame(48_000, 30), 1600);
        assert_eq!(samples_per_frame(44_100, 24), 1838); // 1837.5 → 1838
        assert_eq!(samples_per_frame(48_000, 0), 0);
    }

    #[test]
    fn single_placement_copies_at_offset() {
        // Mono, one clip of 3 samples starting at frame 2.
        let p = placement(2, vec![0.5, 0.6, 0.7], 1.0);
        let out = mix(&[p], 1, 0);
        assert_eq!(out, vec![0.0, 0.0, 0.5, 0.6, 0.7]);
    }

    #[test]
    fn volume_scales_linearly() {
        let p = placement(0, vec![0.8, 0.4], 0.5);
        let out = mix(&[p], 1, 0);
        assert_eq!(out, vec![0.4, 0.2]);
    }

    #[test]
    fn overlapping_placements_sum() {
        let a = placement(0, vec![0.3, 0.3, 0.3], 1.0);
        let b = placement(1, vec![0.4, 0.4], 1.0);
        let out = mix(&[a, b], 1, 0);
        // frame1 and 2 overlap: 0.3+0.4 = 0.7 (float-approx).
        let expected = [0.3f32, 0.7, 0.7];
        assert_eq!(out.len(), 3);
        for (got, want) in out.iter().zip(expected) {
            assert!((got - want).abs() < 1e-6, "got {got}, want {want}");
        }
    }

    #[test]
    fn sum_clamps_to_unity() {
        let a = placement(0, vec![0.8], 1.0);
        let b = placement(0, vec![0.8], 1.0);
        let out = mix(&[a, b], 1, 0);
        assert_eq!(out, vec![1.0]); // 1.6 clamped
    }

    #[test]
    fn stereo_interleaving_preserved() {
        // 2ch, one frame [L=0.2, R=-0.2] starting at frame 1.
        let p = placement(1, vec![0.2, -0.2], 1.0);
        let out = mix(&[p], 2, 0);
        assert_eq!(out, vec![0.0, 0.0, 0.2, -0.2]);
    }

    #[test]
    fn fade_in_ramps_from_near_zero() {
        let p = AudioPlacement {
            start_sample: 0,
            samples: vec![1.0, 1.0, 1.0, 1.0],
            volume: 1.0,
            fade_in_samples: 4,
            fade_out_samples: 0,
        };
        let out = mix(&[p], 1, 0);
        assert!(out[0] < out[1] && out[1] < out[2] && out[2] < out[3]);
        assert!(out[0] < 0.2, "first sample near zero, got {}", out[0]);
        assert!(out[3] < 1.0, "still ramping at last fade sample");
    }

    #[test]
    fn fade_out_ramps_to_near_zero() {
        let p = AudioPlacement {
            start_sample: 0,
            samples: vec![1.0, 1.0, 1.0, 1.0],
            volume: 1.0,
            fade_in_samples: 0,
            fade_out_samples: 4,
        };
        let out = mix(&[p], 1, 0);
        assert!(out[0] > out[1] && out[1] > out[2] && out[2] > out[3]);
        assert!(out[3] < 0.2, "last sample near zero, got {}", out[3]);
    }

    #[test]
    fn min_frames_pads_with_silence() {
        let p = placement(0, vec![1.0], 1.0);
        let out = mix(&[p], 1, 4);
        assert_eq!(out, vec![1.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn empty_input_is_empty() {
        assert!(mix(&[], 2, 0).is_empty());
        assert!(mix(&[placement(0, vec![1.0], 1.0)], 0, 0).is_empty());
    }
}

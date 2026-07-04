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
#[derive(Debug, Clone, Default)]
pub struct AudioPlacement {
    pub start_sample: usize,
    pub samples: Vec<f32>,
    pub volume: f32,
    pub fade_in_samples: usize,
    pub fade_out_samples: usize,
    /// `smoothstep` easing on the head/tail ramp instead of linear.
    pub fade_in_smooth: bool,
    pub fade_out_smooth: bool,
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

fn smoothstep(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// Fade envelope at per-channel frame `i` of a clip `frames` long. Mirrors Swift
/// `Timeline.fadeMultiplier(at:)` in sample space: each ramp is `t` (or
/// `smoothstep(t)`), the two combine by `min` so overlapping head/tail fades take
/// the deeper ramp rather than double-attenuating, and there is no half-sample
/// offset (the first sample is exactly 0, the last is `1/fade`).
fn fade_gain(p: &AudioPlacement, i: usize, frames: usize) -> f32 {
    let in_mul = if p.fade_in_samples > 0 {
        let t = (i as f32 / p.fade_in_samples as f32).min(1.0);
        if p.fade_in_smooth {
            smoothstep(t)
        } else {
            t
        }
    } else {
        1.0
    };
    let out_mul = if p.fade_out_samples > 0 {
        let rem = frames.saturating_sub(i);
        let t = (rem as f32 / p.fade_out_samples as f32).min(1.0);
        if p.fade_out_smooth {
            smoothstep(t)
        } else {
            t
        }
    } else {
        1.0
    };
    in_mul.min(out_mul)
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
            let gain = p.volume * fade_gain(p, i, frames);
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

/// Resample interleaved PCM to exactly `out_frames` per-channel frames
/// (time-stretch / speed change). Upsampling uses linear interpolation;
/// downsampling box-averages the input window per output frame to avoid aliasing
/// on sped-up audio. Empty input yields silence; a single input frame is held.
pub fn resample_linear(samples: &[f32], channels: usize, out_frames: usize) -> Vec<f32> {
    if channels == 0 || out_frames == 0 {
        return Vec::new();
    }
    let in_frames = samples.len() / channels;
    let mut out = vec![0.0f32; out_frames * channels];
    if in_frames == 0 {
        return out;
    }
    if in_frames == 1 {
        for f in 0..out_frames {
            for c in 0..channels {
                out[f * channels + c] = samples[c];
            }
        }
        return out;
    }
    if out_frames < in_frames {
        // Downsample: average each output frame's input window (anti-alias).
        for f in 0..out_frames {
            let start = f * in_frames / out_frames;
            let end = ((f + 1) * in_frames / out_frames).clamp(start + 1, in_frames);
            let n = (end - start) as f32;
            for c in 0..channels {
                let mut sum = 0.0f32;
                for i in start..end {
                    sum += samples[i * channels + c];
                }
                out[f * channels + c] = sum / n;
            }
        }
        return out;
    }
    for f in 0..out_frames {
        let pos = f as f64 * in_frames as f64 / out_frames as f64;
        let i0 = (pos.floor() as usize).min(in_frames - 1);
        let i1 = (i0 + 1).min(in_frames - 1);
        let t = (pos - i0 as f64) as f32;
        for c in 0..channels {
            let a = samples[i0 * channels + c];
            let b = samples[i1 * channels + c];
            out[f * channels + c] = a + (b - a) * t;
        }
    }
    out
}

/// Downsample interleaved PCM to `bucket_count` peak amplitudes for waveform
/// display. Each bucket is the maximum absolute sample across all channels in
/// its slice of frames. Returns at most `bucket_count` values (fewer when there
/// are fewer frames than buckets); empty for empty input or zero buckets.
pub fn compute_peaks(samples: &[f32], channels: usize, bucket_count: usize) -> Vec<f32> {
    if samples.is_empty() || channels == 0 || bucket_count == 0 {
        return Vec::new();
    }
    let frames = samples.len() / channels;
    if frames == 0 {
        return Vec::new();
    }
    let buckets = bucket_count.min(frames);
    let mut peaks = Vec::with_capacity(buckets);
    for b in 0..buckets {
        let start = b * frames / buckets;
        let end = ((b + 1) * frames / buckets).max(start + 1);
        let mut peak = 0.0f32;
        for f in start..end {
            for c in 0..channels {
                peak = peak.max(samples[f * channels + c].abs());
            }
        }
        peaks.push(peak);
    }
    peaks
}

#[cfg(test)]
mod tests {
    use super::*;

    fn placement(start: usize, samples: Vec<f32>, volume: f32) -> AudioPlacement {
        AudioPlacement {
            start_sample: start,
            samples,
            volume,
            ..Default::default()
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
    fn fade_in_ramps_linearly_from_zero() {
        // Swift parity: linear ramp `i/fade`, no half-sample offset → 0, .25, .5, .75.
        let p = AudioPlacement {
            start_sample: 0,
            samples: vec![1.0, 1.0, 1.0, 1.0],
            volume: 1.0,
            fade_in_samples: 4,
            ..Default::default()
        };
        let out = mix(&[p], 1, 0);
        let want = [0.0f32, 0.25, 0.5, 0.75];
        for (got, w) in out.iter().zip(want) {
            assert!((got - w).abs() < 1e-6, "got {out:?}");
        }
    }

    #[test]
    fn fade_out_ramps_linearly_to_last_step() {
        // Swift parity: `rem/fade` → 1, .75, .5, .25 (last sample is 1/fade, not 0).
        let p = AudioPlacement {
            start_sample: 0,
            samples: vec![1.0, 1.0, 1.0, 1.0],
            volume: 1.0,
            fade_out_samples: 4,
            ..Default::default()
        };
        let out = mix(&[p], 1, 0);
        let want = [1.0f32, 0.75, 0.5, 0.25];
        for (got, w) in out.iter().zip(want) {
            assert!((got - w).abs() < 1e-6, "got {out:?}");
        }
    }

    #[test]
    fn fade_smooth_curves_below_then_above_linear() {
        // `.smooth` in-fade: smoothstep(i/fade) — below linear early, above late.
        let p = AudioPlacement {
            start_sample: 0,
            samples: vec![1.0; 10],
            volume: 1.0,
            fade_in_samples: 10,
            fade_in_smooth: true,
            ..Default::default()
        };
        let out = mix(&[p], 1, 0);
        assert!(out[2] < 0.2 - 1e-6, "smoothstep(0.2)=0.104 < 0.2: {}", out[2]);
        assert!(out[8] > 0.8 + 1e-6, "smoothstep(0.8)=0.896 > 0.8: {}", out[8]);
    }

    #[test]
    fn overlapping_fades_take_min_not_product() {
        // 10-sample clip, both fades 8 samples → they overlap. At i=5 both ramps
        // give 0.625; min keeps 0.625 rather than the product 0.39.
        let p = AudioPlacement {
            start_sample: 0,
            samples: vec![1.0; 10],
            volume: 1.0,
            fade_in_samples: 8,
            fade_out_samples: 8,
            ..Default::default()
        };
        let out = mix(&[p], 1, 0);
        assert!((out[5] - 0.625).abs() < 1e-6, "min of the two ramps: {}", out[5]);
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

    #[test]
    fn resample_upsamples_with_interpolation() {
        // 2 mono frames [0, 1] → 4 frames interpolates the ramp.
        let out = resample_linear(&[0.0, 1.0], 1, 4);
        assert_eq!(out.len(), 4);
        assert_eq!(out[0], 0.0);
        assert!(out[1] > 0.0 && out[1] < out[2], "monotonic ramp");
        assert!(out[3] >= out[2]);
    }

    #[test]
    fn resample_downsamples_by_averaging() {
        // 4 frames → 2 (2x speed): each output averages its 2-sample window.
        let out = resample_linear(&[0.0, 0.2, 0.4, 0.6], 1, 2);
        assert_eq!(out.len(), 2);
        assert!((out[0] - 0.1).abs() < 1e-6, "avg(0.0,0.2)=0.1, got {}", out[0]);
        assert!((out[1] - 0.5).abs() < 1e-6, "avg(0.4,0.6)=0.5, got {}", out[1]);
    }

    #[test]
    fn resample_stereo_keeps_channels() {
        // 2 stereo frames → 3 frames, channels stay separate.
        let out = resample_linear(&[0.0, 1.0, 1.0, 0.0], 2, 3);
        assert_eq!(out.len(), 6);
        assert_eq!(out[0], 0.0); // L of frame 0
        assert_eq!(out[1], 1.0); // R of frame 0
    }

    #[test]
    fn resample_empty_and_single() {
        assert_eq!(resample_linear(&[], 1, 3), vec![0.0, 0.0, 0.0]);
        assert_eq!(resample_linear(&[0.7], 1, 2), vec![0.7, 0.7]);
    }

    #[test]
    fn peaks_of_constant_signal_are_flat() {
        let samples = vec![0.5f32; 100];
        let peaks = compute_peaks(&samples, 1, 4);
        assert_eq!(peaks.len(), 4);
        for p in peaks {
            assert!((p - 0.5).abs() < 1e-6);
        }
    }

    #[test]
    fn peaks_take_absolute_max_across_channels() {
        // Stereo: L quiet, R has a loud negative spike in bucket 1.
        let samples = vec![0.1, 0.1, 0.1, -0.9, 0.1, 0.1, 0.1, 0.1];
        let peaks = compute_peaks(&samples, 2, 2);
        assert_eq!(peaks.len(), 2);
        assert!((peaks[0] - 0.9).abs() < 1e-6, "bucket 0 catches the -0.9 spike");
        assert!((peaks[1] - 0.1).abs() < 1e-6);
    }

    #[test]
    fn peaks_clamp_buckets_to_frame_count() {
        let samples = vec![0.2f32, 0.4, 0.6]; // 3 mono frames
        let peaks = compute_peaks(&samples, 1, 10);
        assert_eq!(peaks.len(), 3);
        assert_eq!(peaks, vec![0.2, 0.4, 0.6]);
    }

    #[test]
    fn peaks_empty_guards() {
        assert!(compute_peaks(&[], 1, 4).is_empty());
        assert!(compute_peaks(&[1.0, 1.0], 0, 4).is_empty());
        assert!(compute_peaks(&[1.0, 1.0], 1, 0).is_empty());
    }
}

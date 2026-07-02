//! Silence detection from audio RMS envelopes (Issue #174).
//!
//! Pure logic — converts RMS amplitude samples into silence ranges in source
//! seconds, then maps those ranges to project frame intervals honoring clip
//! offset, trim, and speed. No AI/transcription dependency.

/// Silence detection configuration.
#[derive(Debug, Clone)]
pub struct SilenceDetectionConfig {
    /// RMS amplitude threshold below which audio is considered silent.
    /// Linear scale 0.0–1.0; typical useful range is 0.001–0.05.
    /// Default: 0.01 (~–40 dBFS).
    pub threshold: f64,
    /// Minimum consecutive silence duration to emit as a range, in seconds.
    /// Shorter gaps are kept to avoid choppy output. Default: 0.5.
    pub min_silence_seconds: f64,
    /// Padding to subtract from each edge of a detected silent range, in seconds.
    /// Prevents cutting too close to the adjacent speech onset. Default: 0.1.
    pub edge_padding_seconds: f64,
}

impl Default for SilenceDetectionConfig {
    fn default() -> Self {
        Self {
            threshold: 0.01,
            min_silence_seconds: 0.5,
            edge_padding_seconds: 0.1,
        }
    }
}

impl SilenceDetectionConfig {
    /// Convert a dBFS threshold value to a linear RMS threshold.
    ///
    /// e.g. `-40.0` dBFS → `0.01` linear.
    pub fn from_db(db: f64) -> f64 {
        10_f64.powf(db / 20.0)
    }

    /// Convert the stored linear threshold to dBFS.
    pub fn threshold_db(&self) -> f64 {
        20.0 * self.threshold.log10()
    }
}

/// A time range in source seconds (inclusive start, exclusive end).
#[derive(Debug, Clone, PartialEq)]
pub struct SourceRange {
    pub start_seconds: f64,
    pub end_seconds: f64,
}

impl SourceRange {
    pub fn duration_seconds(&self) -> f64 {
        self.end_seconds - self.start_seconds
    }
}

/// Detect silence ranges in an RMS envelope.
///
/// `samples` is a slice of linear RMS amplitudes (0.0–1.0), one per frame.
/// `sample_rate_hz` is the number of samples per second.
///
/// Returns silent ranges in source-seconds after applying padding and
/// minimum-duration filtering.
pub fn detect_silence(
    samples: &[f64],
    sample_rate_hz: f64,
    config: &SilenceDetectionConfig,
) -> Vec<SourceRange> {
    if samples.is_empty() || sample_rate_hz <= 0.0 {
        return Vec::new();
    }

    let seconds_per_sample = 1.0 / sample_rate_hz;
    let threshold = config.threshold;

    // Collect raw silent spans as (start_sample, end_sample_exclusive)
    let mut raw: Vec<(usize, usize)> = Vec::new();
    let mut silent_start: Option<usize> = None;

    for (i, &amp) in samples.iter().enumerate() {
        if amp <= threshold {
            if silent_start.is_none() {
                silent_start = Some(i);
            }
        } else if let Some(start) = silent_start.take() {
            raw.push((start, i));
        }
    }
    // Handle trailing silence
    if let Some(start) = silent_start {
        raw.push((start, samples.len()));
    }

    // Convert to seconds, apply padding and minimum-duration filter
    let padding = config.edge_padding_seconds;
    let min_dur = config.min_silence_seconds;

    raw.into_iter()
        .filter_map(|(start_idx, end_idx)| {
            let raw_start = start_idx as f64 * seconds_per_sample;
            let raw_end = end_idx as f64 * seconds_per_sample;

            // Apply edge padding (shrink the silent region so we don't cut into speech)
            let padded_start = raw_start + padding;
            let padded_end = raw_end - padding;

            if padded_end <= padded_start {
                return None;
            }

            let dur = padded_end - padded_start;
            if dur < min_dur {
                return None;
            }

            Some(SourceRange {
                start_seconds: padded_start,
                end_seconds: padded_end,
            })
        })
        .collect()
}

/// Clip placement parameters for converting source ranges to project frames.
#[derive(Debug, Clone)]
pub struct ClipPlacement {
    /// Frame in the project timeline where this clip starts.
    pub timeline_start_frame: i64,
    /// Duration of the visible clip in project frames.
    pub duration_frames: i64,
    /// Source offset in seconds (trim start — first source second that's visible).
    pub source_offset_seconds: f64,
    /// Playback speed multiplier (1.0 = normal, 2.0 = double speed).
    pub speed: f64,
    /// Project frames per second.
    pub fps: f64,
}

/// Convert source-second silence ranges to project frame ranges.
///
/// Only ranges that fall within the clip's visible portion are returned.
/// Each range is clamped to the clip boundaries and converted to project frames.
pub fn source_ranges_to_project_frames(
    source_ranges: &[SourceRange],
    clip: &ClipPlacement,
) -> Vec<(i64, i64)> {
    let clip_dur_seconds = clip.duration_frames as f64 / clip.fps;

    source_ranges
        .iter()
        .filter_map(|r| {
            // Map source seconds to clip-local seconds (accounting for offset and speed)
            let clip_start = (r.start_seconds - clip.source_offset_seconds) / clip.speed;
            let clip_end = (r.end_seconds - clip.source_offset_seconds) / clip.speed;

            // Clamp to clip visible range
            let clip_start = clip_start.max(0.0);
            let clip_end = clip_end.min(clip_dur_seconds);

            if clip_end <= clip_start {
                return None;
            }

            // Convert to absolute project frames
            let frame_start = clip.timeline_start_frame + (clip_start * clip.fps).round() as i64;
            let frame_end = clip.timeline_start_frame + (clip_end * clip.fps).round() as i64;

            if frame_end <= frame_start {
                return None;
            }

            Some((frame_start, frame_end))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_silence_no_samples() {
        let result = detect_silence(&[], 44100.0, &SilenceDetectionConfig::default());
        assert!(result.is_empty());
    }

    #[test]
    fn detect_silence_all_loud() {
        let samples: Vec<f64> = vec![0.5; 100];
        let result = detect_silence(&samples, 10.0, &SilenceDetectionConfig::default());
        assert!(
            result.is_empty(),
            "no silence when all samples above threshold"
        );
    }

    #[test]
    fn detect_silence_all_silent() {
        // 10 seconds of silence at 10 Hz, min_silence=0.5s, padding=0.1s → one range
        let samples: Vec<f64> = vec![0.0; 100]; // 10 seconds at 10 Hz
        let config = SilenceDetectionConfig {
            threshold: 0.01,
            min_silence_seconds: 0.5,
            edge_padding_seconds: 0.1,
        };
        let result = detect_silence(&samples, 10.0, &config);
        assert_eq!(result.len(), 1);
        // Raw range: 0.0–10.0, padded: 0.1–9.9
        assert!((result[0].start_seconds - 0.1).abs() < 0.01);
        assert!((result[0].end_seconds - 9.9).abs() < 0.01);
    }

    #[test]
    fn detect_silence_middle_silence() {
        // Pattern: 1s loud, 2s silent, 1s loud at 10 Hz
        let mut samples = vec![0.5f64; 10]; // 1s loud
        samples.extend(vec![0.0f64; 20]); // 2s silent
        samples.extend(vec![0.5f64; 10]); // 1s loud

        let config = SilenceDetectionConfig {
            threshold: 0.01,
            min_silence_seconds: 0.5,
            edge_padding_seconds: 0.1,
        };
        let result = detect_silence(&samples, 10.0, &config);
        assert_eq!(result.len(), 1, "one silent range in the middle");
        // Raw: 1.0–3.0s, padded: 1.1–2.9s
        assert!((result[0].start_seconds - 1.1).abs() < 0.01);
        assert!((result[0].end_seconds - 2.9).abs() < 0.01);
    }

    #[test]
    fn detect_silence_too_short_filtered() {
        // 0.2s silence — below min_silence=0.5s → filtered out
        let mut samples = vec![0.5f64; 10]; // 1s loud
        samples.extend(vec![0.0f64; 2]); // 0.2s silent
        samples.extend(vec![0.5f64; 10]); // 1s loud

        let config = SilenceDetectionConfig {
            threshold: 0.01,
            min_silence_seconds: 0.5,
            edge_padding_seconds: 0.0, // no padding for this test
        };
        let result = detect_silence(&samples, 10.0, &config);
        assert!(result.is_empty(), "0.2s silence is below 0.5s minimum");
    }

    #[test]
    fn detect_silence_padding_eliminates_short_ranges() {
        // 0.4s silence — after 0.1+0.1=0.2s padding → 0.2s remaining → below 0.5s → filtered
        let mut samples = vec![0.5f64; 10]; // 1s loud
        samples.extend(vec![0.0f64; 4]); // 0.4s silent
        samples.extend(vec![0.5f64; 10]); // 1s loud

        let config = SilenceDetectionConfig {
            threshold: 0.01,
            min_silence_seconds: 0.5,
            edge_padding_seconds: 0.1,
        };
        let result = detect_silence(&samples, 10.0, &config);
        assert!(
            result.is_empty(),
            "0.4s - 0.2s padding = 0.2s < min 0.5s → filtered"
        );
    }

    #[test]
    fn db_to_linear_conversion() {
        let linear = SilenceDetectionConfig::from_db(-40.0);
        assert!((linear - 0.01).abs() < 1e-4, "linear={linear}");
        let linear_0db = SilenceDetectionConfig::from_db(0.0);
        assert!((linear_0db - 1.0).abs() < 1e-9);
    }

    #[test]
    fn linear_to_db_conversion() {
        let config = SilenceDetectionConfig {
            threshold: 0.01,
            ..Default::default()
        };
        let db = config.threshold_db();
        assert!((db - (-40.0)).abs() < 0.1, "db={db}");
    }

    #[test]
    fn source_ranges_to_frames_basic() {
        let placement = ClipPlacement {
            timeline_start_frame: 100,
            duration_frames: 300, // 10s at 30fps
            source_offset_seconds: 0.0,
            speed: 1.0,
            fps: 30.0,
        };
        let ranges = vec![SourceRange {
            start_seconds: 1.0,
            end_seconds: 3.0,
        }];
        let frames = source_ranges_to_project_frames(&ranges, &placement);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].0, 130); // 100 + 1.0*30
        assert_eq!(frames[0].1, 190); // 100 + 3.0*30
    }

    #[test]
    fn source_ranges_to_frames_with_speed() {
        let placement = ClipPlacement {
            timeline_start_frame: 0,
            duration_frames: 150, // 5s at 30fps (sped up clip)
            source_offset_seconds: 0.0,
            speed: 2.0, // 2x speed — 5s source → 2.5s visible
            fps: 30.0,
        };
        // Silence at 0–2s in source → 0–1s in timeline at 2x speed
        let ranges = vec![SourceRange {
            start_seconds: 0.0,
            end_seconds: 2.0,
        }];
        let frames = source_ranges_to_project_frames(&ranges, &placement);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].0, 0);
        assert_eq!(frames[0].1, 30); // 1s * 30fps
    }

    #[test]
    fn source_ranges_to_frames_outside_clip_filtered() {
        let placement = ClipPlacement {
            timeline_start_frame: 0,
            duration_frames: 60, // 2s at 30fps
            source_offset_seconds: 0.0,
            speed: 1.0,
            fps: 30.0,
        };
        // Silence entirely outside the clip's visible window
        let ranges = vec![SourceRange {
            start_seconds: 5.0,
            end_seconds: 8.0,
        }];
        let frames = source_ranges_to_project_frames(&ranges, &placement);
        assert!(frames.is_empty(), "range outside clip should be filtered");
    }

    #[test]
    fn source_ranges_to_frames_with_offset() {
        // Clip starts at source second 10.0 (trim start)
        let placement = ClipPlacement {
            timeline_start_frame: 0,
            duration_frames: 300, // 10s
            source_offset_seconds: 10.0,
            speed: 1.0,
            fps: 30.0,
        };
        // Silence at source seconds 12–14 → clip-local 2–4s → frames 60–120
        let ranges = vec![SourceRange {
            start_seconds: 12.0,
            end_seconds: 14.0,
        }];
        let frames = source_ranges_to_project_frames(&ranges, &placement);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].0, 60);
        assert_eq!(frames[0].1, 120);
    }
}

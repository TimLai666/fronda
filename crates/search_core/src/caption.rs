use serde::{Deserialize, Serialize};

use crate::transcript::TranscribedWord;

/// Text case mode for caption output (CAP-013).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TextCase {
    #[default]
    Auto,
    Upper,
    Lower,
}

/// Configuration for caption generation.
/// CAP-004: auto-detect chooses dominant spoken track.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CaptionConfig {
    pub words_per_caption: u32,
    pub min_duration_seconds: f64,
    pub max_gap_seconds: f64,
    pub auto_detect_track: bool,
    pub target_clip_ids: Option<Vec<String>>,
    /// CAP-006: bypass transcript cache when profanity-censoring differs.
    #[serde(default)]
    pub profanity_censor: bool,
    /// CAP-006: explicit locale override bypasses cached locale transcript.
    #[serde(default)]
    pub locale_override: Option<String>,
    /// CAP-013: text case mode for caption output.
    #[serde(default)]
    pub text_case: TextCase,
}

impl Default for CaptionConfig {
    /// CAP-004: auto_detect = true, words_per_caption = 6,
    /// min_duration = 0.7, max_gap = 0.7
    fn default() -> Self {
        Self {
            words_per_caption: 6,
            min_duration_seconds: 0.7,
            max_gap_seconds: 0.7,
            auto_detect_track: true,
            target_clip_ids: None,
            profanity_censor: false,
            locale_override: None,
            text_case: TextCase::Auto,
        }
    }
}

/// A single caption segment with timing.
#[derive(Debug, Clone, PartialEq)]
pub struct CaptionSegment {
    pub text: String,
    pub start_frame: i64,
    pub end_frame: i64,
}

impl CaptionSegment {
    pub fn new(text: &str, start_frame: i64, end_frame: i64) -> Self {
        Self {
            text: text.to_string(),
            start_frame,
            end_frame,
        }
    }

    pub fn duration_frames(&self) -> i64 {
        self.end_frame - self.start_frame
    }

    /// Apply text case transformation in-place (CAP-013).
    pub fn apply_text_case(&mut self, mode: TextCase) {
        match mode {
            TextCase::Upper => self.text = self.text.to_uppercase(),
            TextCase::Lower => self.text = self.text.to_lowercase(),
            TextCase::Auto => {} // leave as-is
        }
    }
}

/// Metadata describing a clip's suitability as a caption source.
#[derive(Debug, Clone, PartialEq)]
pub struct ClipCaptionInfo {
    pub clip_id: String,
    pub media_type: String, // "video", "audio", "image", etc.
    pub link_group_id: Option<String>,
    pub has_audio_track: bool,
    pub is_silent: bool,
    pub is_generating: bool,
    pub word_count: u64,
    /// Frame range of this clip on the timeline.
    pub start_frame: i64,
    pub end_frame: i64,
}

/// Result of caption generation planning.
#[derive(Debug, Clone, PartialEq)]
pub struct CaptionPlan {
    pub segments: Vec<CaptionSegment>,
    pub source_media_id: Option<String>,
}

/// The outcome of attempting to place captions on the timeline (CAP-010..012).
#[derive(Debug, Clone, PartialEq)]
pub struct CaptionPlacement {
    /// The generated caption segments after placement.
    pub segments: Vec<CaptionSegment>,
    /// The id of the track that was created/used for captions, if any.
    pub track_id: Option<String>,
    /// Whether the placement resulted in any clips (CAP-011).
    pub has_clips: bool,
}

// ===================================================================
// CAP-001/002: Valid caption source checking
// ===================================================================

/// Returns true if a clip has transcribable audio (CAP-001).
pub fn has_transcribable_audio(info: &ClipCaptionInfo) -> bool {
    if info.is_generating {
        return false;
    }
    info.has_audio_track && !info.is_silent
}

// ===================================================================
// CAP-002/003: Caption source selection
// ===================================================================

/// Select a single best caption source from available clips (CAP-002, CAP-003).
///
/// - Silent videos are never selected (CAP-002).
/// - When linked audio/video clips exist for the same source, the audio side
///   is preferred over the video side (CAP-003).
/// - Returns `None` if no valid source exists.
pub fn select_caption_source(clips: &[ClipCaptionInfo]) -> Option<&ClipCaptionInfo> {
    // Filter out clips without transcribable audio (CAP-001, CAP-002).
    let valid: Vec<&ClipCaptionInfo> = clips
        .iter()
        .filter(|c| has_transcribable_audio(c))
        .collect();

    if valid.is_empty() {
        return None;
    }

    // For CAP-003: prefer audio-side clips when linked pairs exist.
    // First, check if any linked audio clip exists.
    for clip in &valid {
        if clip.media_type == "audio" && clip.link_group_id.is_some() {
            return Some(clip);
        }
    }

    // No linked audio preference; return the first valid source.
    // In auto-detect mode (CAP-004), the caller sorts by word count first.
    Some(valid[0])
}

// ===================================================================
// CAP-005/006: Transcript cache decisions
// ===================================================================

/// Determine whether the transcript cache should be bypassed (CAP-005, CAP-006).
///
/// Returns `true` if the cache should be bypassed because the requested
/// profanity-censoring or locale option would produce a different transcript
/// than what is already cached.
pub fn should_bypass_cache(
    config: &CaptionConfig,
    cached_locale: Option<&str>,
    cached_profanity_censored: bool,
) -> bool {
    // CAP-005: By default, reuse cached transcripts.
    // Only bypass when something changed that would produce a different result.

    // CAP-006: Bypass if profanity-censoring doesn't match.
    if config.profanity_censor != cached_profanity_censored {
        return true;
    }

    // CAP-006: Bypass if locale override differs from cached locale.
    if let Some(requested_locale) = &config.locale_override {
        match cached_locale {
            Some(cached) if cached == requested_locale => {}
            _ => return true,
        }
    }

    false
}

// ===================================================================
// CAP-009: Meaningful overlap checking
// ===================================================================

/// Minimum overlap fraction required for a caption segment to be assigned
/// to a destination clip (CAP-009).
const MIN_OVERLAP_FRACTION: f64 = 0.3;

/// Check whether a caption segment has meaningful overlap with a clip
/// (CAP-009). Returns `true` if the overlap fraction exceeds the threshold.
pub fn has_meaningful_overlap(
    caption_start: i64,
    caption_end: i64,
    clip_start: i64,
    clip_end: i64,
) -> bool {
    let overlap_start = caption_start.max(clip_start);
    let overlap_end = caption_end.min(clip_end);

    if overlap_end <= overlap_start {
        return false;
    }

    let caption_duration = caption_end - caption_start;
    if caption_duration <= 0 {
        return false;
    }

    let overlap_fraction = (overlap_end - overlap_start) as f64 / caption_duration as f64;
    overlap_fraction >= MIN_OVERLAP_FRACTION
}

// ===================================================================
// CAP-010/011/012: Caption placement on timeline
// ===================================================================

/// Place caption segments onto a new video track (CAP-010..012).
///
/// This is a pure-logic validation function that checks:
/// - CAP-011: If no clips result from placement, the track should be reverted.
/// - CAP-012: Placement must not accidentally prune unrelated tracks.
///
/// Returns a `CaptionPlacement` describing the outcome.
pub fn plan_caption_placement(
    segments: &[CaptionSegment],
    existing_track_count: usize,
) -> CaptionPlacement {
    // CAP-010: Captions go on a fresh top video track.
    // (The actual track creation is done by the caller; we validate here.)

    // CAP-011: If no segments, no clips will result.
    let has_clips = !segments.is_empty();

    // CAP-012: Existing tracks are untouched — we only add a new track.
    // This check is done by the caller verifying existing_track_count is preserved.

    let track_id = if has_clips {
        Some(format!("caption-track-{}", existing_track_count + 1))
    } else {
        None
    };

    CaptionPlacement {
        segments: segments.to_vec(),
        track_id,
        has_clips,
    }
}

/// Returns true if placement plan has no clips and should be reverted (CAP-011).
pub fn should_revert_placement(placement: &CaptionPlacement) -> bool {
    // CAP-011: If caption placement yields no clips, the inserted track
    // should be reverted.
    !placement.has_clips && placement.track_id.is_some()
}

/// Groups words into caption segments using real pause gaps.
///
/// This is from upstream #92: word-accurate per-word timestamps grouping.
///
/// Algorithm: iterate words, group until word count reaches `words_per_caption`
/// OR the gap between words exceeds `max_gap` (converted to seconds).
/// Each group becomes a `CaptionSegment` with frame-based timing derived
/// from word timestamps. The segments' text case is applied per CAP-013.
pub fn phrases_from_words(
    words: &[TranscribedWord],
    config: &CaptionConfig,
    fps: i64,
) -> Vec<CaptionSegment> {
    if words.is_empty() {
        return Vec::new();
    }

    let fps = fps.max(1);
    let mut segments: Vec<CaptionSegment> = Vec::new();
    let mut group_start: Option<f64> = None;
    let mut group_end: f64 = 0.0;
    let mut group_words: Vec<String> = Vec::new();
    let mut word_count_in_group: u32 = 0;

    for word in words {
        if !word.has_valid_timestamps() {
            continue;
        }

        if group_start.is_none() {
            group_start = Some(word.start_seconds);
            group_end = word.end_seconds;
            group_words.push(word.word.clone());
            word_count_in_group = 1;
            continue;
        }

        // Check if gap between previous word end and current word start
        // exceeds the max_gap threshold.
        let gap = word.start_seconds - group_end;

        if gap > config.max_gap_seconds || word_count_in_group >= config.words_per_caption {
            // Emit the current group and start a new one.
            if let Some(start) = group_start {
                let start_frame = (start * fps as f64).round() as i64;
                let end_frame = (group_end * fps as f64).round() as i64;
                segments.push(CaptionSegment::new(
                    &group_words.join(" "),
                    start_frame,
                    end_frame,
                ));
            }
            group_start = Some(word.start_seconds);
            group_end = word.end_seconds;
            group_words = vec![word.word.clone()];
            word_count_in_group = 1;
        } else {
            group_end = word.end_seconds;
            group_words.push(word.word.clone());
            word_count_in_group += 1;
        }
    }

    // Emit the final group.
    if let Some(start) = group_start {
        let start_frame = (start * fps as f64).round() as i64;
        let end_frame = (group_end * fps as f64).round() as i64;
        segments.push(CaptionSegment::new(
            &group_words.join(" "),
            start_frame,
            end_frame,
        ));
    }

    // CAP-008 / spec THM-015: floor each caption's display duration.
    let min_frames = (config.min_duration_seconds * fps as f64).round() as i64;
    enforce_min_duration(&mut segments, min_frames);

    // Apply text case to all segments (CAP-013).
    for seg in &mut segments {
        seg.apply_text_case(config.text_case);
    }

    segments
}

/// CAP-008: enforce a minimum display duration on word-timestamp caption segments
/// (assumed chronological and non-overlapping) by **clamping, not shifting**.
///
/// Each segment's `end_frame` extends toward `start_frame + min_frames`, but never
/// past the next segment's `start_frame`; the final segment extends freely. Starts
/// are never moved and segments are never shrunk, so real word onsets stay synced
/// to speech and the non-overlap invariant (`end <= next.start`) is preserved.
///
/// This deliberately diverges from Swift `CaptionBuilder.enforceMinDuration`, which
/// extends *and shifts every later phrase* to avoid overlap. That is safe in Swift
/// because its phrases are synthetic character-distributed subdivisions of a single
/// transcript segment (no real per-phrase timing); shifting them here — where each
/// segment is anchored to real spoken word times — would drift captions
/// progressively behind the audio across fast contiguous speech.
fn enforce_min_duration(segments: &mut [CaptionSegment], min_frames: i64) {
    if min_frames <= 0 {
        return;
    }
    let n = segments.len();
    for i in 0..n {
        let desired_end = segments[i].start_frame + min_frames;
        // Cap the extension at the next segment's (unchanged) start; last is free.
        let cap = if i + 1 < n {
            segments[i + 1].start_frame
        } else {
            i64::MAX
        };
        segments[i].end_frame = segments[i].end_frame.max(desired_end.min(cap));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcript::TranscribedWord;

    // ===================================================================
    // CAP-001: Only clips with transcribable audio are valid
    // ===================================================================

    #[test]
    fn cap_001_transcribable_audio_valid() {
        let info = ClipCaptionInfo {
            clip_id: "c1".into(),
            media_type: "audio".into(),
            link_group_id: None,
            has_audio_track: true,
            is_silent: false,
            is_generating: false,
            word_count: 100,
            start_frame: 0,
            end_frame: 300,
        };
        assert!(has_transcribable_audio(&info));
    }

    #[test]
    fn cap_001_no_audio_track_invalid() {
        let info = ClipCaptionInfo {
            clip_id: "c1".into(),
            media_type: "video".into(),
            link_group_id: None,
            has_audio_track: false,
            is_silent: false,
            is_generating: false,
            word_count: 0,
            start_frame: 0,
            end_frame: 100,
        };
        assert!(!has_transcribable_audio(&info));
    }

    #[test]
    fn cap_001_generating_clip_invalid() {
        let info = ClipCaptionInfo {
            clip_id: "c1".into(),
            media_type: "audio".into(),
            link_group_id: None,
            has_audio_track: true,
            is_silent: false,
            is_generating: true,
            word_count: 0,
            start_frame: 0,
            end_frame: 100,
        };
        assert!(!has_transcribable_audio(&info));
    }

    // ===================================================================
    // CAP-002: Silent video never selected
    // ===================================================================

    #[test]
    fn cap_002_silent_video_not_selected() {
        let clips = vec![
            ClipCaptionInfo {
                clip_id: "silent".into(),
                media_type: "video".into(),
                link_group_id: None,
                has_audio_track: true,
                is_silent: true,
                is_generating: false,
                word_count: 0,
                start_frame: 0,
                end_frame: 100,
            },
            ClipCaptionInfo {
                clip_id: "audio-ok".into(),
                media_type: "audio".into(),
                link_group_id: None,
                has_audio_track: true,
                is_silent: false,
                is_generating: false,
                word_count: 50,
                start_frame: 0,
                end_frame: 100,
            },
        ];
        let selected = select_caption_source(&clips);
        assert!(selected.is_some());
        assert_eq!(selected.unwrap().clip_id, "audio-ok");
    }

    #[test]
    fn cap_002_all_silent_returns_none() {
        let clips = vec![ClipCaptionInfo {
            clip_id: "silent".into(),
            media_type: "video".into(),
            link_group_id: None,
            has_audio_track: true,
            is_silent: true,
            is_generating: false,
            word_count: 0,
            start_frame: 0,
            end_frame: 100,
        }];
        assert!(select_caption_source(&clips).is_none());
    }

    #[test]
    fn cap_002_empty_clips_returns_none() {
        assert!(select_caption_source(&[]).is_none());
    }

    // ===================================================================
    // CAP-003: Linked audio/video → target audio side
    // ===================================================================

    #[test]
    fn cap_003_linked_audio_preferred_over_video() {
        let clips = vec![
            ClipCaptionInfo {
                clip_id: "video-clip".into(),
                media_type: "video".into(),
                link_group_id: Some("link-1".into()),
                has_audio_track: true,
                is_silent: false,
                is_generating: false,
                word_count: 80,
                start_frame: 0,
                end_frame: 200,
            },
            ClipCaptionInfo {
                clip_id: "audio-clip".into(),
                media_type: "audio".into(),
                link_group_id: Some("link-1".into()),
                has_audio_track: true,
                is_silent: false,
                is_generating: false,
                word_count: 80,
                start_frame: 0,
                end_frame: 200,
            },
        ];
        let selected = select_caption_source(&clips);
        assert!(selected.is_some());
        assert_eq!(selected.unwrap().clip_id, "audio-clip");
    }

    #[test]
    fn cap_003_no_linked_audio_fallback_to_video() {
        let clips = vec![ClipCaptionInfo {
            clip_id: "vid-only".into(),
            media_type: "video".into(),
            link_group_id: None,
            has_audio_track: true,
            is_silent: false,
            is_generating: false,
            word_count: 50,
            start_frame: 0,
            end_frame: 100,
        }];
        let selected = select_caption_source(&clips);
        assert!(selected.is_some());
        assert_eq!(selected.unwrap().clip_id, "vid-only");
    }

    // ===================================================================
    // CAP-005: Reuse cached transcripts by default
    // ===================================================================

    #[test]
    fn cap_005_no_bypass_when_matches() {
        let config = CaptionConfig::default();
        assert!(!should_bypass_cache(&config, Some("en-US"), false));
    }

    #[test]
    fn cap_005_bypass_when_cache_missing() {
        let config = CaptionConfig {
            locale_override: Some("zh-TW".into()),
            ..Default::default()
        };
        assert!(should_bypass_cache(&config, None, false));
    }

    // ===================================================================
    // CAP-006: Bypass cache when profanity/locale differs
    // ===================================================================

    #[test]
    fn cap_006_bypass_when_profanity_differs() {
        let config = CaptionConfig {
            profanity_censor: true,
            ..Default::default()
        };
        assert!(should_bypass_cache(&config, Some("en-US"), false));
        // Not bypass when they match
        assert!(!should_bypass_cache(&config, Some("en-US"), true));
    }

    #[test]
    fn cap_006_bypass_when_locale_differs() {
        let config = CaptionConfig {
            locale_override: Some("fr-FR".into()),
            ..Default::default()
        };
        assert!(should_bypass_cache(&config, Some("en-US"), false));
        assert!(!should_bypass_cache(&config, Some("fr-FR"), false));
    }

    #[test]
    fn cap_006_no_bypass_when_both_match() {
        let config = CaptionConfig {
            profanity_censor: true,
            locale_override: Some("zh-TW".into()),
            ..Default::default()
        };
        assert!(!should_bypass_cache(&config, Some("zh-TW"), true));
    }

    // ===================================================================
    // CAP-009: Meaningful overlap required
    // ===================================================================

    #[test]
    fn cap_009_full_overlap_is_meaningful() {
        assert!(has_meaningful_overlap(10, 50, 10, 50));
    }

    #[test]
    fn cap_009_partial_overlap_above_threshold() {
        // Caption: 0–100, clip: 60–100 → overlap 40/100 = 0.4 >= 0.3
        assert!(has_meaningful_overlap(0, 100, 60, 100));
    }

    #[test]
    fn cap_009_small_overlap_below_threshold() {
        // Caption: 0–100, clip: 95–100 → overlap 5/100 = 0.05 < 0.3
        assert!(!has_meaningful_overlap(0, 100, 95, 100));
    }

    #[test]
    fn cap_009_no_overlap() {
        assert!(!has_meaningful_overlap(0, 50, 60, 100));
    }

    #[test]
    fn cap_009_empty_caption() {
        assert!(!has_meaningful_overlap(10, 10, 10, 20));
    }

    // ===================================================================
    // CAP-010: Generated captions on fresh top video track
    // ===================================================================

    #[test]
    fn cap_010_placement_creates_new_track() {
        let segs = vec![CaptionSegment::new("hello", 0, 30)];
        let placement = plan_caption_placement(&segs, 3);
        assert_eq!(placement.track_id, Some("caption-track-4".into()));
        assert!(placement.has_clips);
    }

    // ===================================================================
    // CAP-011: No clips → revert inserted track
    // ===================================================================

    #[test]
    fn cap_011_empty_segments_means_no_clips() {
        let placement = plan_caption_placement(&[], 2);
        assert!(!placement.has_clips);
        assert_eq!(placement.track_id, None);
    }

    #[test]
    fn cap_011_should_revert_when_no_clips() {
        let placement = CaptionPlacement {
            segments: vec![],
            track_id: Some("caption-1".into()),
            has_clips: false,
        };
        assert!(should_revert_placement(&placement));
    }

    #[test]
    fn cap_011_should_not_revert_when_clips_exist() {
        let placement = CaptionPlacement {
            segments: vec![CaptionSegment::new("test", 0, 30)],
            track_id: Some("caption-1".into()),
            has_clips: true,
        };
        assert!(!should_revert_placement(&placement));
    }

    // ===================================================================
    // CAP-012: Don't prune unrelated tracks
    // ===================================================================

    #[test]
    fn cap_012_planning_does_not_modify_existing_tracks() {
        // plan_caption_placement validates that existing tracks are preserved.
        // The caller is responsible for not pruning unrelated tracks.
        let segs = vec![CaptionSegment::new("hello", 0, 30)];
        let existing = 5;
        let placement = plan_caption_placement(&segs, existing);
        // Track id reflects existing count + 1, not overwriting anything.
        assert_eq!(placement.track_id, Some("caption-track-6".into()));
        assert!(placement.has_clips);
    }

    // ===================================================================
    // CAP-013: Text case modes
    // ===================================================================

    #[test]
    fn cap_013_text_case_auto() {
        let mut seg = CaptionSegment::new("Hello World", 0, 30);
        seg.apply_text_case(TextCase::Auto);
        assert_eq!(seg.text, "Hello World");
    }

    #[test]
    fn cap_013_text_case_upper() {
        let mut seg = CaptionSegment::new("Hello World", 0, 30);
        seg.apply_text_case(TextCase::Upper);
        assert_eq!(seg.text, "HELLO WORLD");
    }

    #[test]
    fn cap_013_text_case_lower() {
        let mut seg = CaptionSegment::new("Hello World", 0, 30);
        seg.apply_text_case(TextCase::Lower);
        assert_eq!(seg.text, "hello world");
    }

    #[test]
    fn cap_013_phrases_from_words_upper() {
        let words = vec![
            TranscribedWord {
                word: "hello".into(),
                start_seconds: 0.0,
                end_seconds: 0.5,
            },
            TranscribedWord {
                word: "world".into(),
                start_seconds: 0.6,
                end_seconds: 1.2,
            },
        ];
        let config = CaptionConfig {
            words_per_caption: 10,
            max_gap_seconds: 10.0,
            text_case: TextCase::Upper,
            ..Default::default()
        };
        let segs = phrases_from_words(&words, &config, 30);
        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0].text, "HELLO WORLD");
    }

    #[test]
    fn cap_013_phrases_from_words_lower() {
        let words = vec![TranscribedWord {
            word: "Hello".into(),
            start_seconds: 0.0,
            end_seconds: 0.5,
        }];
        let config = CaptionConfig {
            text_case: TextCase::Lower,
            ..Default::default()
        };
        let segs = phrases_from_words(&words, &config, 30);
        assert_eq!(segs[0].text, "hello");
    }

    // ===================================================================
    // Existing phrase-from-words tests (upstream #92)
    // ===================================================================

    #[test]
    fn cap_001_caption_config_default() {
        let config = CaptionConfig::default();
        assert_eq!(config.words_per_caption, 6);
        assert!((config.min_duration_seconds - 0.7).abs() < 1e-9);
        assert!((config.max_gap_seconds - 0.7).abs() < 1e-9);
        assert_eq!(config.text_case, TextCase::Auto);
        assert!(!config.profanity_censor);
        assert!(config.locale_override.is_none());
    }

    #[test]
    fn cap_004_auto_detect_default() {
        let config = CaptionConfig::default();
        assert!(config.auto_detect_track);
        assert!(config.target_clip_ids.is_none());
    }

    #[test]
    fn caption_segment_duration() {
        let seg = CaptionSegment::new("hello world", 10, 25);
        assert_eq!(seg.duration_frames(), 15);
    }

    #[test]
    fn upstream_092_phrases_from_words_basic() {
        let words = vec![
            TranscribedWord {
                word: "hello".into(),
                start_seconds: 0.0,
                end_seconds: 0.5,
            },
            TranscribedWord {
                word: "world".into(),
                start_seconds: 0.6,
                end_seconds: 1.2,
            },
            TranscribedWord {
                word: "foo".into(),
                start_seconds: 1.3,
                end_seconds: 1.8,
            },
        ];
        let config = CaptionConfig {
            words_per_caption: 10,
            max_gap_seconds: 10.0,
            ..Default::default()
        };
        let segs = phrases_from_words(&words, &config, 30);
        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0].text, "hello world foo");
    }

    #[test]
    fn upstream_092_phrases_from_words_with_gap() {
        let words = vec![
            TranscribedWord {
                word: "hello".into(),
                start_seconds: 0.0,
                end_seconds: 0.5,
            },
            TranscribedWord {
                word: "world".into(),
                start_seconds: 0.6,
                end_seconds: 1.2,
            },
            TranscribedWord {
                word: "far".into(),
                start_seconds: 3.2,
                end_seconds: 3.8,
            },
        ];
        let config = CaptionConfig {
            words_per_caption: 10,
            max_gap_seconds: 1.0,
            ..Default::default()
        };
        let segs = phrases_from_words(&words, &config, 30);
        assert_eq!(segs.len(), 2);
        assert_eq!(segs[0].text, "hello world");
        assert_eq!(segs[1].text, "far");
    }

    #[test]
    fn upstream_092_phrases_from_words_max_words() {
        let words = vec![
            TranscribedWord {
                word: "one".into(),
                start_seconds: 0.0,
                end_seconds: 0.3,
            },
            TranscribedWord {
                word: "two".into(),
                start_seconds: 0.4,
                end_seconds: 0.7,
            },
            TranscribedWord {
                word: "three".into(),
                start_seconds: 0.8,
                end_seconds: 1.1,
            },
            TranscribedWord {
                word: "four".into(),
                start_seconds: 1.2,
                end_seconds: 1.5,
            },
            TranscribedWord {
                word: "five".into(),
                start_seconds: 1.6,
                end_seconds: 1.9,
            },
        ];
        let config = CaptionConfig {
            words_per_caption: 2,
            max_gap_seconds: 10.0,
            ..Default::default()
        };
        let segs = phrases_from_words(&words, &config, 30);
        assert_eq!(segs.len(), 3);
        assert_eq!(segs[0].text, "one two");
        assert_eq!(segs[1].text, "three four");
        assert_eq!(segs[2].text, "five");
    }

    #[test]
    fn phrases_from_words_no_gap_groups_together() {
        let words = vec![
            TranscribedWord {
                word: "a".into(),
                start_seconds: 0.0,
                end_seconds: 0.5,
            },
            TranscribedWord {
                word: "b".into(),
                start_seconds: 0.5,
                end_seconds: 1.0,
            },
        ];
        let config = CaptionConfig::default();
        let phrases = phrases_from_words(&words, &config, 30);
        assert_eq!(phrases.len(), 1, "no gap → one group");
        assert_eq!(phrases[0].text, "a b");
    }
}

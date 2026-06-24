//! Spec tests for caption generation (CAP series + Upstream #92).
//!
//! These tests validate the observable behavior of `phrases_from_words`
//! and related caption types against the spec-compatibility baseline.
//! Higher-level orchestration (cache reuse, track insertion) is validated
//! at the integration level; these tests cover the phrase-building layer.

use search_core::caption::{phrases_from_words, CaptionConfig, CaptionSegment};
use search_core::transcript::TranscribedWord;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build evenly-spaced word vectors from a text string.
fn even_words(text: &str, start: f64, end: f64) -> Vec<TranscribedWord> {
    let parts: Vec<&str> = text.split_whitespace().collect();
    let n = parts.len() as f64;
    parts
        .into_iter()
        .enumerate()
        .map(|(i, word)| {
            let ws = start + (end - start) * (i as f64 / n);
            let we = ws + (end - start) / n;
            TranscribedWord {
                word: word.to_string(),
                start_seconds: ws,
                end_seconds: we,
            }
        })
        .collect()
}

/// Shorthand: single-word transcript.
fn word(w: &str, s: f64, e: f64) -> TranscribedWord {
    TranscribedWord {
        word: w.to_string(),
        start_seconds: s,
        end_seconds: e,
    }
}

// ===================================================================
// CAP-001: Only clips with transcribable audio are valid caption sources
// ===================================================================

#[test]
fn cap_001_empty_words_produces_no_captions() {
    let config = CaptionConfig::default();
    let segs = phrases_from_words(&[], &config, 30);
    assert!(segs.is_empty(), "no words → no captions");
}

#[test]
fn cap_001_words_without_valid_timestamps_skipped() {
    // Every word has invalid (negative) timestamps → no valid input
    let words = vec![word("silent", -1.0, -0.5), word("clip", -1.0, -0.5)];
    let config = CaptionConfig::default();
    let segs = phrases_from_words(&words, &config, 30);
    assert!(segs.is_empty(), "no valid timestamps → no captions");
}

#[test]
fn cap_001_mixed_valid_invalid_timestamps() {
    // Only valid words should appear in output
    let words = vec![
        word("valid", 0.0, 0.5),
        word("bad", -1.0, 0.3),
        word("also", 0.0, -1.0),
        word("good", 0.6, 1.0),
    ];
    let config = CaptionConfig {
        words_per_caption: 10,
        max_gap_seconds: 10.0,
        ..Default::default()
    };
    let segs = phrases_from_words(&words, &config, 30);
    assert_eq!(segs.len(), 1, "only valid words produce segments");
    assert_eq!(segs[0].text, "valid good");
}

// ===================================================================
// CAP-005: Caption generation reuses cached transcripts by default
// ===================================================================

#[test]
fn cap_005_phrases_from_words_is_deterministic() {
    // Deterministic output → same input always produces same phrases.
    let words = even_words("hello world this is a test", 0.0, 3.0);
    let config = CaptionConfig::default();
    let a = phrases_from_words(&words, &config, 30);
    let b = phrases_from_words(&words, &config, 30);
    assert_eq!(a, b, "deterministic input → deterministic output");
}

// ===================================================================
// CAP-007: Phrase splitting preserves heuristics
// ===================================================================

#[test]
fn cap_007_gap_based_split() {
    // A large gap between groups of words should split into separate phrases.
    let words = vec![
        word("first", 0.0, 0.5),
        word("group", 0.6, 1.0),
        word("second", 3.0, 3.5),
        word("group", 3.6, 4.0),
    ];
    let config = CaptionConfig {
        words_per_caption: 10,
        max_gap_seconds: 1.0,
        ..Default::default()
    };
    let segs = phrases_from_words(&words, &config, 30);
    assert_eq!(segs.len(), 2);
    assert_eq!(segs[0].text, "first group");
    assert_eq!(segs[1].text, "second group");
}

#[test]
fn cap_007_word_count_based_split() {
    // Even when gaps are tiny, hitting words_per_caption forces a split.
    let words = even_words("a b c d e f g h i j k l m n o p", 0.0, 4.0);
    let config = CaptionConfig {
        words_per_caption: 5,
        max_gap_seconds: 10.0,
        ..Default::default()
    };
    let segs = phrases_from_words(&words, &config, 30);
    // 16 words / 5 = 4 groups (5+5+5+1)
    assert_eq!(segs.len(), 4);
    assert_eq!(segs[0].text, "a b c d e");
    assert_eq!(segs[1].text, "f g h i j");
    assert_eq!(segs[2].text, "k l m n o");
    assert_eq!(segs[3].text, "p");
}

#[test]
fn cap_007_small_gap_within_word_count_stays_together() {
    // If both gap and word count are under threshold, keep as one phrase.
    let words = vec![
        word("a", 0.0, 0.3),
        word("b", 0.31, 0.6),
        word("c", 0.61, 0.9),
    ];
    let config = CaptionConfig {
        words_per_caption: 5,
        max_gap_seconds: 0.5,
        ..Default::default()
    };
    let segs = phrases_from_words(&words, &config, 30);
    assert_eq!(segs.len(), 1);
    assert_eq!(segs[0].text, "a b c");
}

// ===================================================================
// CAP-008: Phrase timing is proportional, respects min display duration
// ===================================================================

#[test]
fn cap_008_timing_proportional_to_word_timestamps() {
    let words = even_words("hello world", 1.0, 3.0);
    let config = CaptionConfig {
        words_per_caption: 10,
        max_gap_seconds: 10.0,
        ..Default::default()
    };
    let segs = phrases_from_words(&words, &config, 30);
    assert_eq!(segs.len(), 1);
    // 1.0 s @ 30 fps → frame 30, 3.0 s → frame 90
    assert_eq!(segs[0].start_frame, 30);
    assert_eq!(segs[0].end_frame, 90);
}

#[test]
fn cap_008_timing_multiple_phrases() {
    let words = vec![
        word("first", 0.0, 0.5),
        word("only", 0.5, 1.0),
        word("late", 5.0, 5.5),
    ];
    let config = CaptionConfig {
        words_per_caption: 10,
        max_gap_seconds: 1.0,
        ..Default::default()
    };
    let segs = phrases_from_words(&words, &config, 30);
    assert_eq!(segs.len(), 2);
    // Group 1: frames 0–30 (0.0–1.0 s)
    assert_eq!(segs[0].start_frame, 0);
    assert_eq!(segs[0].end_frame, 30);
    // Group 2: frames 150–165 (5.0–5.5 s)
    assert_eq!(segs[1].start_frame, 150);
    assert_eq!(segs[1].end_frame, 165);
}

#[test]
fn cap_008_zero_fps_does_not_crash() {
    // Edge case: fps = 0 should be promoted to 1 by the implementation.
    let words = vec![word("only", 0.0, 1.0)];
    let config = CaptionConfig::default();
    let segs = phrases_from_words(&words, &config, 0);
    assert_eq!(segs.len(), 1);
    assert_eq!(segs[0].start_frame, 0);
    assert_eq!(segs[0].end_frame, 1);
}

// ===================================================================
// CAP-010: Generated captions inserted on fresh top video track
// ===================================================================

#[test]
fn cap_010_segments_in_chronological_order() {
    // segments_from_words returns segments in ascending time order.
    let words = vec![
        word("early", 0.0, 0.5),
        word("middle", 2.0, 2.5),
        word("late", 5.0, 5.5),
    ];
    let config = CaptionConfig {
        words_per_caption: 1,
        max_gap_seconds: 0.5,
        ..Default::default()
    };
    let segs = phrases_from_words(&words, &config, 30);
    assert!(segs.len() >= 3);
    for pair in segs.windows(2) {
        assert!(
            pair[0].start_frame <= pair[1].start_frame,
            "segments must be in chronological order"
        );
    }
}

#[test]
fn cap_010_no_overlapping_segments_same_source() {
    // When words are grouped properly, segments should not overlap
    // (a segment's end should be <= the next segment's start).
    let words = even_words("one two three four five six seven eight nine ten", 0.0, 5.0);
    let config = CaptionConfig {
        words_per_caption: 3,
        max_gap_seconds: 0.3,
        ..Default::default()
    };
    let segs = phrases_from_words(&words, &config, 30);
    for pair in segs.windows(2) {
        assert!(
            pair[0].end_frame <= pair[1].start_frame,
            "segments must not overlap: {:?} → {:?}",
            pair[0],
            pair[1]
        );
    }
}

// ===================================================================
// CAP-013: Caption text case modes (auto, upper, lower)
// ===================================================================

#[test]
fn cap_013_auto_preserves_original_case() {
    // In "auto" mode the text is passed through unchanged.
    let words = vec![word("Hello World", 0.0, 1.0)];
    let config = CaptionConfig::default();
    let segs = phrases_from_words(&words, &config, 30);
    assert_eq!(segs[0].text, "Hello World");
}

#[test]
fn cap_013_mixed_case_preserved_in_auto_mode() {
    // Verify that no automatic case transformation is applied.
    let words = vec![
        word("UPPER", 0.0, 0.3),
        word("lower", 0.4, 0.7),
        word("Title", 0.8, 1.0),
    ];
    let config = CaptionConfig {
        words_per_caption: 10,
        max_gap_seconds: 10.0,
        ..Default::default()
    };
    let segs = phrases_from_words(&words, &config, 30);
    assert_eq!(segs[0].text, "UPPER lower Title");
}

// ===================================================================
// Upstream #92: phrase grouping with wordsPerCaption parameter
// ===================================================================

#[test]
fn upstream_092_words_per_caption_one() {
    // words_per_caption = 1 → every word is its own caption.
    let words = vec![
        word("a", 0.0, 0.3),
        word("b", 0.4, 0.7),
        word("c", 0.8, 1.1),
    ];
    let config = CaptionConfig {
        words_per_caption: 1,
        max_gap_seconds: 10.0,
        ..Default::default()
    };
    let segs = phrases_from_words(&words, &config, 30);
    assert_eq!(segs.len(), 3);
    assert_eq!(segs[0].text, "a");
    assert_eq!(segs[1].text, "b");
    assert_eq!(segs[2].text, "c");
}

#[test]
fn upstream_092_words_per_caption_large_value() {
    // A very large words_per_caption groups everything together (gap allowing).
    let words = even_words("a b c d e", 0.0, 2.0);
    let config = CaptionConfig {
        words_per_caption: 999,
        max_gap_seconds: 10.0,
        ..Default::default()
    };
    let segs = phrases_from_words(&words, &config, 30);
    assert_eq!(segs.len(), 1);
    assert_eq!(segs[0].text, "a b c d e");
}

// ===================================================================
// Upstream #92: word-accurate per-word timestamps
// ===================================================================

#[test]
fn upstream_092_word_accurate_frame_mapping() {
    // Each word's position is reflected precisely in frame timing.
    let words = vec![
        word("quick", 0.0, 0.4),
        word("brown", 0.5, 1.0),
        word("fox", 1.1, 1.5),
    ];
    // Tight config: no gap splitting, high word cap → single segment
    let config = CaptionConfig {
        words_per_caption: 10,
        max_gap_seconds: 10.0,
        ..Default::default()
    };
    let segs = phrases_from_words(&words, &config, 60);
    assert_eq!(segs.len(), 1);
    // First word starts at 0.0 s → frame 0
    assert_eq!(segs[0].start_frame, 0);
    // Last word ends at 1.5 s → frame 90
    assert_eq!(segs[0].end_frame, 90);
}

#[test]
fn upstream_092_word_accurate_per_segment_timing() {
    // Each segment timing is bounded by its first-word start and last-word end.
    let words = vec![
        word("first", 1.0, 1.3),
        word("group", 1.4, 1.8),
        word("delayed", 5.0, 5.4),
    ];
    let config = CaptionConfig {
        words_per_caption: 2,
        max_gap_seconds: 0.5,
        ..Default::default()
    };
    let segs = phrases_from_words(&words, &config, 30);
    assert_eq!(segs.len(), 2);
    // Segment 0: first+group (1.0–1.8 s → frames 30–54)
    assert_eq!(segs[0].start_frame, 30);
    assert_eq!(segs[0].end_frame, 54);
    // Segment 1: delayed (5.0–5.4 s → frames 150–162)
    assert_eq!(segs[1].start_frame, 150);
    assert_eq!(segs[1].end_frame, 162);
}

// ===================================================================
// Upstream #92: wordsPerCaption clamped to 1-12 range
// ===================================================================

#[test]
fn upstream_092_words_per_caption_zero_does_not_panic() {
    // Although u32 has no negative, zero is a degenerate value.
    let words = even_words("a b c", 0.0, 1.0);
    let config = CaptionConfig {
        words_per_caption: 0,
        max_gap_seconds: 10.0,
        ..Default::default()
    };
    // With words_per_caption = 0, every word triggers word_count >= config,
    // so each word becomes its own segment.
    let segs = phrases_from_words(&words, &config, 30);
    assert_eq!(segs.len(), 3, "words_per_caption=0 → each word separate");
}

#[test]
fn upstream_092_words_per_caption_single_word_per_caption() {
    // Minimum meaningful value: 1 word per caption.
    let words = even_words("hello world", 0.0, 2.0);
    let config = CaptionConfig {
        words_per_caption: 1,
        ..Default::default()
    };
    let segs = phrases_from_words(&words, &config, 30);
    assert_eq!(segs.len(), 2);
    assert_eq!(segs[0].text, "hello");
    assert_eq!(segs[1].text, "world");
}

// ===================================================================
// Edge cases & additional coverage
// ===================================================================

#[test]
fn cap_caption_segment_duration_frames() {
    let seg = CaptionSegment::new("hello", 100, 150);
    assert_eq!(seg.duration_frames(), 50);
}

#[test]
fn cap_caption_segment_zero_duration() {
    let seg = CaptionSegment::new("now", 50, 50);
    assert_eq!(seg.duration_frames(), 0);
}

#[test]
fn cap_default_config_values() {
    let config = CaptionConfig::default();
    assert_eq!(config.words_per_caption, 6);
    assert!((config.min_duration_seconds - 0.7).abs() < 1e-9);
    assert!((config.max_gap_seconds - 0.7).abs() < 1e-9);
    assert!(config.auto_detect_track);
    assert!(config.target_clip_ids.is_none());
}

#[test]
fn cap_single_word_input() {
    let words = vec![word("only", 0.0, 0.5)];
    let config = CaptionConfig::default();
    let segs = phrases_from_words(&words, &config, 30);
    assert_eq!(segs.len(), 1);
    assert_eq!(segs[0].text, "only");
}

#[test]
fn cap_large_gap_after_first_word() {
    // First word isolated by huge gap → two segments
    let words = vec![
        word("intro", 0.0, 0.5),
        word("body", 10.0, 10.5),
        word("rest", 10.6, 11.0),
    ];
    let config = CaptionConfig {
        words_per_caption: 10,
        max_gap_seconds: 1.0,
        ..Default::default()
    };
    let segs = phrases_from_words(&words, &config, 30);
    assert_eq!(segs.len(), 2);
    assert_eq!(segs[0].text, "intro");
    assert_eq!(segs[1].text, "body rest");
}

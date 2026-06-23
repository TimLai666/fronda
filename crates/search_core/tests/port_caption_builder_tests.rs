//! Ported from PalmierPro Tests/Captions/CaptionBuilderTests.swift
//!
//! Tests caption phrase splitting and spec generation.
//! All times are in seconds.

use search_core::caption::{phrases_from_words, CaptionConfig};
use search_core::transcript::TranscribedWord;

// Helper to create a single-segment transcript from text.
fn segment(text: &str, start: f64, end: f64) -> Vec<TranscribedWord> {
    text.split_whitespace()
        .enumerate()
        .map(|(i, word)| {
            let word_count = text.split_whitespace().count() as f64;
            let word_start = start + (end - start) * (i as f64 / word_count);
            let word_end = word_start + (end - start) / word_count;
            TranscribedWord {
                word: word.to_string(),
                start_seconds: word_start,
                end_seconds: word_end,
            }
        })
        .collect()
}

/// Swift: keepsSegmentWholeWhenItFits
#[test]
fn port_caption_keeps_segment_whole_when_it_fits() {
    let words = segment("Hello there", 1.0, 2.0);
    let config = CaptionConfig {
        words_per_caption: 10,
        min_duration_seconds: 0.0,
        max_gap_seconds: 999.0,
        auto_detect_track: false,
        target_clip_ids: None,
    };
    let phrases = phrases_from_words(&words, &config, 30);
    assert_eq!(phrases.len(), 1);
    assert_eq!(phrases[0].text, "Hello there");
    assert_eq!(phrases[0].start_frame, 30); // 1.0s @ 30fps
    assert_eq!(phrases[0].end_frame, 60); // 2.0s @ 30fps
}

/// Swift: dropsPhraseEntirelyBeforeTrimIn
/// Note: Our port tests the words→phrases layer. Trim handling is done by the caller.
#[test]
fn port_caption_words_divided_by_gap() {
    // Two words with a big gap should become separate phrases
    let words = vec![
        TranscribedWord {
            word: "first".into(),
            start_seconds: 0.0,
            end_seconds: 1.0,
        },
        TranscribedWord {
            word: "second".into(),
            start_seconds: 5.0,
            end_seconds: 6.0,
        },
    ];
    let config = CaptionConfig {
        words_per_caption: 10,
        min_duration_seconds: 0.7,
        max_gap_seconds: 0.7,
        auto_detect_track: false,
        target_clip_ids: None,
    };
    let phrases = phrases_from_words(&words, &config, 30);
    // Gap between 1.0 and 5.0 = 4.0 > 0.7 → split
    assert_eq!(phrases.len(), 2);
    assert_eq!(phrases[0].text, "first");
    assert_eq!(phrases[1].text, "second");
}

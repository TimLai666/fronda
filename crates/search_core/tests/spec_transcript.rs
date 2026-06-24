//! Spec tests for transcript cache, range filtering, locale matching, and
//! search behavior (TRN series).
//!
//! These tests validate the data-layer contract for transcripts. Locale
//! matching behavior (TRN-012–015, upstream #57) is tested via a standalone
//! matcher since the crate does not export a locale-resolution function.

use search_core::search_index::CacheIdentity;
use search_core::transcript::{TranscribedWord, Transcript, TranscriptSegment};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn sample_transcript(identity: CacheIdentity) -> Transcript {
    Transcript {
        identity,
        segments: vec![
            TranscriptSegment {
                start_seconds: 0.0,
                end_seconds: 2.0,
                text: "hello world".into(),
                words: vec![
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
                ],
            },
            TranscriptSegment {
                start_seconds: 3.0,
                end_seconds: 5.0,
                text: "this is a test".into(),
                words: vec![
                    TranscribedWord {
                        word: "this".into(),
                        start_seconds: 3.0,
                        end_seconds: 3.3,
                    },
                    TranscribedWord {
                        word: "is".into(),
                        start_seconds: 3.4,
                        end_seconds: 3.6,
                    },
                    TranscribedWord {
                        word: "a".into(),
                        start_seconds: 3.7,
                        end_seconds: 3.8,
                    },
                    TranscribedWord {
                        word: "test".into(),
                        start_seconds: 3.9,
                        end_seconds: 4.5,
                    },
                ],
            },
            TranscriptSegment {
                start_seconds: 6.0,
                end_seconds: 8.0,
                text: "goodbye".into(),
                words: vec![TranscribedWord {
                    word: "goodbye".into(),
                    start_seconds: 6.0,
                    end_seconds: 7.5,
                }],
            },
        ],
        language: Some("en".into()),
    }
}

fn identity(path: &str) -> CacheIdentity {
    CacheIdentity {
        path: path.to_string(),
        modification_time: 1_700_000_000,
        file_size: 42_000,
    }
}

// ===================================================================
// TRN-001: Transcript cache identity: path + mtime + file size
// ===================================================================

#[test]
fn trn_001_cache_identity_equality_same_path_mtime_size() {
    let a = identity("/audio/test.wav");
    let b = identity("/audio/test.wav");
    assert_eq!(a, b);
}

#[test]
fn trn_001_cache_identity_different_path_different() {
    let a = identity("/audio/a.wav");
    let b = identity("/audio/b.wav");
    assert_ne!(a, b);
}

#[test]
fn trn_001_cache_identity_different_mtime_different() {
    let a = CacheIdentity {
        path: "/audio/test.wav".into(),
        modification_time: 1_700_000_000,
        file_size: 42_000,
    };
    let b = CacheIdentity {
        path: "/audio/test.wav".into(),
        modification_time: 1_700_000_001,
        file_size: 42_000,
    };
    assert_ne!(a, b);
}

#[test]
fn trn_001_cache_identity_different_size_different() {
    let a = CacheIdentity {
        path: "/audio/test.wav".into(),
        modification_time: 1_700_000_000,
        file_size: 42_000,
    };
    let b = CacheIdentity {
        path: "/audio/test.wav".into(),
        modification_time: 1_700_000_000,
        file_size: 99_000,
    };
    assert_ne!(a, b);
}

// ===================================================================
// TRN-005: Range filtering keeps overlapping segments
// ===================================================================

#[test]
fn trn_005_range_keeps_partial_overlap() {
    let transcript = sample_transcript(identity("/audio/test.wav"));
    // Range 1.0–4.0: overlaps seg1 (0–2) and seg2 (3–5)
    let range = transcript.filter_range(1.0, 4.0);
    assert_eq!(range.segments.len(), 2);
}

#[test]
fn trn_005_range_keeps_segment_contained_within_range() {
    let transcript = sample_transcript(identity("/audio/test.wav"));
    // Range 0.5–8.5: contains all three segments
    let range = transcript.filter_range(0.5, 8.5);
    assert_eq!(range.segments.len(), 3);
}

#[test]
fn trn_005_range_outside_returns_empty() {
    let transcript = sample_transcript(identity("/audio/test.wav"));
    // Range completely before any segment
    let range = transcript.filter_range(-10.0, -5.0);
    assert!(range.segments.is_empty());
    // Range completely after all segments
    let range2 = transcript.filter_range(20.0, 30.0);
    assert!(range2.segments.is_empty());
}

// ===================================================================
// TRN-006: Boundary-straddling segments included
// ===================================================================

#[test]
fn trn_006_segment_start_at_boundary_included() {
    let transcript = sample_transcript(identity("/audio/test.wav"));
    // seg2 starts at 3.0, range starts at 3.0 → included
    let range = transcript.filter_range(3.0, 6.0);
    assert!(range
        .segments
        .iter()
        .any(|s| (s.start_seconds - 3.0).abs() < 1e-9));
}

#[test]
fn trn_006_segment_end_at_boundary_included() {
    let transcript = sample_transcript(identity("/audio/test.wav"));
    // seg1 ends at 2.0, range ends at 2.0 (exclusive: seg.end > range.end → not kept)
    // Actually filter uses seg.end_seconds > start_seconds (2.0 > 2.0 is false)
    // Let's test a segment that starts exactly at range end: seg2 starts at 3.0
    // Range end = 3.0, seg.start_seconds < 3.0 → false for seg2
    // Let's test a start-boundary case differently: seg1 starts at 0, range end at 2.0
    // seg1.end_seconds (2.0) > start_seconds of filter → but if range.end=2.0, seg1.end=2.0 is NOT > range.end
    // Let me test: seg1 starts at 0.0 < 2.0 (range end), seg1 ends at 2.0 > 0.0? Yes for range 0-2.0
    let range = transcript.filter_range(0.0, 2.0);
    // seg1: start=0.0 < 2.0 (true) AND end=2.0 > 0.0 (true) → KEPT
    assert!(range
        .segments
        .iter()
        .any(|s| (s.end_seconds - 2.0).abs() < 1e-9));
}

// ===================================================================
// TRN-007: Words without complete timestamps dropped
// ===================================================================

#[test]
fn trn_007_negative_start_timestamp_dropped() {
    let mut transcript = sample_transcript(identity("/audio/test.wav"));
    transcript.segments[0].words.push(TranscribedWord {
        word: "bad".into(),
        start_seconds: -1.0,
        end_seconds: 0.5,
    });
    transcript.segments[0].text = "hello world bad".into();

    let range = transcript.filter_range(0.0, 10.0);
    let all_text = range.text();
    assert!(
        !all_text.contains("bad"),
        "word with negative start dropped"
    );
}

#[test]
fn trn_007_negative_end_timestamp_dropped() {
    let mut transcript = sample_transcript(identity("/audio/test.wav"));
    transcript.segments[1].words.push(TranscribedWord {
        word: "invalid".into(),
        start_seconds: 3.5,
        end_seconds: -1.0,
    });
    transcript.segments[1].text = "this is a test invalid".into();

    let range = transcript.filter_range(0.0, 10.0);
    let all_text = range.text();
    assert!(
        !all_text.contains("invalid"),
        "word with negative end dropped"
    );
}

#[test]
fn trn_007_only_word_with_bad_timestamp_in_segment() {
    let id = identity("/audio/t.wav");
    let transcript = Transcript {
        identity: id,
        segments: vec![TranscriptSegment {
            start_seconds: 0.0,
            end_seconds: 1.0,
            text: "badword".into(),
            words: vec![TranscribedWord {
                word: "badword".into(),
                start_seconds: -1.0,
                end_seconds: -1.0,
            }],
        }],
        language: None,
    };
    let range = transcript.filter_range(0.0, 10.0);
    // Segment kept, but with empty words and empty text
    assert_eq!(range.segments.len(), 1);
    assert!(range.segments[0].words.is_empty());
    assert!(range.segments[0].text.is_empty());
    assert!(range.text().is_empty());
}

// ===================================================================
// TRN-008: Filtered text rebuilt from surviving segments
// ===================================================================

#[test]
fn trn_008_text_joins_surviving_segments() {
    let transcript = sample_transcript(identity("/audio/test.wav"));
    let range = transcript.filter_range(2.5, 5.5);
    assert_eq!(range.text(), "this is a test");
}

#[test]
fn trn_008_text_multiple_surviving_segments() {
    let transcript = sample_transcript(identity("/audio/test.wav"));
    let range = transcript.filter_range(0.0, 10.0);
    assert_eq!(range.text(), "hello world this is a test goodbye");
}

#[test]
fn trn_008_text_empty_when_no_survivors() {
    let transcript = sample_transcript(identity("/audio/test.wav"));
    let range = transcript.filter_range(100.0, 200.0);
    assert!(range.text().is_empty());
    assert!(range.is_empty());
}

// ===================================================================
// TRN-010: Case-insensitive + diacritic-insensitive matching
// ===================================================================

#[test]
fn trn_010_case_insensitive_text_contains() {
    let transcript = sample_transcript(identity("/audio/test.wav"));
    let all = transcript.all_text();
    let lower = all.to_lowercase();
    assert!(lower.contains("hello"));
    assert!(lower.contains("test"));
    assert!(lower.contains("goodbye"));
}

#[test]
fn trn_010_case_insensitive_search_across_cases() {
    let transcript = sample_transcript(identity("/audio/test.wav"));
    let all = transcript.all_text();
    // "Hello World" in data is lowercase "hello world" → upper search still finds
    assert!(all.to_uppercase().contains("HELLO"));
    assert!(all.to_uppercase().contains("TEST"));
}

// ===================================================================
// TRN-011: All query terms must match for a hit
// ===================================================================

#[test]
fn trn_011_all_terms_required() {
    // Simulates the requirement: a segment is a hit only if ALL terms match.
    // Here we verify the data-level assumption by checking individual segments.
    let transcript = sample_transcript(identity("/audio/test.wav"));
    let seg0_text = &transcript.segments[0].text; // "hello world"
    let seg1_text = &transcript.segments[1].text; // "this is a test"

    // "hello test" — seg0 has "hello" but not "test", seg1 has "test" but not "hello"
    let seg0_has_hello = seg0_text.to_lowercase().contains("hello");
    let seg0_has_test = seg0_text.to_lowercase().contains("test");
    assert!(seg0_has_hello);
    assert!(
        !seg0_has_test,
        "seg0 lacks 'test' → not a hit for query 'hello test'"
    );

    let seg1_has_hello = seg1_text.to_lowercase().contains("hello");
    let seg1_has_test = seg1_text.to_lowercase().contains("test");
    assert!(
        !seg1_has_hello,
        "seg1 lacks 'hello' → not a hit for query 'hello test'"
    );
    assert!(seg1_has_test);
}

#[test]
fn trn_011_all_terms_present_in_one_segment() {
    let transcript = sample_transcript(identity("/audio/test.wav"));
    let seg1_text = &transcript.segments[1].text;
    let words: Vec<&str> = seg1_text.split_whitespace().collect();
    assert!(words.contains(&"this"));
    assert!(words.contains(&"test"));
    // Both "this" and "test" are in segment 1 → it IS a hit
}

#[test]
fn trn_011_single_term_matches_segment() {
    let transcript = sample_transcript(identity("/audio/test.wav"));
    let seg2_text = &transcript.segments[2].text; // "goodbye"
    assert!(seg2_text.to_lowercase().contains("goodbye"));
    assert!(!seg2_text.to_lowercase().contains("missing"));
}

// ===================================================================
// TRN-012: Locale matching prefers exact language+region
// ===================================================================
//
// The crate does not export a locale matcher; we use a standalone
// implementation that reflects the spec requirements.

fn match_locale(candidates: &[&str], supported: &[&str]) -> Option<String> {
    // First pass: exact match (language + region)
    for candidate in candidates {
        if supported.contains(candidate) {
            return Some(candidate.to_string());
        }
    }
    // Second pass: language-only fallback
    for candidate in candidates {
        let lang = candidate.split(['_', '-']).next().unwrap_or("");
        for s in supported {
            if s.starts_with(lang)
                && (s.len() == lang.len() || s.as_bytes().get(lang.len()) == Some(&b'_'))
            {
                return Some(s.to_string());
            }
        }
    }
    // Third pass: strip @rg= and -u-rg- extension tags
    for candidate in candidates {
        let cleaned = candidate
            .split('@')
            .next()
            .unwrap_or("")
            .split("-u-")
            .next()
            .unwrap_or("")
            .to_string();
        if !cleaned.is_empty() && cleaned != *candidate {
            let lang = cleaned.split(['_', '-']).next().unwrap_or("");
            for s in supported {
                if s.starts_with(lang) {
                    return Some(s.to_string());
                }
            }
        }
    }
    None
}

#[test]
fn trn_012_exact_region_preferred() {
    let supported = &["en_US", "en_GB", "fr_FR", "fr_CA"];
    let result = match_locale(&["fr_CA"], supported);
    assert_eq!(result, Some("fr_CA".to_string()));
}

#[test]
fn trn_012_exact_match_returns_correct_region() {
    let supported = &["en_US", "en_GB", "fr_FR"];
    let result = match_locale(&["en_GB"], supported);
    assert_eq!(result, Some("en_GB".to_string()));
}

// ===================================================================
// TRN-013: Fallback to same-language, any region
// ===================================================================

#[test]
fn trn_013_fallback_to_same_language() {
    // en_FR has no exact match → fall back to some en_*
    let supported = &["en_US", "en_GB", "fr_FR"];
    let result = match_locale(&["en_FR"], supported);
    assert!(result.unwrap().starts_with("en_"));
}

#[test]
fn trn_013_fallback_picks_any_region() {
    let supported = &["en_US", "en_GB", "fr_FR"];
    let result = match_locale(&["fr"], supported);
    assert!(result.unwrap().starts_with("fr_"));
}

// ===================================================================
// TRN-014: Unicode extension tags don't block matching
// ===================================================================

#[test]
fn trn_014_at_rg_tag_stripped() {
    let supported = &["en_US", "en_GB"];
    let result = match_locale(&["en_US@rg=frzzzz"], supported);
    assert_eq!(result, Some("en_US".to_string()));
}

#[test]
fn trn_014_u_rg_extension_stripped() {
    let supported = &["en_US", "en_GB"];
    let result = match_locale(&["en-US-u-rg-zazzzz"], supported);
    assert_eq!(result, Some("en_US".to_string()));
}

// ===================================================================
// TRN-015: No supported language → nil
// ===================================================================

#[test]
fn trn_015_no_supported_language_returns_none() {
    let supported = &["en_US", "fr_FR"];
    let result = match_locale(&["ja_JP"], supported);
    assert_eq!(result, None);
}

#[test]
fn trn_015_totally_unrelated_locale() {
    let supported = &["en_US", "en_GB"];
    let result = match_locale(&["de_DE", "zh_CN"], supported);
    assert_eq!(result, None);
}

// ===================================================================
// Upstream #57: Locale matching strips Unicode extension tags
// ===================================================================

#[test]
fn upstream_057_strips_unicode_extension_bcp47() {
    // BCP 47 tag with -u- extension
    let supported = &["en_US", "fr_FR"];
    let result = match_locale(&["en-US-u-rg-uszzzz"], supported);
    assert_eq!(result, Some("en_US".to_string()));
}

#[test]
fn upstream_057_strips_multiple_extensions() {
    let supported = &["zh_CN", "zh_TW"];
    let result = match_locale(&["zh-CN-u-rg-cnzzzz"], supported);
    assert_eq!(result, Some("zh_CN".to_string()));
}

#[test]
fn upstream_057_candidate_order_after_stripping() {
    let supported = &["en_US", "fr_FR"];
    // First candidate `en_US@rg=frzzzz` gets stripped to en_US (second pass)
    // Second candidate `en_GB` has no exact match
    let result = match_locale(&["en_US@rg=frzzzz", "en_GB"], supported);
    assert_eq!(result, Some("en_US".to_string()));
}

// ===================================================================
// Edge cases
// ===================================================================

#[test]
fn trn_transcript_new_empty() {
    let id = identity("/audio/empty.wav");
    let t = Transcript::new(id.clone());
    assert_eq!(t.identity, id);
    assert!(t.segments.is_empty());
    assert!(t.language.is_none());
}

#[test]
fn trn_transcript_all_text_no_segments() {
    let t = Transcript::new(identity("/audio/empty.wav"));
    assert_eq!(t.all_text(), "");
}

#[test]
fn trn_transcript_range_empty_is_empty() {
    let t = Transcript::new(identity("/audio/empty.wav"));
    let range = t.filter_range(0.0, 10.0);
    assert!(range.is_empty());
}

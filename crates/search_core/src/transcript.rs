use serde::{Deserialize, Serialize};

use crate::search_index::CacheIdentity;

/// A transcribed word with timing.
/// TRN-007: words without valid timestamps should be dropped.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TranscribedWord {
    pub word: String,
    pub start_seconds: f64,
    pub end_seconds: f64,
}

impl TranscribedWord {
    /// TRN-007: both start and end must be >= 0.
    pub fn has_valid_timestamps(&self) -> bool {
        self.start_seconds >= 0.0 && self.end_seconds >= 0.0
    }
}

/// A transcript segment containing contiguous words.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TranscriptSegment {
    pub start_seconds: f64,
    pub end_seconds: f64,
    pub text: String,
    pub words: Vec<TranscribedWord>,
}

/// Full transcript for one media asset.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Transcript {
    pub identity: CacheIdentity,
    pub segments: Vec<TranscriptSegment>,
    pub language: Option<String>,
}

impl Transcript {
    pub fn new(identity: CacheIdentity) -> Self {
        Self {
            identity,
            segments: Vec::new(),
            language: None,
        }
    }

    /// Concatenated text from all segments.
    pub fn all_text(&self) -> String {
        self.segments
            .iter()
            .map(|s| s.text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// TRN-005: keeps segments whose time spans overlap the requested range.
    /// TRN-006: boundary-straddling segments remain included.
    /// TRN-007: words without complete timestamps are dropped from the output.
    pub fn filter_range(&self, start_seconds: f64, end_seconds: f64) -> TranscriptRange {
        let filtered: Vec<TranscriptSegment> = self
            .segments
            .iter()
            .filter(|seg| seg.start_seconds < end_seconds && seg.end_seconds > start_seconds)
            .map(|seg| {
                let valid_words: Vec<TranscribedWord> = seg
                    .words
                    .iter()
                    .filter(|w| w.has_valid_timestamps())
                    .cloned()
                    .collect();
                TranscriptSegment {
                    start_seconds: seg.start_seconds,
                    end_seconds: seg.end_seconds,
                    text: valid_words
                        .iter()
                        .map(|w| w.word.as_str())
                        .collect::<Vec<_>>()
                        .join(" "),
                    words: valid_words,
                }
            })
            .collect();

        TranscriptRange {
            segments: filtered,
            original_start_seconds: start_seconds,
            original_end_seconds: end_seconds,
        }
    }
}

/// Filtered transcript for a time range.
/// TRN-003: range-limited requests reuse full-file cache.
#[derive(Debug, Clone, PartialEq)]
pub struct TranscriptRange {
    pub segments: Vec<TranscriptSegment>,
    pub original_start_seconds: f64,
    pub original_end_seconds: f64,
}

impl TranscriptRange {
    /// TRN-008: text rebuilt from surviving segments.
    pub fn text(&self) -> String {
        self.segments
            .iter()
            .map(|s| s.text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search_index::CacheIdentity;

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

    #[test]
    fn trn_003_range_reuses_full_cache() {
        let identity = CacheIdentity::from_path("/audio/test.wav");
        let transcript = sample_transcript(identity);
        let range = transcript.filter_range(2.5, 5.5);
        assert_eq!(range.segments.len(), 1);
        assert_eq!(range.segments[0].text, "this is a test");
    }

    #[test]
    fn trn_005_range_filtering_keeps_overlapping() {
        let identity = CacheIdentity::from_path("/audio/test.wav");
        let transcript = sample_transcript(identity);
        // Range 1.0–4.0 overlaps segment 1 (0–2) and segment 2 (3–5)
        let range = transcript.filter_range(1.0, 4.0);
        assert_eq!(range.segments.len(), 2);
    }

    #[test]
    fn trn_006_boundary_straddling_included() {
        let identity = CacheIdentity::from_path("/audio/test.wav");
        let transcript = sample_transcript(identity);
        // Range 4.0–6.5: segment 2 (3–5) overlaps, segment 3 (6–8) overlaps
        let range = transcript.filter_range(4.0, 6.5);
        assert_eq!(range.segments.len(), 2);
        // Segment 2 ends at 5.0, which is > 4.0
        // Segment 3 starts at 6.0, which is < 6.5
        assert_eq!(range.segments[0].text, "this is a test");
        assert_eq!(range.segments[1].text, "goodbye");
    }

    #[test]
    fn trn_007_words_without_timestamps_dropped() {
        let identity = CacheIdentity::from_path("/audio/test.wav");
        let mut transcript = sample_transcript(identity);
        // Add a word with missing end timestamp
        transcript.segments[0].words.push(TranscribedWord {
            word: "bad".into(),
            start_seconds: 2.0,
            end_seconds: -1.0,
        });
        transcript.segments[0].text = "hello world bad".into();

        let range = transcript.filter_range(0.0, 10.0);
        let word_count: usize = range.segments.iter().map(|s| s.words.len()).sum();
        // The bad word should be dropped
        assert_eq!(word_count, 7); // 2 from seg1 + 4 from seg2 + 1 from seg3
        assert!(!range.text().contains("bad"));
    }

    #[test]
    fn trn_008_text_rebuilt_from_surviving() {
        let identity = CacheIdentity::from_path("/audio/test.wav");
        let transcript = sample_transcript(identity);
        let range = transcript.filter_range(3.0, 5.0);
        assert_eq!(range.text(), "this is a test");
    }

    #[test]
    fn trn_010_case_insensitive_search() {
        let identity = CacheIdentity::from_path("/audio/test.wav");
        let transcript = sample_transcript(identity);
        let all = transcript.all_text().to_lowercase();
        assert!(all.contains("hello"));
        assert!(all.contains("test"));
        assert!(!all.contains("missing"));
    }
}

use serde::{Deserialize, Serialize};
use std::fmt;

use crate::search_index::CacheIdentity;

// ── Transcription errors (TRN-016, TRN-017, TRN-019) ─────────────────────────

/// Error variants for the video transcription pipeline.
///
/// Covers the three failure modes the platform adapter must map to:
/// - TRN-016: audio extraction failure (`.caf` temp write)
/// - TRN-017: video with no audio track
/// - TRN-019: on-device speech model installation failure
#[derive(Debug, Clone, PartialEq)]
pub enum TranscriptionError {
    /// TRN-017: The video source has no extractable audio track.
    NoAudioTrack { source: String },
    /// TRN-016: Extracting audio from the video source to a temp PCM file failed.
    AudioExtractionFailed { reason: String },
    /// TRN-019: The on-device speech model could not be installed.
    ModelInstallFailed { reason: String },
    /// The transcription result could not be decoded.
    DecodeFailed,
    /// Transcription analysis failed for another reason.
    AnalysisFailed { reason: String },
}

impl fmt::Display for TranscriptionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TranscriptionError::NoAudioTrack { source } => {
                write!(f, "No audio track in {source}")
            }
            TranscriptionError::AudioExtractionFailed { reason } => {
                write!(f, "Audio extraction failed: {reason}")
            }
            TranscriptionError::ModelInstallFailed { reason } => {
                write!(f, "Could not install the on-device speech model: {reason}")
            }
            TranscriptionError::DecodeFailed => {
                write!(f, "Could not parse transcription result")
            }
            TranscriptionError::AnalysisFailed { reason } => {
                write!(f, "Transcription analysis failed: {reason}")
            }
        }
    }
}

/// Audio extraction configuration for the temp PCM `.caf` file.
///
/// TRN-016: Video transcription first extracts audio to a temp PCM `.caf`
/// file using this sample-rate/channel/bit-depth contract before passing
/// the audio to the on-device speech analyzer.
#[derive(Debug, Clone, PartialEq)]
pub struct AudioExtractionConfig {
    /// Output file path (should be a `.caf` extension in a temp directory).
    pub output_path: String,
    /// The nominal sample rate is preserved from the source asset's audio track
    /// rather than resampled, matching the Swift `AVAudioFile` behavior.
    pub preserve_source_format: bool,
}

impl AudioExtractionConfig {
    /// Create a config for a temp `.caf` file in the given directory.
    ///
    /// `dir` should be a writable temp directory (e.g. `NSTemporaryDirectory()`).
    /// A UUID-based filename is used to avoid collisions.
    pub fn new_temp(dir: &str, uuid: &str) -> Self {
        let output_path = format!("{dir}/palmier-stt-{uuid}.caf");
        Self {
            output_path,
            preserve_source_format: true,
        }
    }

    /// Whether the output path ends with `.caf`, as required by the contract.
    pub fn is_caf(&self) -> bool {
        self.output_path.ends_with(".caf")
    }
}

/// Request to transcribe a video file.
///
/// Bundles all parameters needed by the platform adapter to:
/// 1. Check for an audio track (TRN-017)
/// 2. Extract audio to a temp `.caf` (TRN-016)
/// 3. Run on-device speech model, installing it first if needed (TRN-019)
#[derive(Debug, Clone, PartialEq)]
pub struct VideoTranscriptionRequest {
    /// Path to the source video file.
    pub source_path: String,
    /// BCP-47 locale string (e.g. `"en-US"`). `None` auto-detects.
    pub locale: Option<String>,
    /// Optional time range (start, end) in seconds for range-limited transcription.
    pub range_seconds: Option<(f64, f64)>,
    /// Audio extraction config (temp `.caf` destination).
    pub audio_config: AudioExtractionConfig,
}

impl VideoTranscriptionRequest {
    /// Returns true if the request is for the full file (no range limit).
    pub fn is_full_file(&self) -> bool {
        self.range_seconds.is_none()
    }
}

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
    /// TRN-002: Only full-file transcripts are cached on disk.
    /// This flag is `true` for a complete-file transcript and `false`
    /// for partial/range-limited results that should not be persisted.
    #[serde(default = "default_true")]
    pub is_full_file: bool,
}

fn default_true() -> bool {
    true
}

impl Transcript {
    pub fn new(identity: CacheIdentity) -> Self {
        Self {
            identity,
            segments: Vec::new(),
            language: None,
            is_full_file: true,
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
    /// TRN-004: The returned TranscriptRange carries `is_partial: true`.
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
            is_partial: true,
        }
    }

    /// Search for query terms within this transcript (TRN-009, TRN-011).
    ///
    /// Only operates on full-file transcripts (TRN-009). Returns segments where
    /// ALL query terms match within a single segment (TRN-011). Matching is
    /// case-insensitive (TRN-010).
    ///
    /// Returns `None` if the transcript is not a full-file transcript.
    pub fn keyword_search(&self, query: &str) -> Option<Vec<&TranscriptSegment>> {
        if !self.is_full_file {
            return None;
        }

        let terms: Vec<String> = query.split_whitespace().map(|t| t.to_lowercase()).collect();

        if terms.is_empty() {
            return Some(Vec::new());
        }

        let hits: Vec<&TranscriptSegment> = self
            .segments
            .iter()
            .filter(|seg| {
                let seg_lower = seg.text.to_lowercase();
                terms.iter().all(|term| seg_lower.contains(term.as_str()))
            })
            .collect();

        Some(hits)
    }
}

/// Filtered transcript for a time range.
/// TRN-003: range-limited requests reuse full-file cache.
#[derive(Debug, Clone, PartialEq)]
pub struct TranscriptRange {
    pub segments: Vec<TranscriptSegment>,
    pub original_start_seconds: f64,
    pub original_end_seconds: f64,
    /// TRN-004: `true` when this result came from a range-limited request.
    /// Partial results must not overwrite the canonical full-file cache.
    pub is_partial: bool,
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

/// TRN-018: Offset a TranscriptRange's timestamps back to original source time.
///
/// When transcription is done on a sub-range of the source, the transcription
/// service returns timestamps relative to that sub-range (starting at 0).
/// This function shifts all timestamps by `offset_seconds` so they align with
/// the original source timeline.
///
/// Returns `None` if the offset is negative (invalid).
pub fn offset_transcript_range(
    range: &TranscriptRange,
    offset_seconds: f64,
) -> Option<TranscriptRange> {
    if offset_seconds < 0.0 {
        return None;
    }

    let segments: Vec<TranscriptSegment> = range
        .segments
        .iter()
        .map(|seg| {
            let words: Vec<TranscribedWord> = seg
                .words
                .iter()
                .map(|w| TranscribedWord {
                    word: w.word.clone(),
                    start_seconds: w.start_seconds + offset_seconds,
                    end_seconds: w.end_seconds + offset_seconds,
                })
                .collect();

            TranscriptSegment {
                start_seconds: seg.start_seconds + offset_seconds,
                end_seconds: seg.end_seconds + offset_seconds,
                text: seg.text.clone(),
                words,
            }
        })
        .collect();

    Some(TranscriptRange {
        segments,
        original_start_seconds: range.original_start_seconds,
        original_end_seconds: range.original_end_seconds,
        is_partial: range.is_partial,
    })
}

/// TRN-018: Offset a single Transcript's timestamps.
///
/// Used when the entire transcript was produced from a sub-range extraction
/// and needs to be realigned with the original source timeline.
pub fn offset_transcript(transcript: &Transcript, offset_seconds: f64) -> Option<Transcript> {
    if offset_seconds < 0.0 {
        return None;
    }

    let segments: Vec<TranscriptSegment> = transcript
        .segments
        .iter()
        .map(|seg| {
            let words: Vec<TranscribedWord> = seg
                .words
                .iter()
                .map(|w| TranscribedWord {
                    word: w.word.clone(),
                    start_seconds: w.start_seconds + offset_seconds,
                    end_seconds: w.end_seconds + offset_seconds,
                })
                .collect();

            TranscriptSegment {
                start_seconds: seg.start_seconds + offset_seconds,
                end_seconds: seg.end_seconds + offset_seconds,
                text: seg.text.clone(),
                words,
            }
        })
        .collect();

    Some(Transcript {
        identity: transcript.identity.clone(),
        segments,
        language: transcript.language.clone(),
        is_full_file: transcript.is_full_file,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search_index::CacheIdentity;

    fn sample_transcript(identity: CacheIdentity) -> Transcript {
        Transcript {
            identity,
            is_full_file: true,
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

    // -----------------------------------------------------------------------
    // TRN-002: Only full-file transcripts cached on disk
    // -----------------------------------------------------------------------

    #[test]
    fn trn_002_new_transcript_is_full_file() {
        let identity = CacheIdentity::from_path("/audio/test.wav");
        let transcript = Transcript::new(identity);
        assert!(
            transcript.is_full_file,
            "TRN-002: new transcript is full-file by default"
        );
    }

    #[test]
    fn trn_002_partial_transcript_not_full_file() {
        let identity = CacheIdentity::from_path("/audio/test.wav");
        let mut transcript = sample_transcript(identity);
        transcript.is_full_file = false;
        assert!(
            !transcript.is_full_file,
            "TRN-002: partial transcript not full-file"
        );
    }

    // -----------------------------------------------------------------------
    // TRN-004: Range-limited requests do not overwrite full-file cache
    // -----------------------------------------------------------------------

    #[test]
    fn trn_004_range_result_is_partial() {
        let identity = CacheIdentity::from_path("/audio/test.wav");
        let transcript = sample_transcript(identity);
        let range = transcript.filter_range(3.0, 5.0);
        assert!(range.is_partial, "TRN-004: range result is partial");
    }

    #[test]
    fn trn_004_range_result_segments_correct() {
        let identity = CacheIdentity::from_path("/audio/test.wav");
        let transcript = sample_transcript(identity);
        let range = transcript.filter_range(0.0, 10.0);
        // Even a full-coverage range request is still `is_partial` because it
        // went through filter_range (a range-limited operation).
        assert!(
            range.is_partial,
            "TRN-004: filter_range always sets is_partial"
        );
        assert_eq!(range.segments.len(), 3);
    }

    // -----------------------------------------------------------------------
    // TRN-009: Keyword search operates over cached-on-disk (full-file) transcripts
    // -----------------------------------------------------------------------

    #[test]
    fn trn_009_keyword_search_returns_none_for_partial() {
        let identity = CacheIdentity::from_path("/audio/test.wav");
        let mut transcript = sample_transcript(identity);
        transcript.is_full_file = false;
        let result = transcript.keyword_search("hello");
        assert_eq!(result, None, "TRN-009: partial transcript returns None");
    }

    #[test]
    fn trn_009_keyword_search_works_for_full_file() {
        let identity = CacheIdentity::from_path("/audio/test.wav");
        let transcript = sample_transcript(identity);
        let result = transcript.keyword_search("hello");
        assert!(
            result.is_some(),
            "TRN-009: full-file transcript returns results"
        );
        let hits = result.unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].text, "hello world");
    }

    #[test]
    fn trn_009_empty_query_returns_empty_list() {
        let identity = CacheIdentity::from_path("/audio/test.wav");
        let transcript = sample_transcript(identity);
        let result = transcript.keyword_search("");
        assert!(result.is_some(), "empty query returns empty list");
        assert!(result.unwrap().is_empty());
    }

    // -----------------------------------------------------------------------
    // TRN-011: All query terms must match within a single segment
    // -----------------------------------------------------------------------

    #[test]
    fn trn_011_all_terms_must_match_same_segment() {
        let identity = CacheIdentity::from_path("/audio/test.wav");
        let transcript = sample_transcript(identity);
        // "hello" and "world" are both in segment 0
        let result = transcript.keyword_search("hello world");
        let hits = result.unwrap();
        assert_eq!(hits.len(), 1, "TRN-011: both terms in same segment");
        assert_eq!(hits[0].text, "hello world");
    }

    #[test]
    fn trn_011_terms_across_segments_no_hit() {
        let identity = CacheIdentity::from_path("/audio/test.wav");
        let transcript = sample_transcript(identity);
        // "hello" is in segment 0, "test" is in segment 1 — no segment has both
        let result = transcript.keyword_search("hello test");
        let hits = result.unwrap();
        assert_eq!(hits.len(), 0, "TRN-011: terms across segments = no hit");
    }

    #[test]
    fn trn_011_missing_word_no_hit() {
        let identity = CacheIdentity::from_path("/audio/test.wav");
        let transcript = sample_transcript(identity);
        let result = transcript.keyword_search("hello nonexistent");
        let hits = result.unwrap();
        assert_eq!(hits.len(), 0, "TRN-011: one term missing = no hit");
    }

    #[test]
    fn trn_011_single_word_matches_correct_segment() {
        let identity = CacheIdentity::from_path("/audio/test.wav");
        let transcript = sample_transcript(identity);
        let result = transcript.keyword_search("goodbye");
        let hits = result.unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].text, "goodbye");
    }

    // ── TRN-018: Timestamp offset ────────────────────────────────

    #[test]
    fn trn_018_offset_range_shifts_timestamps() {
        let range = TranscriptRange {
            segments: vec![TranscriptSegment {
                start_seconds: 0.0,
                end_seconds: 5.0,
                text: "hello world".into(),
                words: vec![
                    TranscribedWord {
                        word: "hello".into(),
                        start_seconds: 0.0,
                        end_seconds: 1.0,
                    },
                    TranscribedWord {
                        word: "world".into(),
                        start_seconds: 1.5,
                        end_seconds: 2.5,
                    },
                ],
            }],
            original_start_seconds: 30.0,
            original_end_seconds: 35.0,
            is_partial: true,
        };

        let offset = offset_transcript_range(&range, 30.0).unwrap();
        assert!((offset.segments[0].start_seconds - 30.0).abs() < f64::EPSILON);
        assert!((offset.segments[0].end_seconds - 35.0).abs() < f64::EPSILON);
        assert!((offset.segments[0].words[0].start_seconds - 30.0).abs() < f64::EPSILON);
        assert!((offset.segments[0].words[1].start_seconds - 31.5).abs() < f64::EPSILON);
        assert_eq!(offset.original_start_seconds, 30.0); // preserved
    }

    #[test]
    fn trn_018_offset_range_negative_offset_returns_none() {
        let range = TranscriptRange {
            segments: vec![],
            original_start_seconds: 10.0,
            original_end_seconds: 20.0,
            is_partial: true,
        };
        assert!(offset_transcript_range(&range, -5.0).is_none());
    }

    #[test]
    fn trn_018_offset_range_zero_offset_preserves() {
        let range = TranscriptRange {
            segments: vec![TranscriptSegment {
                start_seconds: 0.0,
                end_seconds: 10.0,
                text: "test".into(),
                words: vec![TranscribedWord {
                    word: "test".into(),
                    start_seconds: 0.0,
                    end_seconds: 2.0,
                }],
            }],
            original_start_seconds: 0.0,
            original_end_seconds: 10.0,
            is_partial: true,
        };
        let offset = offset_transcript_range(&range, 0.0).unwrap();
        assert!((offset.segments[0].start_seconds - 0.0).abs() < f64::EPSILON);
        assert!((offset.segments[0].words[0].start_seconds - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn trn_018_offset_range_empty_segments() {
        let range = TranscriptRange {
            segments: vec![],
            original_start_seconds: 60.0,
            original_end_seconds: 90.0,
            is_partial: true,
        };
        let offset = offset_transcript_range(&range, 60.0).unwrap();
        assert!(offset.segments.is_empty());
        assert_eq!(offset.original_start_seconds, 60.0);
        assert_eq!(offset.original_end_seconds, 90.0);
        assert!(offset.is_partial);
    }

    #[test]
    fn trn_018_offset_transcript_shifts_timestamps() {
        let identity = CacheIdentity {
            path: "/test/audio.wav".into(),
            modification_time: 100,
            file_size: 500,
        };
        let transcript = Transcript {
            identity: identity.clone(),
            segments: vec![TranscriptSegment {
                start_seconds: 0.0,
                end_seconds: 3.0,
                text: "hello from range".into(),
                words: vec![TranscribedWord {
                    word: "hello".into(),
                    start_seconds: 0.0,
                    end_seconds: 1.0,
                }],
            }],
            language: Some("en".into()),
            is_full_file: false,
        };
        let offset = offset_transcript(&transcript, 45.0).unwrap();
        assert!((offset.segments[0].start_seconds - 45.0).abs() < f64::EPSILON);
        assert!((offset.segments[0].end_seconds - 48.0).abs() < f64::EPSILON);
        assert!((offset.segments[0].words[0].start_seconds - 45.0).abs() < f64::EPSILON);
        assert_eq!(offset.identity, identity);
        assert_eq!(offset.language, Some("en".into()));
        assert!(!offset.is_full_file); // preserved
    }

    #[test]
    fn trn_018_offset_transcript_negative_offset_returns_none() {
        let identity = CacheIdentity {
            path: "/x.wav".into(),
            modification_time: 0,
            file_size: 0,
        };
        let transcript = Transcript::new(identity);
        assert!(offset_transcript(&transcript, -1.0).is_none());
    }

    // ── TRN-016/017/019 error contract tests ─────────────────────────────────

    #[test]
    fn trn_017_no_audio_track_error_display() {
        let err = TranscriptionError::NoAudioTrack {
            source: "clip.mp4".into(),
        };
        let msg = err.to_string();
        assert!(msg.contains("clip.mp4"), "msg={msg}");
        assert!(msg.contains("No audio"), "msg={msg}");
    }

    #[test]
    fn trn_016_audio_extraction_failed_display() {
        let err = TranscriptionError::AudioExtractionFailed {
            reason: "reader could not start".into(),
        };
        let msg = err.to_string();
        assert!(msg.contains("reader could not start"), "msg={msg}");
        assert!(msg.contains("Audio extraction"), "msg={msg}");
    }

    #[test]
    fn trn_019_model_install_failed_display() {
        let err = TranscriptionError::ModelInstallFailed {
            reason: "no disk space".into(),
        };
        let msg = err.to_string();
        assert!(msg.contains("no disk space"), "msg={msg}");
        assert!(msg.contains("speech model"), "msg={msg}");
    }

    #[test]
    fn trn_016_audio_extraction_config_caf_extension() {
        let cfg = AudioExtractionConfig::new_temp("/tmp", "abc123");
        assert!(
            cfg.is_caf(),
            "output must be .caf, got: {}",
            cfg.output_path
        );
        assert!(cfg.output_path.contains("abc123"));
        assert!(cfg.preserve_source_format);
    }

    #[test]
    fn trn_016_audio_extraction_config_path_construction() {
        let cfg = AudioExtractionConfig::new_temp("/var/folders/tmp", "uuid-xyz");
        assert_eq!(cfg.output_path, "/var/folders/tmp/palmier-stt-uuid-xyz.caf");
    }

    #[test]
    fn trn_016_video_transcription_request_is_full_file() {
        let cfg = AudioExtractionConfig::new_temp("/tmp", "u1");
        let req = VideoTranscriptionRequest {
            source_path: "/video/clip.mp4".into(),
            locale: Some("en-US".into()),
            range_seconds: None,
            audio_config: cfg,
        };
        assert!(req.is_full_file());
    }

    #[test]
    fn trn_016_video_transcription_request_range_not_full_file() {
        let cfg = AudioExtractionConfig::new_temp("/tmp", "u2");
        let req = VideoTranscriptionRequest {
            source_path: "/video/clip.mp4".into(),
            locale: None,
            range_seconds: Some((10.0, 30.0)),
            audio_config: cfg,
        };
        assert!(!req.is_full_file());
    }

    #[test]
    fn transcription_error_variants_are_distinct() {
        let e1 = TranscriptionError::NoAudioTrack { source: "a".into() };
        let e2 = TranscriptionError::AudioExtractionFailed { reason: "b".into() };
        let e3 = TranscriptionError::ModelInstallFailed { reason: "c".into() };
        let e4 = TranscriptionError::DecodeFailed;
        let e5 = TranscriptionError::AnalysisFailed { reason: "d".into() };
        assert_ne!(e1, e2);
        assert_ne!(e2, e3);
        assert_ne!(e3, e4);
        assert_ne!(e4, e5);
    }
}

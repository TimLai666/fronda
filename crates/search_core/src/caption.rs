use serde::{Deserialize, Serialize};

use crate::transcript::TranscribedWord;

/// Configuration for caption generation.
/// CAP-004: auto-detect chooses dominant spoken track.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CaptionConfig {
    pub words_per_caption: u32,
    pub min_duration_seconds: f64,
    pub max_gap_seconds: f64,
    pub auto_detect_track: bool,
    pub target_clip_ids: Option<Vec<String>>,
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
}

/// Result of caption generation planning.
#[derive(Debug, Clone, PartialEq)]
pub struct CaptionPlan {
    pub segments: Vec<CaptionSegment>,
    pub source_media_id: Option<String>,
}

/// Groups words into caption segments using real pause gaps.
///
/// This is from upstream #92: word-accurate per-word timestamps grouping.
///
/// Algorithm: iterate words, group until word count reaches `words_per_caption`
/// OR the gap between words exceeds `max_gap` (converted to seconds).
/// Each group becomes a `CaptionSegment` with frame-based timing derived
/// from word timestamps.
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

    segments
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcript::TranscribedWord;

    #[test]
    fn cap_001_caption_config_default() {
        let config = CaptionConfig::default();
        assert_eq!(config.words_per_caption, 6);
        assert!((config.min_duration_seconds - 0.7).abs() < 1e-9);
        assert!((config.max_gap_seconds - 0.7).abs() < 1e-9);
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
        // All words fit in one group (gaps are small, well under max_gap)
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
            // Big gap here — 2.0 seconds
            TranscribedWord {
                word: "far".into(),
                start_seconds: 3.2,
                end_seconds: 3.8,
            },
        ];
        let config = CaptionConfig {
            words_per_caption: 10,
            max_gap_seconds: 1.0, // gap of 2.0 > 1.0, so split
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
            words_per_caption: 2,  // max 2 words per caption
            max_gap_seconds: 10.0, // gaps won't trigger splits
            ..Default::default()
        };
        let segs = phrases_from_words(&words, &config, 30);
        assert_eq!(segs.len(), 3); // 5 words / 2 per group = 3 groups (2+2+1)
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

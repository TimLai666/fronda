//! Text animation data model (upstream #225).
//!
//! The renderer (Swift `TextAnimator` / `TextFrameRenderer`) is UI and stays
//! deferred; this is the portable data model + the agent-facing preset surface, so
//! projects carrying text animation round-trip through Rust without losing the data.

use serde::{Deserialize, Serialize};

use crate::timeline::TextRgba;

/// One word's timing within a text clip. Frames are project frames.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WordTiming {
    pub text: String,
    pub start_frame: i64,
    pub end_frame: i64,
}

/// How a preset drives its animation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAnimationRenderMode {
    /// Whole-clip / per-line entrance.
    Entrance,
    /// Per-word reveal / highlight.
    PerWord,
    /// Character-by-character typewriter.
    Typewriter,
}

/// A text-animation preset. Raw (serde) names match Swift's `Codable` rawValues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TextAnimationPreset {
    #[default]
    None,
    // Whole-clip / per-line.
    FadeIn,
    PopIn,
    SlideUp,
    Typewriter,
    // Per word.
    WordReveal,
    WordSlide,
    WordPop,
    WordCycle,
    HighlightPop,
    HighlightBlock,
}

impl TextAnimationPreset {
    /// Raw serde names except `none`, in declaration order — the per-word/per-line
    /// presets an agent may request.
    pub const AGENT_PRESETS: [&'static str; 10] = [
        "fadeIn",
        "popIn",
        "slideUp",
        "typewriter",
        "wordReveal",
        "wordSlide",
        "wordPop",
        "wordCycle",
        "highlightPop",
        "highlightBlock",
    ];

    pub fn render_mode(self) -> TextAnimationRenderMode {
        use TextAnimationPreset::*;
        match self {
            None | FadeIn | PopIn | SlideUp => TextAnimationRenderMode::Entrance,
            Typewriter => TextAnimationRenderMode::Typewriter,
            WordReveal | WordSlide | WordPop | WordCycle | HighlightPop | HighlightBlock => {
                TextAnimationRenderMode::PerWord
            }
        }
    }

    pub fn is_per_word(self) -> bool {
        self.render_mode() == TextAnimationRenderMode::PerWord
    }

    /// Per-word presets carry a highlight colour.
    pub fn uses_highlight(self) -> bool {
        self.is_per_word()
    }

    /// Agent-facing values: `"off"` plus every preset except `none`.
    pub fn agent_values() -> Vec<&'static str> {
        let mut v = vec!["off"];
        v.extend_from_slice(&Self::AGENT_PRESETS);
        v
    }

    /// Parse an agent preset string (`"off"` and `"none"` → `None`), or `None` for
    /// an unrecognised value.
    pub fn from_agent_str(s: &str) -> Option<Self> {
        use TextAnimationPreset::*;
        Some(match s {
            "off" | "none" => None,
            "fadeIn" => FadeIn,
            "popIn" => PopIn,
            "slideUp" => SlideUp,
            "typewriter" => Typewriter,
            "wordReveal" => WordReveal,
            "wordSlide" => WordSlide,
            "wordPop" => WordPop,
            "wordCycle" => WordCycle,
            "highlightPop" => HighlightPop,
            "highlightBlock" => HighlightBlock,
            _ => return Option::None,
        })
    }
}

fn default_per_word_frames() -> i64 {
    6
}

/// Per-clip text animation settings (upstream #225).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextAnimation {
    #[serde(default)]
    pub preset: TextAnimationPreset,
    #[serde(default = "default_per_word_frames")]
    pub per_word_frames: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub highlight: Option<TextRgba>,
}

impl Default for TextAnimation {
    fn default() -> Self {
        Self {
            preset: TextAnimationPreset::None,
            per_word_frames: default_per_word_frames(),
            highlight: None,
        }
    }
}

impl TextAnimation {
    pub fn is_active(&self) -> bool {
        self.preset != TextAnimationPreset::None
    }

    /// The Swift default highlight (warm yellow), used when a per-word preset is set
    /// without an explicit highlight colour.
    pub fn default_highlight() -> TextRgba {
        TextRgba {
            r: 1.0,
            g: 0.85,
            b: 0.0,
            a: 1.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_serde_names_match_swift_rawvalues() {
        assert_eq!(
            serde_json::to_string(&TextAnimationPreset::WordReveal).unwrap(),
            "\"wordReveal\""
        );
        assert_eq!(
            serde_json::to_string(&TextAnimationPreset::HighlightBlock).unwrap(),
            "\"highlightBlock\""
        );
        let p: TextAnimationPreset = serde_json::from_str("\"popIn\"").unwrap();
        assert_eq!(p, TextAnimationPreset::PopIn);
    }

    #[test]
    fn render_mode_and_highlight_classification() {
        assert_eq!(
            TextAnimationPreset::FadeIn.render_mode(),
            TextAnimationRenderMode::Entrance
        );
        assert_eq!(
            TextAnimationPreset::Typewriter.render_mode(),
            TextAnimationRenderMode::Typewriter
        );
        assert!(TextAnimationPreset::WordPop.is_per_word());
        assert!(TextAnimationPreset::HighlightPop.uses_highlight());
        assert!(!TextAnimationPreset::SlideUp.uses_highlight());
    }

    #[test]
    fn agent_values_and_parsing() {
        let vals = TextAnimationPreset::agent_values();
        assert_eq!(vals[0], "off");
        assert!(vals.contains(&"wordCycle"));
        assert!(!vals.contains(&"none"));
        assert_eq!(
            TextAnimationPreset::from_agent_str("off"),
            Some(TextAnimationPreset::None)
        );
        assert_eq!(
            TextAnimationPreset::from_agent_str("slideUp"),
            Some(TextAnimationPreset::SlideUp)
        );
        assert_eq!(TextAnimationPreset::from_agent_str("bogus"), None);
    }

    #[test]
    fn text_animation_defaults_and_lenient_decode() {
        let a = TextAnimation::default();
        assert!(!a.is_active());
        assert_eq!(a.per_word_frames, 6);
        // Missing fields default (empty object → preset none, 6 frames, no highlight).
        let decoded: TextAnimation = serde_json::from_str("{}").unwrap();
        assert_eq!(decoded, TextAnimation::default());
        // perWordFrames omitted defaults to 6 even when a preset is present.
        let d2: TextAnimation = serde_json::from_str("{\"preset\":\"wordReveal\"}").unwrap();
        assert_eq!(d2.preset, TextAnimationPreset::WordReveal);
        assert_eq!(d2.per_word_frames, 6);
    }

    #[test]
    fn word_timing_camel_case_keys() {
        let w = WordTiming {
            text: "hi".into(),
            start_frame: 3,
            end_frame: 9,
        };
        let json = serde_json::to_string(&w).unwrap();
        assert!(json.contains("\"startFrame\":3"), "{json}");
        assert!(json.contains("\"endFrame\":9"), "{json}");
        let back: WordTiming = serde_json::from_str(&json).unwrap();
        assert_eq!(back, w);
    }
}

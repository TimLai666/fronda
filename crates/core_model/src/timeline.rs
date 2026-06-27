use crate::effect::Effect;
use crate::shape_style::ShapeStyle;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

fn new_id() -> String {
    Uuid::new_v4().to_string()
}

fn default_timeline_fps() -> i64 {
    30
}

fn default_timeline_width() -> i64 {
    1920
}

fn default_timeline_height() -> i64 {
    1080
}

fn default_false() -> bool {
    false
}

fn default_true() -> bool {
    true
}

fn default_zero_i64() -> i64 {
    0
}

fn default_one_f64() -> f64 {
    1.0
}

fn default_clip_type_video() -> ClipType {
    ClipType::Video
}

fn default_interpolation_linear() -> Interpolation {
    Interpolation::Linear
}

fn default_interpolation_smooth() -> Interpolation {
    Interpolation::Smooth
}

fn default_text_font_name() -> String {
    "Helvetica-Bold".to_string()
}

fn default_text_font_size() -> f64 {
    96.0
}

fn default_text_alignment() -> TextAlignment {
    TextAlignment::Center
}

fn default_shadow_color() -> TextRgba {
    TextRgba {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.6,
    }
}

fn default_text_background() -> TextFill {
    TextFill {
        enabled: false,
        color: default_shadow_color(),
        padding: None,
        corner_radius: None,
    }
}

fn default_text_border() -> TextFill {
    TextFill {
        enabled: false,
        color: TextRgba {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
        padding: None,
        corner_radius: None,
    }
}

fn is_blend_mode_normal(m: &BlendMode) -> bool {
    *m == BlendMode::Normal
}

fn default_font_weight() -> f64 {
    400.0
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClipType {
    Video,
    Audio,
    Image,
    Text,
    Lottie,
    /// Shape annotations (rect, oval, arrow, etc.). Upstream PR #46.
    Shape,
}

impl ClipType {
    /// Classify a file extension into a ClipType.
    /// Upstream PR #105: added .aifc and .flac.
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "mov" | "mp4" | "m4v" => Some(Self::Video),
            "mp3" | "wav" | "aac" | "m4a" | "aiff" | "aif" | "aifc" | "flac" => Some(Self::Audio),
            "png" | "jpg" | "jpeg" | "tiff" | "heic" | "webp" => Some(Self::Image),
            "json" | "lottie" => Some(Self::Lottie),
            _ => None,
        }
    }

    /// Human-readable name for this clip type.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Video => "video",
            Self::Audio => "audio",
            Self::Image => "image",
            Self::Text => "text",
            Self::Lottie => "lottie",
            Self::Shape => "shape",
        }
    }

    /// Returns true for visual clip types (CORE-002).
    /// Video, image, text, lottie, and shape are visual.
    pub fn is_visual(&self) -> bool {
        matches!(
            self,
            Self::Video | Self::Image | Self::Text | Self::Lottie | Self::Shape
        )
    }

    /// Returns true for audio clip types.
    pub fn is_audio(&self) -> bool {
        matches!(self, Self::Audio)
    }
}

/// Per-clip blend mode (Issue #98 — compositor blend modes).
///
/// Applied when compositing this clip over the layers below it.
/// Default is `Normal` (standard alpha compositing).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum BlendMode {
    /// Standard alpha compositing (default).
    #[default]
    Normal,
    Multiply,
    Screen,
    Overlay,
    SoftLight,
    HardLight,
    Darken,
    Lighten,
    ColorDodge,
    ColorBurn,
    Difference,
    Exclusion,
    Hue,
    Saturation,
    Color,
    Luminosity,
}

impl BlendMode {
    pub fn all() -> &'static [BlendMode] {
        &[
            BlendMode::Normal,
            BlendMode::Multiply,
            BlendMode::Screen,
            BlendMode::Overlay,
            BlendMode::SoftLight,
            BlendMode::HardLight,
            BlendMode::Darken,
            BlendMode::Lighten,
            BlendMode::ColorDodge,
            BlendMode::ColorBurn,
            BlendMode::Difference,
            BlendMode::Exclusion,
            BlendMode::Hue,
            BlendMode::Saturation,
            BlendMode::Color,
            BlendMode::Luminosity,
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Interpolation {
    Linear,
    Hold,
    Smooth,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Keyframe<Value> {
    pub frame: i64,
    pub value: Value,
    #[serde(default = "default_interpolation_smooth")]
    pub interpolation_out: Interpolation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyframeTrack<Value> {
    #[serde(default)]
    pub keyframes: Vec<Keyframe<Value>>,
}

impl<Value> Default for KeyframeTrack<Value> {
    fn default() -> Self {
        Self {
            keyframes: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct AnimPair {
    pub a: f64,
    pub b: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct Crop {
    #[serde(default)]
    pub left: f64,
    #[serde(default)]
    pub top: f64,
    #[serde(default)]
    pub right: f64,
    #[serde(default)]
    pub bottom: f64,
}

impl Crop {
    /// Returns true when all insets are zero (no cropping).
    pub fn is_identity(&self) -> bool {
        self.left == 0.0 && self.top == 0.0 && self.right == 0.0 && self.bottom == 0.0
    }

    /// Fraction of original width visible after left/right cropping.
    pub fn visible_width_fraction(&self) -> f64 {
        (1.0 - self.left - self.right).max(0.0)
    }

    /// Fraction of original height visible after top/bottom cropping.
    pub fn visible_height_fraction(&self) -> f64 {
        (1.0 - self.top - self.bottom).max(0.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Transform {
    pub center_x: f64,
    pub center_y: f64,
    pub width: f64,
    pub height: f64,
    pub rotation: f64,
    pub flip_horizontal: bool,
    pub flip_vertical: bool,
}

impl Transform {
    /// Top-left corner in normalized coordinates.
    pub fn top_left(&self) -> (f64, f64) {
        (
            self.center_x - self.width / 2.0,
            self.center_y - self.height / 2.0,
        )
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            center_x: 0.5,
            center_y: 0.5,
            width: 1.0,
            height: 1.0,
            rotation: 0.0,
            flip_horizontal: false,
            flip_vertical: false,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TransformRepr {
    center_x: Option<f64>,
    center_y: Option<f64>,
    width: Option<f64>,
    height: Option<f64>,
    rotation: Option<f64>,
    flip_horizontal: Option<bool>,
    flip_vertical: Option<bool>,
    x: Option<f64>,
    y: Option<f64>,
}

impl<'de> Deserialize<'de> for Transform {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let repr = TransformRepr::deserialize(deserializer)?;
        let width = repr.width.unwrap_or(1.0);
        let height = repr.height.unwrap_or(1.0);

        let center_x = match (repr.center_x, repr.x) {
            (Some(center_x), _) => center_x,
            (None, Some(x)) => x + width - 0.5,
            (None, None) => 0.5,
        };

        let center_y = match (repr.center_y, repr.y) {
            (Some(center_y), _) => center_y,
            (None, Some(y)) => y + height - 0.5,
            (None, None) => 0.5,
        };

        Ok(Self {
            center_x,
            center_y,
            width,
            height,
            rotation: repr.rotation.unwrap_or(0.0),
            flip_horizontal: repr.flip_horizontal.unwrap_or(false),
            flip_vertical: repr.flip_vertical.unwrap_or(false),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TextAlignment {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TextRgba {
    #[serde(default = "default_one_f64")]
    pub r: f64,
    #[serde(default = "default_one_f64")]
    pub g: f64,
    #[serde(default = "default_one_f64")]
    pub b: f64,
    #[serde(default = "default_one_f64")]
    pub a: f64,
}

impl Default for TextRgba {
    fn default() -> Self {
        Self {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextShadow {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_shadow_color")]
    pub color: TextRgba,
    #[serde(default)]
    pub offset_x: f64,
    #[serde(default = "default_shadow_offset_y")]
    pub offset_y: f64,
    #[serde(default = "default_shadow_blur")]
    pub blur: f64,
}

fn default_shadow_offset_y() -> f64 {
    -2.0
}

fn default_shadow_blur() -> f64 {
    6.0
}

impl Default for TextShadow {
    fn default() -> Self {
        Self {
            enabled: true,
            color: default_shadow_color(),
            offset_x: 0.0,
            offset_y: -2.0,
            blur: 6.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextFill {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub color: TextRgba,
    /// Padding around the text in pixels (Issue #18 — caption background styling).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub padding: Option<f64>,
    /// Corner radius for the background pill/rounded rect (Issue #18).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub corner_radius: Option<f64>,
}

impl Default for TextFill {
    fn default() -> Self {
        Self {
            enabled: false,
            color: TextRgba::default(),
            padding: None,
            corner_radius: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextStyle {
    #[serde(default = "default_text_font_name")]
    pub font_name: String,
    #[serde(default = "default_text_font_size")]
    pub font_size: f64,
    #[serde(default = "default_one_f64")]
    pub font_scale: f64,
    #[serde(default)]
    pub color: TextRgba,
    #[serde(default = "default_text_alignment")]
    pub alignment: TextAlignment,
    #[serde(default)]
    pub shadow: TextShadow,
    #[serde(default = "default_text_background")]
    pub background: TextFill,
    #[serde(default = "default_text_border")]
    pub border: TextFill,
    /// Font weight (400 = normal, 700 = bold). Upstream PR #65.
    #[serde(default = "default_font_weight")]
    pub font_weight: f64,
    /// Variable font axis values (Issue #50).
    ///
    /// Maps OpenType axis tag → value, e.g. {"wdth": 100.0, "GRAD": 0.0}.
    /// Requires a variable font; ignored on static fonts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variable_font_axes: Option<std::collections::HashMap<String, f64>>,
    /// Letter spacing in points (Issue #50 / motion typography).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub letter_spacing: Option<f64>,
    /// Line height multiplier (Issue #50). 1.0 = normal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_height: Option<f64>,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            font_name: default_text_font_name(),
            font_size: default_text_font_size(),
            font_scale: 1.0,
            color: TextRgba::default(),
            alignment: TextAlignment::Center,
            shadow: TextShadow::default(),
            background: default_text_background(),
            border: default_text_border(),
            font_weight: 400.0,
            variable_font_axes: None,
            letter_spacing: None,
            line_height: None,
        }
    }
}

/// Chroma-key (green-screen) removal settings for a clip (Issue #97).
///
/// The key color is specified as a normalized RGB triplet (0.0–1.0 per channel).
/// `tolerance` controls how wide a hue range is keyed out (0.0–1.0).
/// `spill_suppression` reduces color fringing from the key (0.0–1.0).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChromaKey {
    pub enabled: bool,
    /// Key color red component (0.0–1.0).
    pub key_r: f64,
    /// Key color green component (0.0–1.0).
    pub key_g: f64,
    /// Key color blue component (0.0–1.0).
    pub key_b: f64,
    /// Hue tolerance (0.0–1.0). Higher = more of the range is removed.
    #[serde(default = "default_chroma_tolerance")]
    pub tolerance: f64,
    /// Spill suppression (0.0–1.0).
    #[serde(default)]
    pub spill_suppression: f64,
}

fn default_chroma_tolerance() -> f64 {
    0.1
}

impl ChromaKey {
    /// Green-screen preset (pure green key, tolerance 0.1).
    pub fn green_screen() -> Self {
        Self {
            enabled: true,
            key_r: 0.0,
            key_g: 1.0,
            key_b: 0.0,
            tolerance: 0.1,
            spill_suppression: 0.0,
        }
    }

    /// Blue-screen preset.
    pub fn blue_screen() -> Self {
        Self {
            enabled: true,
            key_r: 0.0,
            key_g: 0.0,
            key_b: 1.0,
            tolerance: 0.1,
            spill_suppression: 0.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Clip {
    #[serde(default = "new_id")]
    pub id: String,
    pub media_ref: String,
    #[serde(default = "default_clip_type_video")]
    pub media_type: ClipType,
    #[serde(default = "default_clip_type_video")]
    pub source_clip_type: ClipType,
    pub start_frame: i64,
    pub duration_frames: i64,
    #[serde(default = "default_zero_i64")]
    pub trim_start_frame: i64,
    #[serde(default = "default_zero_i64")]
    pub trim_end_frame: i64,
    #[serde(default = "default_one_f64")]
    pub speed: f64,
    #[serde(default = "default_one_f64")]
    pub volume: f64,
    #[serde(default = "default_zero_i64")]
    pub fade_in_frames: i64,
    #[serde(default = "default_zero_i64")]
    pub fade_out_frames: i64,
    #[serde(default = "default_interpolation_linear")]
    pub fade_in_interpolation: Interpolation,
    #[serde(default = "default_interpolation_linear")]
    pub fade_out_interpolation: Interpolation,
    #[serde(default = "default_one_f64")]
    pub opacity: f64,
    #[serde(default)]
    pub transform: Transform,
    #[serde(default)]
    pub crop: Crop,
    pub link_group_id: Option<String>,
    pub caption_group_id: Option<String>,
    pub text_content: Option<String>,
    pub text_style: Option<TextStyle>,
    pub opacity_track: Option<KeyframeTrack<f64>>,
    pub position_track: Option<KeyframeTrack<AnimPair>>,
    pub scale_track: Option<KeyframeTrack<AnimPair>>,
    pub rotation_track: Option<KeyframeTrack<f64>>,
    pub crop_track: Option<KeyframeTrack<Crop>>,
    pub volume_track: Option<KeyframeTrack<f64>>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effects: Option<Vec<Effect>>,

    /// Shape annotation style. PR #46.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shape_style: Option<ShapeStyle>,
    /// Stroke-draw progress keyframes for draw-on/un-draw animation. PR #46.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke_progress_track: Option<KeyframeTrack<f64>>,
    /// Compound clip (nested sequence) reference (Issue #155).
    ///
    /// When `Some`, this clip is a compound clip whose internal timeline
    /// is stored in the project's `compound_timelines` map under this key.
    /// Double-clicking opens the nested timeline; dissolving flattens it back.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compound_timeline_id: Option<String>,
    /// Compositing blend mode for this clip (Issue #98).
    /// Default `Normal` = standard alpha compositing. Omitted from JSON when Normal.
    #[serde(default, skip_serializing_if = "is_blend_mode_normal")]
    pub blend_mode: BlendMode,
    /// Chroma-key / green-screen removal config (Issue #97).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chroma_key: Option<ChromaKey>,
}

impl Track {
    /// Returns true if a clip of the given type is compatible with this track (CORE-003).
    /// Audio clips are compatible only with audio tracks.
    /// All visual clip types (video, image, text, lottie, shape) are compatible with visual tracks.
    pub fn is_compatible_with(&self, clip_type: ClipType) -> bool {
        match self.r#type {
            ClipType::Audio => clip_type == ClipType::Audio,
            _ => clip_type.is_visual(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    #[serde(default = "new_id")]
    pub id: String,
    #[serde(rename = "type")]
    pub r#type: ClipType,
    #[serde(default = "default_false")]
    pub muted: bool,
    #[serde(default = "default_false")]
    pub hidden: bool,
    #[serde(default = "default_true")]
    pub sync_locked: bool,
    #[serde(default)]
    pub clips: Vec<Clip>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Timeline {
    #[serde(default = "default_timeline_fps")]
    pub fps: i64,
    #[serde(default = "default_timeline_width")]
    pub width: i64,
    #[serde(default = "default_timeline_height")]
    pub height: i64,
    #[serde(default = "default_false")]
    pub settings_configured: bool,
    #[serde(default)]
    pub selected_clip_ids: HashSet<String>,
    #[serde(default)]
    pub tracks: Vec<Track>,
    /// Spoken language for transcription, as BCP-47 tag.
    /// When None, the system/engine default language is used.
    /// Serialized only when set (skip_serializing_if = Option::is_none).
    /// Upstream PR #40.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transcription_language: Option<String>,
    /// Nested timelines for compound clips (Issue #155).
    ///
    /// Maps `compound_timeline_id` → nested `Timeline`. When a clip has
    /// `compound_timeline_id = Some(id)`, the corresponding nested timeline
    /// lives here. Serialized only when non-empty.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub compound_timelines: HashMap<String, Box<Timeline>>,
}

impl Timeline {
    /// Convert source seconds to project frames using project fps (CORE-005).
    /// This must use the project timeline fps, not the source file's native fps.
    pub fn seconds_to_frames(&self, seconds: f64) -> i64 {
        (seconds * self.fps as f64).round() as i64
    }

    /// Convert project frames back to seconds using project fps.
    pub fn frames_to_seconds(&self, frames: i64) -> f64 {
        frames as f64 / self.fps as f64
    }
}

impl Default for Timeline {
    fn default() -> Self {
        Self {
            fps: 30,
            width: 1920,
            height: 1080,
            settings_configured: false,
            selected_clip_ids: HashSet::new(),
            tracks: Vec::new(),
            transcription_language: None,
            compound_timelines: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clip_type_from_extension_video() {
        for ext in &["mov", "mp4", "m4v", "MOV", "Mp4"] {
            assert_eq!(ClipType::from_extension(ext), Some(ClipType::Video));
        }
    }

    #[test]
    fn clip_type_from_extension_audio() {
        for ext in &["mp3", "wav", "aac", "m4a", "aiff", "aif", "aifc", "flac"] {
            assert_eq!(ClipType::from_extension(ext), Some(ClipType::Audio));
        }
    }

    #[test]
    fn clip_type_from_extension_image() {
        for ext in &["png", "jpg", "jpeg", "tiff", "heic", "webp"] {
            assert_eq!(ClipType::from_extension(ext), Some(ClipType::Image));
        }
    }

    #[test]
    fn clip_type_from_extension_lottie() {
        for ext in &["json", "lottie"] {
            assert_eq!(ClipType::from_extension(ext), Some(ClipType::Lottie));
        }
    }

    #[test]
    fn clip_type_from_extension_unknown() {
        assert_eq!(ClipType::from_extension("txt"), None);
        assert_eq!(ClipType::from_extension(""), None);
        assert_eq!(ClipType::from_extension("exe"), None);
    }

    #[test]
    fn core_002_is_visual() {
        assert!(ClipType::Video.is_visual());
        assert!(ClipType::Image.is_visual());
        assert!(ClipType::Text.is_visual());
        assert!(ClipType::Lottie.is_visual());
        assert!(ClipType::Shape.is_visual());
        assert!(!ClipType::Audio.is_visual());
    }

    #[test]
    fn core_002_is_audio() {
        assert!(ClipType::Audio.is_audio());
        assert!(!ClipType::Video.is_audio());
        assert!(!ClipType::Image.is_audio());
        assert!(!ClipType::Text.is_audio());
        assert!(!ClipType::Lottie.is_audio());
        assert!(!ClipType::Shape.is_audio());
    }

    #[test]
    fn core_003_track_compatibility_audio() {
        let audio_track = Track {
            id: "a1".into(),
            r#type: ClipType::Audio,
            muted: false,
            hidden: false,
            sync_locked: false,
            clips: vec![],
        };
        assert!(audio_track.is_compatible_with(ClipType::Audio));
        assert!(!audio_track.is_compatible_with(ClipType::Video));
        assert!(!audio_track.is_compatible_with(ClipType::Image));
        assert!(!audio_track.is_compatible_with(ClipType::Text));
        assert!(!audio_track.is_compatible_with(ClipType::Lottie));
        assert!(!audio_track.is_compatible_with(ClipType::Shape));
    }

    #[test]
    fn core_003_track_compatibility_visual() {
        let video_track = Track {
            id: "v1".into(),
            r#type: ClipType::Video,
            muted: false,
            hidden: false,
            sync_locked: false,
            clips: vec![],
        };
        assert!(video_track.is_compatible_with(ClipType::Video));
        assert!(video_track.is_compatible_with(ClipType::Image));
        assert!(video_track.is_compatible_with(ClipType::Text));
        assert!(video_track.is_compatible_with(ClipType::Lottie));
        assert!(video_track.is_compatible_with(ClipType::Shape));
        assert!(!video_track.is_compatible_with(ClipType::Audio));
    }

    #[test]
    fn core_005_seconds_to_frames() {
        let mut timeline = Timeline::default();
        timeline.fps = 30;
        assert_eq!(timeline.seconds_to_frames(0.0), 0);
        assert_eq!(timeline.seconds_to_frames(1.0), 30);
        assert_eq!(timeline.seconds_to_frames(2.5), 75);
        assert_eq!(timeline.seconds_to_frames(0.033), 1);
    }

    #[test]
    fn core_005_frames_to_seconds() {
        let mut timeline = Timeline::default();
        timeline.fps = 30;
        assert!((timeline.frames_to_seconds(0) - 0.0).abs() < 1e-9);
        assert!((timeline.frames_to_seconds(30) - 1.0).abs() < 1e-9);
        assert!((timeline.frames_to_seconds(75) - 2.5).abs() < 1e-9);
    }

    #[test]
    fn core_005_custom_fps() {
        let mut timeline = Timeline::default();
        timeline.fps = 60;
        assert_eq!(timeline.seconds_to_frames(1.0), 60);
        assert_eq!(timeline.seconds_to_frames(0.5), 30);
    }

    // ── Issue #50: variable fonts ─────────────────────────────────────────────

    #[test]
    fn issue_050_textstyle_default_has_no_variable_axes() {
        let style = TextStyle::default();
        assert!(style.variable_font_axes.is_none());
        assert!(style.letter_spacing.is_none());
        assert!(style.line_height.is_none());
    }

    #[test]
    fn issue_050_textstyle_variable_axes_roundtrip() {
        let mut style = TextStyle::default();
        let mut axes = std::collections::HashMap::new();
        axes.insert("wdth".to_string(), 100.0_f64);
        axes.insert("GRAD".to_string(), 0.0_f64);
        style.variable_font_axes = Some(axes.clone());
        style.letter_spacing = Some(1.5);
        style.line_height = Some(1.2);

        let json = serde_json::to_string(&style).unwrap();
        let restored: TextStyle = serde_json::from_str(&json).unwrap();

        let axes_restored = restored.variable_font_axes.unwrap();
        assert!((axes_restored["wdth"] - 100.0).abs() < 1e-9);
        assert!((axes_restored["GRAD"] - 0.0).abs() < 1e-9);
        assert!((restored.letter_spacing.unwrap() - 1.5).abs() < 1e-9);
        assert!((restored.line_height.unwrap() - 1.2).abs() < 1e-9);
    }

    #[test]
    fn issue_050_textstyle_without_variable_axes_skips_field() {
        let style = TextStyle::default();
        let json = serde_json::to_string(&style).unwrap();
        // None fields should not appear in JSON
        assert!(!json.contains("variableFontAxes"), "json={json}");
        assert!(!json.contains("letterSpacing"), "json={json}");
        assert!(!json.contains("lineHeight"), "json={json}");
    }

    // ── Issue #155: compound clips ────────────────────────────────────────────

    #[test]
    fn issue_155_timeline_default_has_empty_compound_timelines() {
        let t = Timeline::default();
        assert!(t.compound_timelines.is_empty());
    }

    #[test]
    fn issue_155_compound_timelines_roundtrip() {
        let mut t = Timeline::default();
        let mut nested = Timeline::default();
        nested.fps = 24;
        t.compound_timelines.insert("ct-1".to_string(), Box::new(nested));

        let json = serde_json::to_string(&t).unwrap();
        let restored: Timeline = serde_json::from_str(&json).unwrap();
        assert!(restored.compound_timelines.contains_key("ct-1"));
        assert_eq!(restored.compound_timelines["ct-1"].fps, 24);
    }

    #[test]
    fn issue_155_empty_compound_timelines_not_serialized() {
        let t = Timeline::default();
        let json = serde_json::to_string(&t).unwrap();
        assert!(!json.contains("compoundTimelines"), "json={json}");
    }

    // ── Issue #98: Blend modes ────────────────────────────────────────────────

    #[test]
    fn issue_098_blend_mode_default_is_normal() {
        assert_eq!(BlendMode::default(), BlendMode::Normal);
    }

    #[test]
    fn issue_098_blend_mode_all_has_16_variants() {
        assert_eq!(BlendMode::all().len(), 16);
    }

    #[test]
    fn issue_098_clip_default_blend_mode_omitted_from_json() {
        // Normal blend mode must not be serialized (skip_serializing_if)
        let clip_json = r#"{"id":"c1","mediaRef":"m","mediaType":"video","sourceClipType":"video","startFrame":0,"durationFrames":30}"#;
        let clip: Clip = serde_json::from_str(clip_json).unwrap();
        assert_eq!(clip.blend_mode, BlendMode::Normal);
        let out = serde_json::to_string(&clip).unwrap();
        assert!(!out.contains("blendMode"), "Normal blend mode must be omitted: {out}");
    }

    #[test]
    fn issue_098_non_normal_blend_mode_serialized() {
        let mut clip = Clip {
            id: "c1".into(),
            media_ref: "m".into(),
            media_type: ClipType::Video,
            source_clip_type: ClipType::Video,
            start_frame: 0,
            duration_frames: 30,
            trim_start_frame: 0,
            trim_end_frame: 0,
            speed: 1.0,
            volume: 1.0,
            fade_in_frames: 0,
            fade_out_frames: 0,
            fade_in_interpolation: Interpolation::Linear,
            fade_out_interpolation: Interpolation::Linear,
            opacity: 1.0,
            transform: Transform::default(),
            crop: Crop::default(),
            link_group_id: None,
            caption_group_id: None,
            text_content: None,
            text_style: None,
            opacity_track: None,
            position_track: None,
            scale_track: None,
            rotation_track: None,
            crop_track: None,
            volume_track: None,
            effects: None,
            shape_style: None,
            stroke_progress_track: None,
            compound_timeline_id: None,
            blend_mode: BlendMode::Multiply,
            chroma_key: None,
        };
        let json = serde_json::to_string(&clip).unwrap();
        assert!(json.contains("\"blendMode\":\"multiply\""), "Multiply must be serialized: {json}");
    }

    // ── Issue #97: Chroma key ─────────────────────────────────────────────────

    #[test]
    fn issue_097_chroma_key_green_screen_preset() {
        let ck = ChromaKey::green_screen();
        assert!(ck.enabled);
        assert!((ck.key_g - 1.0).abs() < 1e-9, "green channel must be 1.0");
        assert!((ck.key_r).abs() < 1e-9);
        assert!((ck.key_b).abs() < 1e-9);
    }

    #[test]
    fn issue_097_chroma_key_blue_screen_preset() {
        let ck = ChromaKey::blue_screen();
        assert!(ck.enabled);
        assert!((ck.key_b - 1.0).abs() < 1e-9);
    }

    #[test]
    fn issue_097_chroma_key_serde_roundtrip() {
        let ck = ChromaKey::green_screen();
        let json = serde_json::to_string(&ck).unwrap();
        let restored: ChromaKey = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.enabled, ck.enabled);
        assert!((restored.key_g - 1.0).abs() < 1e-9);
    }

    #[test]
    fn issue_097_clip_chroma_key_none_omitted() {
        let json = r#"{"id":"c1","mediaRef":"m","mediaType":"video","sourceClipType":"video","startFrame":0,"durationFrames":30}"#;
        let clip: Clip = serde_json::from_str(json).unwrap();
        assert!(clip.chroma_key.is_none());
        let out = serde_json::to_string(&clip).unwrap();
        assert!(!out.contains("chromaKey"), "None chroma_key must be omitted: {out}");
    }

    // ── Issue #18: Caption background styling ────────────────────────────────

    #[test]
    fn issue_018_text_fill_padding_optional() {
        let fill = TextFill {
            enabled: true,
            color: TextRgba { r: 0.0, g: 0.0, b: 0.0, a: 0.5 },
            padding: Some(8.0),
            corner_radius: None,
        };
        let json = serde_json::to_string(&fill).unwrap();
        assert!(json.contains("\"padding\":8.0"), "padding must be serialized: {json}");
        assert!(!json.contains("corner_radius"), "None corner_radius must be omitted: {json}");
    }

    #[test]
    fn issue_018_text_fill_corner_radius() {
        let fill = TextFill {
            enabled: true,
            color: TextRgba { r: 0.0, g: 0.0, b: 0.0, a: 0.5 },
            padding: Some(4.0),
            corner_radius: Some(6.0),
        };
        let json = serde_json::to_string(&fill).unwrap();
        assert!(json.contains("\"corner_radius\":6.0"), "corner_radius must be serialized: {json}");
    }

    #[test]
    fn issue_018_text_fill_default_has_no_padding_or_corner() {
        let fill = TextFill::default();
        assert!(fill.padding.is_none());
        assert!(fill.corner_radius.is_none());
        let json = serde_json::to_string(&fill).unwrap();
        assert!(!json.contains("padding"), "default padding must be omitted: {json}");
        assert!(!json.contains("corner_radius"), "default corner_radius must be omitted: {json}");
    }
}

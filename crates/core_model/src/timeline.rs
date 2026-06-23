use crate::effect::Effect;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashSet;
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
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClipType {
    Video,
    Audio,
    Image,
    Text,
    Lottie,
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
}

impl Default for TextFill {
    fn default() -> Self {
        Self {
            enabled: false,
            color: TextRgba::default(),
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
        }
    }
}

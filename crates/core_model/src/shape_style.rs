use serde::{Deserialize, Serialize};

/// Shape annotation style. Upstream PR #46.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShapeStyle {
    /// Shape kind identifier: rect, oval, circle, arrow, line.
    #[serde(rename = "type")]
    pub kind: ShapeKind,
    #[serde(default)]
    pub stroke: Stroke,
    #[serde(default)]
    pub fill: Fill,
    pub arrowhead: Option<Arrowhead>,
    pub endpoints: Option<Endpoints>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ShapeKind {
    Rect,
    Oval,
    Circle,
    Arrow,
    Line,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stroke {
    #[serde(default)]
    pub color: Rgba,
    #[serde(default = "default_stroke_width")]
    pub width: f64,
    #[serde(default)]
    pub dashed: bool,
    pub arrowhead_style: Option<String>,
}

fn default_stroke_width() -> f64 {
    2.0
}

impl Default for Stroke {
    fn default() -> Self {
        Self {
            color: Rgba::default(),
            width: 2.0,
            dashed: false,
            arrowhead_style: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Fill {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub color: Rgba,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Arrowhead {
    Open,
    Closed,
    Both,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Endpoints {
    pub start: Point2d,
    pub end: Point2d,
    pub start_control: Option<Point2d>,
    pub end_control: Option<Point2d>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point2d {
    pub x: f64,
    pub y: f64,
}

/// RGBA color with 0-1 float components.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rgba {
    #[serde(default = "default_one")]
    pub r: f64,
    #[serde(default = "default_one")]
    pub g: f64,
    #[serde(default = "default_one")]
    pub b: f64,
    #[serde(default = "default_one")]
    pub a: f64,
}

fn default_one() -> f64 {
    1.0
}

impl Default for Rgba {
    fn default() -> Self {
        Self {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        }
    }
}

impl Default for ShapeStyle {
    fn default() -> Self {
        Self {
            kind: ShapeKind::Rect,
            stroke: Stroke::default(),
            fill: Fill::default(),
            arrowhead: None,
            endpoints: None,
        }
    }
}

/// Named animation preset for shape annotations (Issue #45).
///
/// When applied to a clip containing a `ShapeStyle`, the compositor uses the
/// preset to animate the shape's entry, exit, or draw-on sequence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum ShapeAnimationPreset {
    /// No animation (static).
    #[default]
    None,
    /// Shape draws itself along its stroke path (for lines and arrows).
    DrawOn,
    /// Reverse of DrawOn — stroke erases itself out.
    DrawOff,
    /// Shape fades in.
    FadeIn,
    /// Shape fades out.
    FadeOut,
    /// Shape bounces in from below.
    BounceIn,
    /// Shape bounces out below.
    BounceOut,
    /// Shape scales up from a point.
    ScaleIn,
    /// Shape scales down to a point.
    ScaleOut,
}

impl ShapeAnimationPreset {
    pub fn all() -> &'static [ShapeAnimationPreset] {
        &[
            ShapeAnimationPreset::None,
            ShapeAnimationPreset::DrawOn,
            ShapeAnimationPreset::DrawOff,
            ShapeAnimationPreset::FadeIn,
            ShapeAnimationPreset::FadeOut,
            ShapeAnimationPreset::BounceIn,
            ShapeAnimationPreset::BounceOut,
            ShapeAnimationPreset::ScaleIn,
            ShapeAnimationPreset::ScaleOut,
        ]
    }

    /// Whether this preset uses a stroke-progress keyframe track.
    pub fn uses_stroke_progress(&self) -> bool {
        matches!(
            self,
            ShapeAnimationPreset::DrawOn | ShapeAnimationPreset::DrawOff
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Issue #45: AI shape annotations + animation presets

    #[test]
    fn issue_045_shape_animation_preset_default_is_none() {
        assert_eq!(ShapeAnimationPreset::default(), ShapeAnimationPreset::None);
    }

    #[test]
    fn issue_045_preset_all_has_nine_variants() {
        assert_eq!(ShapeAnimationPreset::all().len(), 9);
    }

    #[test]
    fn issue_045_draw_on_uses_stroke_progress() {
        assert!(ShapeAnimationPreset::DrawOn.uses_stroke_progress());
        assert!(ShapeAnimationPreset::DrawOff.uses_stroke_progress());
    }

    #[test]
    fn issue_045_fade_in_does_not_use_stroke_progress() {
        assert!(!ShapeAnimationPreset::FadeIn.uses_stroke_progress());
    }

    #[test]
    fn issue_045_preset_serde_roundtrip() {
        let p = ShapeAnimationPreset::BounceIn;
        let json = serde_json::to_string(&p).unwrap();
        let restored: ShapeAnimationPreset = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, ShapeAnimationPreset::BounceIn);
    }

    #[test]
    fn issue_045_shape_style_defaults() {
        let s = ShapeStyle::default();
        assert_eq!(s.kind, ShapeKind::Rect);
        assert!(!s.fill.enabled);
        assert!((s.stroke.width - 2.0).abs() < 1e-9);
    }
}

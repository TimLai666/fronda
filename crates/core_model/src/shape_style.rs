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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Fill {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub color: Rgba,
}

impl Default for Fill {
    fn default() -> Self {
        Self {
            enabled: false,
            color: Rgba::default(),
        }
    }
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

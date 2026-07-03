//! Predefined multi-video layouts (Swift: VideoLayout). Upstream palmier-pro #226.
//!
//! This is the layout catalog + slot geometry that `apply_layout` builds on: each
//! named layout defines slots (normalized canvas rects, 0..1) with a stacking
//! order `z`. The per-clip fill/fit transform+crop placement math and the
//! `apply_layout` agent tool are the next layer on top of this.

use crate::timeline::{Crop, Transform};

/// A normalized rectangle in canvas space (0..1 on each axis).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayoutRect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl LayoutRect {
    pub const fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Self { x, y, w, h }
    }
}

/// A named region within a layout, plus its stacking order (`z`, higher = on top).
#[derive(Debug, Clone, PartialEq)]
pub struct LayoutSlot {
    pub id: &'static str,
    pub rect: LayoutRect,
    pub z: i32,
}

/// How a clip is placed within its slot: `Fill` covers the slot (cropping
/// overflow), `Fit` letterboxes the whole source inside it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutFit {
    Fill,
    Fit,
}

impl LayoutFit {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "fill" => Some(LayoutFit::Fill),
            "fit" => Some(LayoutFit::Fit),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            LayoutFit::Fill => "fill",
            LayoutFit::Fit => "fit",
        }
    }
}

/// The predefined multi-video layouts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoLayout {
    Full,
    SideBySide,
    TopBottom,
    PipBottomRight,
    PipBottomLeft,
    PipTopRight,
    PipTopLeft,
    Grid2x2,
    MainSidebar,
    ThreeUp,
}

const PIP_INSET: f64 = 0.28;
const PIP_MARGIN: f64 = 0.035;

impl VideoLayout {
    pub const ALL: [VideoLayout; 10] = [
        VideoLayout::Full,
        VideoLayout::SideBySide,
        VideoLayout::TopBottom,
        VideoLayout::PipBottomRight,
        VideoLayout::PipBottomLeft,
        VideoLayout::PipTopRight,
        VideoLayout::PipTopLeft,
        VideoLayout::Grid2x2,
        VideoLayout::MainSidebar,
        VideoLayout::ThreeUp,
    ];

    /// The layout's stable id (matches the Swift `rawValue` and the agent schema).
    pub fn as_str(self) -> &'static str {
        match self {
            VideoLayout::Full => "full",
            VideoLayout::SideBySide => "side_by_side",
            VideoLayout::TopBottom => "top_bottom",
            VideoLayout::PipBottomRight => "pip_bottom_right",
            VideoLayout::PipBottomLeft => "pip_bottom_left",
            VideoLayout::PipTopRight => "pip_top_right",
            VideoLayout::PipTopLeft => "pip_top_left",
            VideoLayout::Grid2x2 => "grid_2x2",
            VideoLayout::MainSidebar => "main_sidebar",
            VideoLayout::ThreeUp => "three_up",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|l| l.as_str() == s)
    }

    /// The layout's slots, in definition order (stacking order via each slot's `z`).
    pub fn slots(self) -> Vec<LayoutSlot> {
        let slot = |id, x, y, w, h, z| LayoutSlot {
            id,
            rect: LayoutRect::new(x, y, w, h),
            z,
        };
        match self {
            VideoLayout::Full => vec![slot("main", 0.0, 0.0, 1.0, 1.0, 0)],
            VideoLayout::SideBySide => vec![
                slot("left", 0.0, 0.0, 0.5, 1.0, 0),
                slot("right", 0.5, 0.0, 0.5, 1.0, 0),
            ],
            VideoLayout::TopBottom => vec![
                slot("top", 0.0, 0.0, 1.0, 0.5, 0),
                slot("bottom", 0.0, 0.5, 1.0, 0.5, 0),
            ],
            VideoLayout::PipBottomRight => {
                pip(1.0 - PIP_MARGIN - PIP_INSET, 1.0 - PIP_MARGIN - PIP_INSET)
            }
            VideoLayout::PipBottomLeft => pip(PIP_MARGIN, 1.0 - PIP_MARGIN - PIP_INSET),
            VideoLayout::PipTopRight => pip(1.0 - PIP_MARGIN - PIP_INSET, PIP_MARGIN),
            VideoLayout::PipTopLeft => pip(PIP_MARGIN, PIP_MARGIN),
            VideoLayout::Grid2x2 => vec![
                slot("top_left", 0.0, 0.0, 0.5, 0.5, 0),
                slot("top_right", 0.5, 0.0, 0.5, 0.5, 0),
                slot("bottom_left", 0.0, 0.5, 0.5, 0.5, 0),
                slot("bottom_right", 0.5, 0.5, 0.5, 0.5, 0),
            ],
            VideoLayout::MainSidebar => vec![
                slot("main", 0.0, 0.0, 0.7, 1.0, 0),
                slot("sidebar", 0.7, 0.0, 0.3, 1.0, 0),
            ],
            VideoLayout::ThreeUp => {
                let third = 1.0 / 3.0;
                vec![
                    slot("left", 0.0, 0.0, third, 1.0, 0),
                    slot("center", third, 0.0, third, 1.0, 0),
                    slot("right", third * 2.0, 0.0, third, 1.0, 0),
                ]
            }
        }
    }
}

/// Source aspect relative to the canvas: `(sw/sh) / (cw/ch)` (Swift:
/// `mediaCanvasAspect`). `None` when any dimension is non-positive.
pub fn media_canvas_aspect(
    source_w: i64,
    source_h: i64,
    canvas_w: i64,
    canvas_h: i64,
) -> Option<f64> {
    if source_w <= 0 || source_h <= 0 || canvas_w <= 0 || canvas_h <= 0 {
        return None;
    }
    let canvas_aspect = canvas_w as f64 / canvas_h as f64;
    Some((source_w as f64 / source_h as f64) / canvas_aspect)
}

/// The crop that makes a source of pixel aspect `sw/sh` cover a region of pixel
/// aspect `target` (Swift: `cropFittingAspect`). Anchors bias which part of the
/// over-scanned axis survives (0..1, default centered). Unknown dims → no crop.
pub fn crop_fitting_aspect(
    source_w: i64,
    source_h: i64,
    target_pixel_aspect: f64,
    anchor_x: f64,
    anchor_y: f64,
) -> Crop {
    if source_w <= 0 || source_h <= 0 || target_pixel_aspect <= 0.0 {
        return Crop::default();
    }
    let source_aspect = source_w as f64 / source_h as f64;
    if (source_aspect - target_pixel_aspect).abs() < 0.0001 {
        return Crop::default();
    }
    let ax = anchor_x.clamp(0.0, 1.0);
    let ay = anchor_y.clamp(0.0, 1.0);
    if source_aspect > target_pixel_aspect {
        let total = 1.0 - target_pixel_aspect / source_aspect;
        let left = total * ax;
        Crop {
            left,
            top: 0.0,
            right: total - left,
            bottom: 0.0,
        }
    } else {
        let total = 1.0 - source_aspect / target_pixel_aspect;
        let top = total * ay;
        Crop {
            left: 0.0,
            top,
            right: 0.0,
            bottom: total - top,
        }
    }
}

fn transform_from_top_left(x: f64, y: f64, w: f64, h: f64) -> Transform {
    Transform {
        center_x: x + w / 2.0,
        center_y: y + h / 2.0,
        width: w,
        height: h,
        rotation: 0.0,
        flip_horizontal: false,
        flip_vertical: false,
    }
}

/// Compute a clip's `(Transform, Crop)` for placing it in `rect` under `fit`
/// (Swift: `layoutPlacement`). `Fill` covers the slot (cropping overscan);
/// `Fit` letterboxes the whole source inside it. Source/canvas dims are pixels;
/// non-positive source dims degrade to filling the slot rect with no crop.
pub fn layout_placement(
    rect: LayoutRect,
    fit: LayoutFit,
    source_w: i64,
    source_h: i64,
    canvas_w: i64,
    canvas_h: i64,
    anchor_x: f64,
    anchor_y: f64,
) -> (Transform, Crop) {
    let canvas_aspect = canvas_w as f64 / (canvas_h.max(1)) as f64;
    let slot_pixel_aspect = if rect.h > 0.0 {
        (rect.w / rect.h) * canvas_aspect
    } else {
        canvas_aspect
    };

    match fit {
        LayoutFit::Fill => {
            let crop = crop_fitting_aspect(source_w, source_h, slot_pixel_aspect, anchor_x, anchor_y);
            let vw = crop.visible_width_fraction();
            let vh = crop.visible_height_fraction();
            if vw <= 0.0 || vh <= 0.0 {
                return (
                    transform_from_top_left(rect.x, rect.y, rect.w, rect.h),
                    crop,
                );
            }
            let w = rect.w / vw;
            let h = rect.h / vh;
            let x = rect.x - crop.left * w;
            let y = rect.y - crop.top * h;
            (transform_from_top_left(x, y, w, h), crop)
        }
        LayoutFit::Fit => {
            let rel = media_canvas_aspect(source_w, source_h, canvas_w, canvas_h);
            let Some(rel) = rel.filter(|r| *r > 0.0) else {
                return (
                    transform_from_top_left(rect.x, rect.y, rect.w, rect.h),
                    Crop::default(),
                );
            };
            let (draw_w, draw_h) = if rel * rect.h <= rect.w {
                (rel * rect.h, rect.h)
            } else {
                (rect.w, rect.w / rel)
            };
            let ax = anchor_x.clamp(0.0, 1.0);
            let ay = anchor_y.clamp(0.0, 1.0);
            let x = rect.x + (rect.w - draw_w) * ax;
            let y = rect.y + (rect.h - draw_h) * ay;
            (transform_from_top_left(x, y, draw_w, draw_h), Crop::default())
        }
    }
}

fn pip(inset_x: f64, inset_y: f64) -> Vec<LayoutSlot> {
    vec![
        LayoutSlot {
            id: "main",
            rect: LayoutRect::new(0.0, 0.0, 1.0, 1.0),
            z: 0,
        },
        LayoutSlot {
            id: "inset",
            rect: LayoutRect::new(inset_x, inset_y, PIP_INSET, PIP_INSET),
            z: 1,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn all_layouts_round_trip_by_id() {
        for layout in VideoLayout::ALL {
            assert_eq!(VideoLayout::from_str(layout.as_str()), Some(layout));
        }
        assert_eq!(VideoLayout::from_str("nope"), None);
        assert_eq!(VideoLayout::ALL.len(), 10);
    }

    #[test]
    fn fit_from_str() {
        assert_eq!(LayoutFit::from_str("fill"), Some(LayoutFit::Fill));
        assert_eq!(LayoutFit::from_str("fit"), Some(LayoutFit::Fit));
        assert_eq!(LayoutFit::from_str("x"), None);
    }

    #[test]
    fn side_by_side_slots() {
        let s = VideoLayout::SideBySide.slots();
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].id, "left");
        assert_eq!(s[0].rect, LayoutRect::new(0.0, 0.0, 0.5, 1.0));
        assert_eq!(s[1].id, "right");
        assert_eq!(s[1].rect, LayoutRect::new(0.5, 0.0, 0.5, 1.0));
    }

    #[test]
    fn grid_2x2_slots() {
        let s = VideoLayout::Grid2x2.slots();
        let ids: Vec<&str> = s.iter().map(|x| x.id).collect();
        assert_eq!(ids, ["top_left", "top_right", "bottom_left", "bottom_right"]);
        assert_eq!(s[3].rect, LayoutRect::new(0.5, 0.5, 0.5, 0.5));
    }

    #[test]
    fn main_sidebar_is_70_30() {
        let s = VideoLayout::MainSidebar.slots();
        assert_eq!(s[0].rect, LayoutRect::new(0.0, 0.0, 0.7, 1.0));
        assert_eq!(s[1].rect, LayoutRect::new(0.7, 0.0, 0.3, 1.0));
    }

    #[test]
    fn three_up_thirds() {
        let s = VideoLayout::ThreeUp.slots();
        let third = 1.0 / 3.0;
        assert!(approx(s[0].rect.w, third));
        assert!(approx(s[1].rect.x, third));
        assert!(approx(s[2].rect.x, third * 2.0));
    }

    #[test]
    fn pip_bottom_right_inset_and_z_order() {
        let s = VideoLayout::PipBottomRight.slots();
        assert_eq!(s[0].id, "main");
        assert_eq!(s[0].rect, LayoutRect::new(0.0, 0.0, 1.0, 1.0));
        assert_eq!(s[0].z, 0);
        assert_eq!(s[1].id, "inset");
        assert_eq!(s[1].z, 1, "inset stacks on top");
        // inset is a 0.28 square at bottom-right with a 0.035 margin.
        assert!(approx(s[1].rect.w, 0.28));
        assert!(approx(s[1].rect.h, 0.28));
        assert!(approx(s[1].rect.x, 1.0 - 0.035 - 0.28));
        assert!(approx(s[1].rect.y, 1.0 - 0.035 - 0.28));
    }

    #[test]
    fn pip_top_left_inset_at_margin() {
        let s = VideoLayout::PipTopLeft.slots();
        assert!(approx(s[1].rect.x, 0.035));
        assert!(approx(s[1].rect.y, 0.035));
    }

    #[test]
    fn media_canvas_aspect_relative() {
        // 16:9 source in a 16:9 canvas → 1.0; missing dims → None.
        assert!(approx(media_canvas_aspect(1920, 1080, 1920, 1080).unwrap(), 1.0));
        assert_eq!(media_canvas_aspect(0, 1080, 1920, 1080), None);
    }

    #[test]
    fn crop_fitting_crops_wider_source_horizontally() {
        // 16:9 source (1.778) into a square target (1.0): crop the sides.
        let c = crop_fitting_aspect(1920, 1080, 1.0, 0.5, 0.5);
        let total = 1.0 - 1.0 / (1920.0 / 1080.0);
        assert!(approx(c.left, total / 2.0));
        assert!(approx(c.right, total / 2.0));
        assert!(approx(c.top, 0.0) && approx(c.bottom, 0.0));
    }

    #[test]
    fn crop_fitting_equal_aspect_is_no_crop() {
        assert_eq!(crop_fitting_aspect(1920, 1080, 1920.0 / 1080.0, 0.5, 0.5), Crop::default());
    }

    #[test]
    fn layout_placement_fill_side_by_side_left() {
        // 16:9 source into the left half of a 16:9 canvas: cover-crop the sides.
        let rect = VideoLayout::SideBySide.slots()[0].rect; // (0,0,0.5,1)
        let (t, c) = layout_placement(rect, LayoutFit::Fill, 1920, 1080, 1920, 1080, 0.5, 0.5);
        assert!(approx(c.left, 0.25), "crop.left={}", c.left);
        assert!(approx(c.right, 0.25));
        assert!(approx(t.center_x, 0.25), "center_x={}", t.center_x);
        // Rendered width × visible fraction == the slot's 0.5 width.
        assert!(approx(t.width * c.visible_width_fraction(), 0.5));
        assert!(approx(t.height, 1.0));
    }

    #[test]
    fn layout_placement_fit_letterboxes_no_crop() {
        let rect = VideoLayout::SideBySide.slots()[0].rect; // (0,0,0.5,1)
        let (t, c) = layout_placement(rect, LayoutFit::Fit, 1920, 1080, 1920, 1080, 0.5, 0.5);
        assert_eq!(c, Crop::default(), "fit never crops");
        // 16:9 source in a 0.5×1 slot → a 0.5×0.5 box centered vertically.
        assert!(approx(t.width, 0.5));
        assert!(approx(t.height, 0.5));
        assert!(approx(t.center_x, 0.25));
        assert!(approx(t.center_y, 0.5));
    }

    #[test]
    fn layout_placement_missing_dims_fills_slot_rect() {
        let rect = VideoLayout::Grid2x2.slots()[3].rect; // (0.5,0.5,0.5,0.5)
        let (t, c) = layout_placement(rect, LayoutFit::Fill, 0, 0, 1920, 1080, 0.5, 0.5);
        assert_eq!(c, Crop::default());
        assert!(approx(t.center_x, 0.75) && approx(t.center_y, 0.75));
        assert!(approx(t.width, 0.5) && approx(t.height, 0.5));
    }
}

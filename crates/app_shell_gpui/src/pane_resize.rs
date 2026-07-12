//! Pure divider-resize clamping for the editor pane layout.
//!
//! Limits mirror Swift `Utilities/Constants.swift`; the space guard keeps the
//! Preview column/region from being squeezed below its minimum.

use crate::pane::LayoutPreset;

pub const AGENT_MIN: f32 = 240.0;
pub const AGENT_MAX: f32 = 640.0;
/// Media content minimum; the tab rail width adds on top at call time.
pub const MEDIA_MIN: f32 = 280.0;
pub const INSPECTOR_MIN: f32 = 150.0;
pub const PREVIEW_MIN_W: f32 = 400.0;
pub const PREVIEW_MIN_H: f32 = 320.0;
pub const TIMELINE_MIN: f32 = 100.0;
pub const TIMELINE_MAX: f32 = 700.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeTarget {
    AgentWidth,
    MediaWidth,
    InspectorWidth,
    TimelineHeight,
    /// Vertical preset: the whole left (media/inspector + timeline) column.
    VerticalLeftWidth,
}

/// Which way a +delta pointer movement grows the pane: +1 targets have their
/// divider on the trailing edge, -1 targets on the leading/top edge.
pub fn drag_direction(target: ResizeTarget) -> f32 {
    match target {
        ResizeTarget::AgentWidth | ResizeTarget::MediaWidth | ResizeTarget::VerticalLeftWidth => {
            1.0
        }
        ResizeTarget::InspectorWidth | ResizeTarget::TimelineHeight => -1.0,
    }
}

/// Space available when clamping a proposed pane size, along the drag axis
/// (width for columns, height for the timeline region).
#[derive(Debug, Clone, Copy)]
pub struct ResizeBounds {
    /// Container space available along the axis.
    pub area: f32,
    /// Sum of the OTHER fixed siblings' current sizes along the axis.
    pub others: f32,
    /// Minimum size of the flexible neighbor being squeezed (Preview column
    /// 400 / Preview region 320 / Inspector 150 in the Vertical upper-left).
    pub neighbor_min: f32,
    /// Media tab-rail width (adds to the media/left-column minimum).
    pub rail_w: f32,
}

/// Clamp a proposed size for `target`. The pane's own minimum wins over the
/// space guard when the window is too small for both.
pub fn clamp_resize(target: ResizeTarget, proposed: f32, b: &ResizeBounds) -> f32 {
    let (min, hard_max) = match target {
        ResizeTarget::AgentWidth => (AGENT_MIN, AGENT_MAX),
        ResizeTarget::MediaWidth | ResizeTarget::VerticalLeftWidth => {
            (MEDIA_MIN + b.rail_w, f32::INFINITY)
        }
        ResizeTarget::InspectorWidth => (INSPECTOR_MIN, f32::INFINITY),
        ResizeTarget::TimelineHeight => (TIMELINE_MIN, TIMELINE_MAX),
    };
    let space_max = b.area - b.others - b.neighbor_min;
    let max = hard_max.min(space_max).max(min);
    proposed.clamp(min, max)
}

/// Initial Toolbar+Timeline region height for a preset area height.
/// Swift positions the divider at 70% (Default) / 55% (Media, Vertical) of
/// the area, i.e. the lower region starts at 30% / 45%.
pub fn initial_timeline_height(preset: LayoutPreset, area_h: f32) -> f32 {
    let frac = match preset {
        LayoutPreset::Default => 0.30,
        LayoutPreset::Media | LayoutPreset::Vertical => 0.45,
    };
    (area_h * frac).round().clamp(TIMELINE_MIN, TIMELINE_MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Column drag frame: 1600 wide, 500 of other columns, preview ≥ 400.
    fn bounds() -> ResizeBounds {
        ResizeBounds {
            area: 1600.0,
            others: 500.0,
            neighbor_min: PREVIEW_MIN_W,
            rail_w: 40.0,
        }
    }

    /// Timeline drag frame: 900 tall, preview region ≥ 320.
    fn tl_bounds(area_h: f32) -> ResizeBounds {
        ResizeBounds {
            area: area_h,
            others: 0.0,
            neighbor_min: PREVIEW_MIN_H,
            rail_w: 0.0,
        }
    }

    #[test]
    fn agent_clamps_to_swift_range() {
        assert_eq!(clamp_resize(ResizeTarget::AgentWidth, 100.0, &bounds()), 240.0);
        assert_eq!(clamp_resize(ResizeTarget::AgentWidth, 400.0, &bounds()), 400.0);
        assert_eq!(clamp_resize(ResizeTarget::AgentWidth, 900.0, &bounds()), 640.0);
    }

    #[test]
    fn media_minimum_includes_rail() {
        assert_eq!(clamp_resize(ResizeTarget::MediaWidth, 10.0, &bounds()), 320.0);
        assert_eq!(clamp_resize(ResizeTarget::MediaWidth, 600.0, &bounds()), 600.0);
    }

    #[test]
    fn inspector_minimum() {
        assert_eq!(clamp_resize(ResizeTarget::InspectorWidth, 20.0, &bounds()), 150.0);
    }

    #[test]
    fn preview_width_guard_limits_columns() {
        // area 1600, others 500 → column max = 1600 - 500 - 400 = 700.
        assert_eq!(clamp_resize(ResizeTarget::MediaWidth, 900.0, &bounds()), 700.0);
        assert_eq!(
            clamp_resize(ResizeTarget::InspectorWidth, 900.0, &bounds()),
            700.0
        );
        // Agent's own 640 hard max is tighter than the space guard here.
        assert_eq!(clamp_resize(ResizeTarget::AgentWidth, 900.0, &bounds()), 640.0);
    }

    #[test]
    fn pane_minimum_wins_when_window_is_tiny() {
        let tiny = ResizeBounds {
            area: 700.0,
            others: 500.0,
            neighbor_min: PREVIEW_MIN_W,
            rail_w: 40.0,
        };
        // Space guard would allow at most -200; the pane minimum wins.
        assert_eq!(clamp_resize(ResizeTarget::MediaWidth, 500.0, &tiny), 320.0);
        assert_eq!(clamp_resize(ResizeTarget::AgentWidth, 500.0, &tiny), 240.0);
    }

    #[test]
    fn timeline_clamps_to_range_and_preview_height() {
        assert_eq!(
            clamp_resize(ResizeTarget::TimelineHeight, 50.0, &tl_bounds(900.0)),
            100.0
        );
        assert_eq!(
            clamp_resize(ResizeTarget::TimelineHeight, 400.0, &tl_bounds(900.0)),
            400.0
        );
        // area 900 → space max = 900 - 320 = 580 < hard max 700.
        assert_eq!(
            clamp_resize(ResizeTarget::TimelineHeight, 650.0, &tl_bounds(900.0)),
            580.0
        );
        assert_eq!(
            clamp_resize(ResizeTarget::TimelineHeight, 900.0, &tl_bounds(2000.0)),
            700.0
        );
    }

    #[test]
    fn vertical_left_clamps_like_a_media_column() {
        assert_eq!(
            clamp_resize(ResizeTarget::VerticalLeftWidth, 100.0, &bounds()),
            320.0
        );
        assert_eq!(
            clamp_resize(ResizeTarget::VerticalLeftWidth, 900.0, &bounds()),
            700.0
        );
    }

    #[test]
    fn vertical_media_squeezes_against_inspector_minimum() {
        // Inside the Vertical left column (600 wide): media may grow until
        // the inspector hits its 150 minimum.
        let inner = ResizeBounds {
            area: 600.0,
            others: 0.0,
            neighbor_min: INSPECTOR_MIN,
            rail_w: 40.0,
        };
        assert_eq!(clamp_resize(ResizeTarget::MediaWidth, 500.0, &inner), 450.0);
        assert_eq!(clamp_resize(ResizeTarget::MediaWidth, 100.0, &inner), 320.0);
    }

    #[test]
    fn drag_directions_match_divider_edges() {
        assert_eq!(drag_direction(ResizeTarget::AgentWidth), 1.0);
        assert_eq!(drag_direction(ResizeTarget::MediaWidth), 1.0);
        assert_eq!(drag_direction(ResizeTarget::VerticalLeftWidth), 1.0);
        assert_eq!(drag_direction(ResizeTarget::InspectorWidth), -1.0);
        assert_eq!(drag_direction(ResizeTarget::TimelineHeight), -1.0);
    }

    #[test]
    fn initial_timeline_height_follows_preset_fractions() {
        assert_eq!(
            initial_timeline_height(LayoutPreset::Default, 1000.0),
            300.0
        );
        assert_eq!(initial_timeline_height(LayoutPreset::Media, 1000.0), 450.0);
        assert_eq!(
            initial_timeline_height(LayoutPreset::Vertical, 1000.0),
            450.0
        );
        // Clamped into the timeline range on extreme heights.
        assert_eq!(initial_timeline_height(LayoutPreset::Default, 200.0), 100.0);
        assert_eq!(
            initial_timeline_height(LayoutPreset::Media, 3000.0),
            700.0
        );
    }
}

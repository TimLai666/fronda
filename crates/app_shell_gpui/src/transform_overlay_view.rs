//! TransformOverlayView — matches Swift TransformOverlayView.
//!
//! Shows on top of the preview canvas when a single visual clip is selected.
//!
//! Visual elements:
//!   • White bounding box (thin border) at clip position/size/rotation
//!   • 4 white corner handle squares at bbox corners
//!   • Center snap guides: pink vertical and horizontal lines at canvas center
//!
//! Static representation — no gesture support in this view. The actual move/
//! resize interaction would be wired through a gesture layer in gpui.

use crate::theme::{Opacity, Spacing};
use gpui::{
    div, prelude::*, px, relative, App, Context, FocusHandle, Focusable, IntoElement,
    ParentElement, Render, Styled, Window,
};

const HANDLE_SIZE: f32 = Spacing::SM_MD;
const BORDER_W: f32 = 1.0;

/// Transform overlay state. All values are normalized (0.0..=1.0) relative to the canvas.
#[derive(Debug, Clone)]
pub struct TransformOverlayState {
    /// Clip bounding box center X (normalized).
    pub center_x: f32,
    /// Clip bounding box center Y (normalized).
    pub center_y: f32,
    /// Clip bounding box width (normalized to canvas width).
    pub width: f32,
    /// Clip bounding box height (normalized to canvas height).
    pub height: f32,
    /// Show center snap guides.
    pub show_snap_guides: bool,
}

impl Default for TransformOverlayState {
    fn default() -> Self {
        Self {
            center_x: 0.5,
            center_y: 0.5,
            width: 0.6,
            height: 0.6,
            show_snap_guides: true,
        }
    }
}

pub struct TransformOverlayView {
    pub state: TransformOverlayState,
    focus_handle: FocusHandle,
}

impl TransformOverlayView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            state: TransformOverlayState::default(),
            focus_handle: cx.focus_handle(),
        }
    }
}

impl Focusable for TransformOverlayView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

/// White box border at fractional position/size.
fn bbox_border(cx: f32, cy: f32, w: f32, h: f32) -> impl IntoElement {
    let left = cx - w / 2.0;
    let top = cy - h / 2.0;
    div()
        .absolute()
        .left(relative(left))
        .top(relative(top))
        .w(relative(w))
        .h(relative(h))
        .border_1()
        .border_color(gpui::Hsla { h: 0.0, s: 0.0, l: 1.0, a: Opacity::STRONG })
}

/// A single corner handle square.
fn corner_handle(left_frac: f32, top_frac: f32) -> impl IntoElement {
    div()
        .absolute()
        .left(relative(left_frac))
        .top(relative(top_frac))
        .w(px(HANDLE_SIZE))
        .h(px(HANDLE_SIZE))
        .bg(gpui::Hsla { h: 0.0, s: 0.0, l: 1.0, a: Opacity::STRONG })
        .border_1()
        .border_color(gpui::Hsla { h: 0.0, s: 0.0, l: 0.0, a: 0.40 })
}

/// Vertical center snap guide (pink).
fn snap_guide_v() -> impl IntoElement {
    div()
        .absolute()
        .left(relative(0.5))
        .top(px(0.0))
        .w(px(BORDER_W))
        .h(relative(1.0))
        .bg(gpui::Hsla { h: 0.94, s: 1.0, l: 0.60, a: Opacity::PROMINENT })
}

/// Horizontal center snap guide (pink).
fn snap_guide_h() -> impl IntoElement {
    div()
        .absolute()
        .top(relative(0.5))
        .left(px(0.0))
        .w(relative(1.0))
        .h(px(BORDER_W))
        .bg(gpui::Hsla { h: 0.94, s: 1.0, l: 0.60, a: Opacity::PROMINENT })
}

impl Render for TransformOverlayView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let s = &self.state;
        let cx = s.center_x;
        let cy = s.center_y;
        let w = s.width;
        let h = s.height;
        let show_snap = s.show_snap_guides;

        // Corner handle positions (normalized to canvas):
        let left = cx - w / 2.0;
        let top = cy - h / 2.0;
        let right = cx + w / 2.0;
        let bottom = cy + h / 2.0;

        div()
            .id("transform-overlay")
            .track_focus(&self.focus_handle.clone())
            .size_full()
            .relative()
            // Bounding box border
            .child(bbox_border(cx, cy, w, h))
            // Corner handles — offset by half handle size so they center on the corner
            .child(corner_handle(left, top))
            .child(corner_handle(right, top))
            .child(corner_handle(left, bottom))
            .child(corner_handle(right, bottom))
            // Snap guides
            .when(show_snap, |el| {
                el.child(snap_guide_v()).child(snap_guide_h())
            })
    }
}

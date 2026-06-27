//! CropOverlayView — matches Swift CropOverlayView.
//!
//! Shows on top of the preview canvas when the crop tool is active and a clip is selected.
//!
//! Visual elements:
//!   • 4 dark dim bands covering the area outside the crop rect
//!   • Orange border around the crop rect
//!   • Rule-of-thirds grid lines inside the crop rect (4 lines, orange @ 50% opacity)
//!   • 4 orange corner handle squares at the crop rect corners
//!
//! All positions are normalized (0.0..=1.0) relative to the canvas. Static — no gestures.

use crate::theme::{BorderWidth, Opacity, Spacing};
use gpui::{
    div, prelude::*, px, relative, App, Context, FocusHandle, Focusable, IntoElement,
    ParentElement, Render, Styled, Window,
};

/// Orange accent color (matches Swift AppTheme.Accent.timecodeColor).
const ORANGE: gpui::Hsla = gpui::Hsla { h: 0.097, s: 0.90, l: 0.55, a: 1.0 };
const HANDLE_SIZE: f32 = Spacing::SM_MD;

/// Crop overlay state. All rect values normalized 0.0..=1.0 within the canvas.
#[derive(Debug, Clone)]
pub struct CropOverlayState {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl Default for CropOverlayState {
    fn default() -> Self {
        Self { left: 0.1, top: 0.1, right: 0.9, bottom: 0.9 }
    }
}

pub struct CropOverlayView {
    pub state: CropOverlayState,
    focus_handle: FocusHandle,
}

impl CropOverlayView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            state: CropOverlayState::default(),
            focus_handle: cx.focus_handle(),
        }
    }
}

impl Focusable for CropOverlayView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

/// A single dim band (covering outside-crop area). Strong black, fills the band rect.
fn dim_band(left: f32, top: f32, width: f32, height: f32) -> impl IntoElement {
    div()
        .absolute()
        .left(relative(left))
        .top(relative(top))
        .w(relative(width))
        .h(relative(height))
        .bg(gpui::Hsla { h: 0.0, s: 0.0, l: 0.0, a: Opacity::STRONG })
}

/// Orange border rect around the crop area.
fn crop_border(left: f32, top: f32, width: f32, height: f32) -> impl IntoElement {
    div()
        .absolute()
        .left(relative(left))
        .top(relative(top))
        .w(relative(width))
        .h(relative(height))
        .border(px(BorderWidth::MEDIUM))
        .border_color(ORANGE)
}

/// Orange corner handle square.
fn corner_handle(left: f32, top: f32) -> impl IntoElement {
    div()
        .absolute()
        .left(relative(left))
        .top(relative(top))
        .w(px(HANDLE_SIZE))
        .h(px(HANDLE_SIZE))
        .bg(ORANGE)
}

impl Render for CropOverlayView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let s = &self.state;
        let l = s.left;
        let t = s.top;
        let r = s.right;
        let b = s.bottom;
        let cw = r - l; // crop width fraction
        let ch = b - t; // crop height fraction

        // Rule-of-thirds positions within crop rect
        let v1 = l + cw / 3.0;
        let v2 = l + cw * 2.0 / 3.0;
        let h1 = t + ch / 3.0;
        let h2 = t + ch * 2.0 / 3.0;

        div()
            .id("crop-overlay")
            .track_focus(&self.focus_handle.clone())
            .size_full()
            .relative()
            // ── Dim bands (4 rectangles covering outside-crop area) ──
            // Top band
            .child(dim_band(0.0, 0.0, 1.0, t))
            // Bottom band
            .child(dim_band(0.0, b, 1.0, 1.0 - b))
            // Left band (between top and bottom bands)
            .child(dim_band(0.0, t, l, ch))
            // Right band (between top and bottom bands)
            .child(dim_band(r, t, 1.0 - r, ch))
            // ── Orange crop border ──
            .child(crop_border(l, t, cw, ch))
            // ── Rule-of-thirds guides ──
            .child(
                div()
                    .absolute()
                    .left(relative(v1))
                    .top(relative(t))
                    .w(px(BorderWidth::THIN))
                    .h(relative(ch))
                    .bg(gpui::Hsla { h: ORANGE.h, s: ORANGE.s, l: ORANGE.l, a: Opacity::MEDIUM }),
            )
            .child(
                div()
                    .absolute()
                    .left(relative(v2))
                    .top(relative(t))
                    .w(px(BorderWidth::THIN))
                    .h(relative(ch))
                    .bg(gpui::Hsla { h: ORANGE.h, s: ORANGE.s, l: ORANGE.l, a: Opacity::MEDIUM }),
            )
            .child(
                div()
                    .absolute()
                    .left(relative(l))
                    .top(relative(h1))
                    .w(relative(cw))
                    .h(px(BorderWidth::THIN))
                    .bg(gpui::Hsla { h: ORANGE.h, s: ORANGE.s, l: ORANGE.l, a: Opacity::MEDIUM }),
            )
            .child(
                div()
                    .absolute()
                    .left(relative(l))
                    .top(relative(h2))
                    .w(relative(cw))
                    .h(px(BorderWidth::THIN))
                    .bg(gpui::Hsla { h: ORANGE.h, s: ORANGE.s, l: ORANGE.l, a: Opacity::MEDIUM }),
            )
            // ── Corner handles ──
            .child(corner_handle(l, t))
            .child(corner_handle(r, t))
            .child(corner_handle(l, b))
            .child(corner_handle(r, b))
    }
}

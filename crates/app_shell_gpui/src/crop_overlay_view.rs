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
//! Drag: each corner handle initiates a CropDrag; on_drag_move on the root updates state.

use crate::theme::{BorderWidth, Opacity, Spacing};
use gpui::{
    div, prelude::*, px, relative, App, Context, DragMoveEvent, FocusHandle, Focusable,
    IntoElement, MouseButton, MouseDownEvent, ParentElement, Render, Styled, Window,
};

/// Orange accent color (matches Swift AppTheme.Accent.timecodeColor).
const ORANGE: gpui::Hsla = gpui::Hsla {
    h: 0.097,
    s: 0.90,
    l: 0.55,
    a: 1.0,
};
const HANDLE_SIZE: f32 = Spacing::SM_MD;

/// Which corner is being dragged.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CropCorner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// Drag token to activate on_drag_move.
#[derive(Debug, Clone)]
struct CropDrag;

/// Invisible drag preview.
struct CropDragPreview;
impl Render for CropDragPreview {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

/// Active drag session.
#[derive(Debug, Clone)]
struct CropDragSession {
    corner: CropCorner,
    start_x: f32,
    start_y: f32,
    start_state: CropOverlayState,
}

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
        Self {
            left: 0.1,
            top: 0.1,
            right: 0.9,
            bottom: 0.9,
        }
    }
}

pub struct CropOverlayView {
    pub state: CropOverlayState,
    focus_handle: FocusHandle,
    drag: Option<CropDragSession>,
}

impl CropOverlayView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            state: CropOverlayState::default(),
            focus_handle: cx.focus_handle(),
            drag: None,
        }
    }
}

impl Focusable for CropOverlayView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

/// A single dim band (covering outside-crop area).
fn dim_band(left: f32, top: f32, width: f32, height: f32) -> impl IntoElement {
    div()
        .absolute()
        .left(relative(left))
        .top(relative(top))
        .w(relative(width))
        .h(relative(height))
        .bg(gpui::Hsla {
            h: 0.0,
            s: 0.0,
            l: 0.0,
            a: Opacity::STRONG,
        })
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

/// Draggable orange corner handle.
fn corner_handle(
    id: &str,
    left: f32,
    top: f32,
    corner: CropCorner,
    cx: &mut Context<CropOverlayView>,
) -> impl IntoElement {
    div()
        .id(id.to_string())
        .absolute()
        .left(relative(left))
        .top(relative(top))
        .w(px(HANDLE_SIZE))
        .h(px(HANDLE_SIZE))
        .bg(ORANGE)
        .cursor_pointer()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(
                move |this: &mut CropOverlayView, e: &MouseDownEvent, _, _| {
                    this.drag = Some(CropDragSession {
                        corner,
                        start_x: e.position.x.as_f32(),
                        start_y: e.position.y.as_f32(),
                        start_state: this.state.clone(),
                    });
                },
            ),
        )
        .on_drag(CropDrag, |_, _, _, cx| cx.new(|_| CropDragPreview))
}

impl Render for CropOverlayView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let s = &self.state;
        let l = s.left;
        let t = s.top;
        let r = s.right;
        let b = s.bottom;
        let cw = r - l;
        let ch = b - t;

        // Rule-of-thirds positions within crop rect
        let v1 = l + cw / 3.0;
        let v2 = l + cw * 2.0 / 3.0;
        let h1 = t + ch / 3.0;
        let h2 = t + ch * 2.0 / 3.0;

        let weak = cx.entity().downgrade();

        div()
            .id("crop-overlay")
            .track_focus(&self.focus_handle.clone())
            .size_full()
            .relative()
            // on_drag_move fires while a CropDrag is in progress
            .on_drag_move::<CropDrag>(move |event: &DragMoveEvent<CropDrag>, _, cx: &mut App| {
                let _ = weak.update(cx, |this: &mut CropOverlayView, inner_cx| {
                    if let Some(ref session) = this.drag {
                        let dx = event.event.position.x.as_f32() - session.start_x;
                        let dy = event.event.position.y.as_f32() - session.start_y;
                        let st = &session.start_state;
                        const MIN_SIZE: f32 = 0.05;
                        match session.corner {
                            CropCorner::TopLeft => {
                                this.state.left = (st.left + dx).clamp(0.0, st.right - MIN_SIZE);
                                this.state.top = (st.top + dy).clamp(0.0, st.bottom - MIN_SIZE);
                            }
                            CropCorner::TopRight => {
                                this.state.right = (st.right + dx).clamp(st.left + MIN_SIZE, 1.0);
                                this.state.top = (st.top + dy).clamp(0.0, st.bottom - MIN_SIZE);
                            }
                            CropCorner::BottomLeft => {
                                this.state.left = (st.left + dx).clamp(0.0, st.right - MIN_SIZE);
                                this.state.bottom = (st.bottom + dy).clamp(st.top + MIN_SIZE, 1.0);
                            }
                            CropCorner::BottomRight => {
                                this.state.right = (st.right + dx).clamp(st.left + MIN_SIZE, 1.0);
                                this.state.bottom = (st.bottom + dy).clamp(st.top + MIN_SIZE, 1.0);
                            }
                        }
                        inner_cx.notify();
                    }
                });
            })
            // ── Dim bands ──
            .child(dim_band(0.0, 0.0, 1.0, t))
            .child(dim_band(0.0, b, 1.0, 1.0 - b))
            .child(dim_band(0.0, t, l, ch))
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
                    .bg(gpui::Hsla {
                        h: ORANGE.h,
                        s: ORANGE.s,
                        l: ORANGE.l,
                        a: Opacity::MEDIUM,
                    }),
            )
            .child(
                div()
                    .absolute()
                    .left(relative(v2))
                    .top(relative(t))
                    .w(px(BorderWidth::THIN))
                    .h(relative(ch))
                    .bg(gpui::Hsla {
                        h: ORANGE.h,
                        s: ORANGE.s,
                        l: ORANGE.l,
                        a: Opacity::MEDIUM,
                    }),
            )
            .child(
                div()
                    .absolute()
                    .left(relative(l))
                    .top(relative(h1))
                    .w(relative(cw))
                    .h(px(BorderWidth::THIN))
                    .bg(gpui::Hsla {
                        h: ORANGE.h,
                        s: ORANGE.s,
                        l: ORANGE.l,
                        a: Opacity::MEDIUM,
                    }),
            )
            .child(
                div()
                    .absolute()
                    .left(relative(l))
                    .top(relative(h2))
                    .w(relative(cw))
                    .h(px(BorderWidth::THIN))
                    .bg(gpui::Hsla {
                        h: ORANGE.h,
                        s: ORANGE.s,
                        l: ORANGE.l,
                        a: Opacity::MEDIUM,
                    }),
            )
            // ── Corner handles (draggable) ──
            .child(corner_handle("crop-tl", l, t, CropCorner::TopLeft, cx))
            .child(corner_handle("crop-tr", r, t, CropCorner::TopRight, cx))
            .child(corner_handle("crop-bl", l, b, CropCorner::BottomLeft, cx))
            .child(corner_handle("crop-br", r, b, CropCorner::BottomRight, cx))
    }
}

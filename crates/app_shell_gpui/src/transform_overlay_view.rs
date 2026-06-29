//! TransformOverlayView — matches Swift TransformOverlayView.
//!
//! Shows on top of the preview canvas when a single visual clip is selected.
//!
//! Visual elements:
//!   • White bounding box (thin border) at clip position/size/rotation
//!   • 4 white corner handle squares at bbox corners
//!   • 4 white mid-edge handle squares (top/bottom/left/right midpoints)
//!   • Rotation handle: circle above top-center, connected by a stem line
//!   • Center snap guides: pink vertical and horizontal lines at canvas center
//!
//! Drag handles:
//!   • Corner handles resize width and height
//!   • Edge handles resize one axis
//!   • Center area (bbox interior) moves position
//!   • Rotation handle rotates around center (angle from drag delta)

use crate::theme::{Opacity, Spacing};
use gpui::{
    div, prelude::*, px, relative, App, Context, DragMoveEvent, FocusHandle, Focusable,
    IntoElement, MouseButton, MouseDownEvent, ParentElement, Render, Styled, Window,
};

const HANDLE_SIZE: f32 = Spacing::SM_MD;
const BORDER_W: f32 = 1.0;

/// Which part of the transform overlay is being dragged.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransformHandle {
    /// Move the whole clip (drag on bbox interior).
    Move,
    /// Resize from a corner.
    CornerTL,
    CornerTR,
    CornerBL,
    CornerBR,
    /// Resize one axis from an edge midpoint.
    EdgeTop,
    EdgeBottom,
    EdgeLeft,
    EdgeRight,
    /// Rotate around center.
    Rotation,
}

/// Drag token to activate on_drag_move.
#[derive(Debug, Clone)]
struct TransformDrag;

/// Invisible drag preview.
struct TransformDragPreview;
impl Render for TransformDragPreview {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

/// Active drag session.
#[derive(Debug, Clone)]
struct TransformDragSession {
    handle: TransformHandle,
    start_x: f32,
    start_y: f32,
    start_state: TransformOverlayState,
}

/// Transform overlay state. All values normalized (0.0..=1.0) relative to the canvas.
#[derive(Debug, Clone)]
pub struct TransformOverlayState {
    pub center_x: f32,
    pub center_y: f32,
    pub width: f32,
    pub height: f32,
    /// Rotation in turns (0.0..=1.0 = 0°..=360°).
    pub rotation: f32,
    pub show_snap_guides: bool,
}

impl Default for TransformOverlayState {
    fn default() -> Self {
        Self {
            center_x: 0.5,
            center_y: 0.5,
            width: 0.6,
            height: 0.6,
            rotation: 0.0,
            show_snap_guides: true,
        }
    }
}

pub struct TransformOverlayView {
    pub state: TransformOverlayState,
    focus_handle: FocusHandle,
    drag: Option<TransformDragSession>,
}

impl TransformOverlayView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            state: TransformOverlayState::default(),
            focus_handle: cx.focus_handle(),
            drag: None,
        }
    }
}

impl Focusable for TransformOverlayView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

const WHITE: gpui::Hsla = gpui::Hsla {
    h: 0.0,
    s: 0.0,
    l: 1.0,
    a: 1.0,
};
const WHITE_BORDER: gpui::Hsla = gpui::Hsla {
    h: 0.0,
    s: 0.0,
    l: 0.0,
    a: 0.40,
};

/// White bounding box border.
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
        .border_color(gpui::Hsla {
            h: 0.0,
            s: 0.0,
            l: 1.0,
            a: Opacity::STRONG,
        })
}

/// Draggable corner handle.
fn corner_handle(
    id: &str,
    left_frac: f32,
    top_frac: f32,
    handle: TransformHandle,
    cx: &mut Context<TransformOverlayView>,
) -> impl IntoElement {
    div()
        .id(id.to_string())
        .absolute()
        .left(relative(left_frac))
        .top(relative(top_frac))
        .w(px(HANDLE_SIZE))
        .h(px(HANDLE_SIZE))
        .bg(WHITE)
        .border_1()
        .border_color(WHITE_BORDER)
        .cursor_pointer()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(
                move |this: &mut TransformOverlayView, e: &MouseDownEvent, _, _| {
                    this.drag = Some(TransformDragSession {
                        handle,
                        start_x: e.position.x.as_f32(),
                        start_y: e.position.y.as_f32(),
                        start_state: this.state.clone(),
                    });
                },
            ),
        )
        .on_drag(TransformDrag, |_, _, _, cx| {
            cx.new(|_| TransformDragPreview)
        })
}

/// Draggable mid-edge handle.
fn edge_handle(
    id: &str,
    left_frac: f32,
    top_frac: f32,
    handle: TransformHandle,
    cx: &mut Context<TransformOverlayView>,
) -> impl IntoElement {
    let sz = HANDLE_SIZE * 0.75;
    div()
        .id(id.to_string())
        .absolute()
        .left(relative(left_frac))
        .top(relative(top_frac))
        .w(px(sz))
        .h(px(sz))
        .rounded(px(1.5))
        .bg(WHITE)
        .border_1()
        .border_color(WHITE_BORDER)
        .cursor_pointer()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(
                move |this: &mut TransformOverlayView, e: &MouseDownEvent, _, _| {
                    this.drag = Some(TransformDragSession {
                        handle,
                        start_x: e.position.x.as_f32(),
                        start_y: e.position.y.as_f32(),
                        start_state: this.state.clone(),
                    });
                },
            ),
        )
        .on_drag(TransformDrag, |_, _, _, cx| {
            cx.new(|_| TransformDragPreview)
        })
}

/// Draggable rotation handle (stem + circle above top-center).
fn rotation_handle(
    top_center_x: f32,
    top_frac: f32,
    cx: &mut Context<TransformOverlayView>,
) -> impl IntoElement {
    let stem_h = 0.04;
    let handle_r = 5.0;
    div()
        .id("rot-handle")
        .absolute()
        .left(relative(top_center_x))
        .top(relative(top_frac - stem_h - 0.015))
        .flex()
        .flex_col()
        .items_center()
        .cursor_pointer()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(
                move |this: &mut TransformOverlayView, e: &MouseDownEvent, _, _| {
                    this.drag = Some(TransformDragSession {
                        handle: TransformHandle::Rotation,
                        start_x: e.position.x.as_f32(),
                        start_y: e.position.y.as_f32(),
                        start_state: this.state.clone(),
                    });
                },
            ),
        )
        .on_drag(TransformDrag, |_, _, _, cx| {
            cx.new(|_| TransformDragPreview)
        })
        .child(div().w(px(1.0)).h(relative(stem_h)).bg(gpui::Hsla {
            h: 0.0,
            s: 0.0,
            l: 1.0,
            a: Opacity::STRONG,
        }))
        .child(
            div()
                .w(px(handle_r * 2.0))
                .h(px(handle_r * 2.0))
                .rounded_full()
                .bg(WHITE)
                .border_1()
                .border_color(WHITE_BORDER),
        )
}

fn snap_guide_v() -> impl IntoElement {
    div()
        .absolute()
        .left(relative(0.5))
        .top(px(0.0))
        .w(px(BORDER_W))
        .h(relative(1.0))
        .bg(gpui::Hsla {
            h: 0.94,
            s: 1.0,
            l: 0.60,
            a: Opacity::PROMINENT,
        })
}

fn snap_guide_h() -> impl IntoElement {
    div()
        .absolute()
        .top(relative(0.5))
        .left(px(0.0))
        .w(relative(1.0))
        .h(px(BORDER_W))
        .bg(gpui::Hsla {
            h: 0.94,
            s: 1.0,
            l: 0.60,
            a: Opacity::PROMINENT,
        })
}

impl Render for TransformOverlayView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let s = &self.state;
        let cx_val = s.center_x;
        let cy_val = s.center_y;
        let w = s.width;
        let h = s.height;
        let show_snap = s.show_snap_guides;

        let left = cx_val - w / 2.0;
        let top = cy_val - h / 2.0;
        let right = cx_val + w / 2.0;
        let bottom = cy_val + h / 2.0;
        let mid_x = cx_val;
        let mid_y = cy_val;

        let weak = cx.entity().downgrade();

        div()
            .id("transform-overlay")
            .track_focus(&self.focus_handle.clone())
            .size_full()
            .relative()
            // Global drag-move handler — updates state for whichever handle is active
            .on_drag_move::<TransformDrag>(
                move |event: &DragMoveEvent<TransformDrag>, _, cx: &mut App| {
                    let _ = weak.update(cx, |this: &mut TransformOverlayView, inner_cx| {
                        if let Some(ref session) = this.drag {
                            let dx = event.event.position.x.as_f32() - session.start_x;
                            let dy = event.event.position.y.as_f32() - session.start_y;
                            let st = &session.start_state;
                            const MIN: f32 = 0.05;
                            match session.handle {
                                TransformHandle::Move => {
                                    this.state.center_x = (st.center_x + dx).clamp(0.0, 1.0);
                                    this.state.center_y = (st.center_y + dy).clamp(0.0, 1.0);
                                }
                                TransformHandle::CornerTL => {
                                    this.state.width = (st.width - dx).max(MIN);
                                    this.state.height = (st.height - dy).max(MIN);
                                    this.state.center_x = st.center_x + dx / 2.0;
                                    this.state.center_y = st.center_y + dy / 2.0;
                                }
                                TransformHandle::CornerTR => {
                                    this.state.width = (st.width + dx).max(MIN);
                                    this.state.height = (st.height - dy).max(MIN);
                                    this.state.center_x = st.center_x + dx / 2.0;
                                    this.state.center_y = st.center_y + dy / 2.0;
                                }
                                TransformHandle::CornerBL => {
                                    this.state.width = (st.width - dx).max(MIN);
                                    this.state.height = (st.height + dy).max(MIN);
                                    this.state.center_x = st.center_x + dx / 2.0;
                                    this.state.center_y = st.center_y + dy / 2.0;
                                }
                                TransformHandle::CornerBR => {
                                    this.state.width = (st.width + dx).max(MIN);
                                    this.state.height = (st.height + dy).max(MIN);
                                    this.state.center_x = st.center_x + dx / 2.0;
                                    this.state.center_y = st.center_y + dy / 2.0;
                                }
                                TransformHandle::EdgeTop => {
                                    this.state.height = (st.height - dy).max(MIN);
                                    this.state.center_y = st.center_y + dy / 2.0;
                                }
                                TransformHandle::EdgeBottom => {
                                    this.state.height = (st.height + dy).max(MIN);
                                    this.state.center_y = st.center_y + dy / 2.0;
                                }
                                TransformHandle::EdgeLeft => {
                                    this.state.width = (st.width - dx).max(MIN);
                                    this.state.center_x = st.center_x + dx / 2.0;
                                }
                                TransformHandle::EdgeRight => {
                                    this.state.width = (st.width + dx).max(MIN);
                                    this.state.center_x = st.center_x + dx / 2.0;
                                }
                                TransformHandle::Rotation => {
                                    // Map horizontal drag to rotation (360° = 1.0 turn over ~300px)
                                    this.state.rotation =
                                        (st.rotation + dx / 300.0).rem_euclid(1.0);
                                }
                            }
                            inner_cx.notify();
                        }
                    });
                },
            )
            // Bounding box border
            .child(bbox_border(cx_val, cy_val, w, h))
            // Draggable move area (bbox interior, transparent)
            .child(
                div()
                    .id("bbox-move")
                    .absolute()
                    .left(relative(left + 0.01))
                    .top(relative(top + 0.01))
                    .w(relative((w - 0.02).max(0.0)))
                    .h(relative((h - 0.02).max(0.0)))
                    .cursor_pointer()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(
                            move |this: &mut TransformOverlayView, e: &MouseDownEvent, _, _| {
                                this.drag = Some(TransformDragSession {
                                    handle: TransformHandle::Move,
                                    start_x: e.position.x.as_f32(),
                                    start_y: e.position.y.as_f32(),
                                    start_state: this.state.clone(),
                                });
                            },
                        ),
                    )
                    .on_drag(TransformDrag, |_, _, _, cx| {
                        cx.new(|_| TransformDragPreview)
                    }),
            )
            // Corner handles
            .child(corner_handle(
                "tf-tl",
                left,
                top,
                TransformHandle::CornerTL,
                cx,
            ))
            .child(corner_handle(
                "tf-tr",
                right,
                top,
                TransformHandle::CornerTR,
                cx,
            ))
            .child(corner_handle(
                "tf-bl",
                left,
                bottom,
                TransformHandle::CornerBL,
                cx,
            ))
            .child(corner_handle(
                "tf-br",
                right,
                bottom,
                TransformHandle::CornerBR,
                cx,
            ))
            // Mid-edge handles
            .child(edge_handle(
                "tf-et",
                mid_x,
                top,
                TransformHandle::EdgeTop,
                cx,
            ))
            .child(edge_handle(
                "tf-eb",
                mid_x,
                bottom,
                TransformHandle::EdgeBottom,
                cx,
            ))
            .child(edge_handle(
                "tf-el",
                left,
                mid_y,
                TransformHandle::EdgeLeft,
                cx,
            ))
            .child(edge_handle(
                "tf-er",
                right,
                mid_y,
                TransformHandle::EdgeRight,
                cx,
            ))
            // Rotation handle above top-center
            .child(rotation_handle(mid_x, top, cx))
            // Snap guides
            .when(show_snap, |el| {
                el.child(snap_guide_v()).child(snap_guide_h())
            })
    }
}

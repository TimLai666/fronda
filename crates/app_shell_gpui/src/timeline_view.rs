//! Timeline panel gpui view — ruler, track headers, and clip area.
//!
//! Covers UIX-009 (track sizes), UIX-010 (layout constants), from spec 07.

use crate::theme::{
    Background, BorderColors, BorderWidth, Layout, Spacing, Text, TrackColor,
};
use crate::timeline_model::{TimelineState, TrackKind, DEFAULT_PIXELS_PER_FRAME, RULER_HEIGHT, TRACK_HEADER_WIDTH};
use gpui::{
    div, prelude::*, px, App, Context, FocusHandle, Focusable, IntoElement,
    InteractiveElement, ParentElement, Render, Styled, Window,
};

/// Timeline panel gpui entity.
pub struct TimelineView {
    pub state: TimelineState,
    focus_handle: FocusHandle,
}

impl TimelineView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            state: TimelineState::new().with_default_tracks(),
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn zoom_in(&mut self, cx: &mut Context<Self>) {
        self.state.zoom_in();
        cx.notify();
    }

    pub fn zoom_out(&mut self, cx: &mut Context<Self>) {
        self.state.zoom_out();
        cx.notify();
    }
}

impl Focusable for TimelineView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TimelineView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let tracks = self.state.tracks.clone();
        let zoom = self.state.zoom_scale;
        let total_frames = self.state.total_frames;
        let fps = self.state.fps;
        let playhead_frame = self.state.playhead_frame;

        div()
            .id("timeline")
            .flex()
            .flex_col()
            .size_full()
            .bg(Background::SURFACE)
            // ── Top: ruler row ──
            .child(
                div()
                    .id("ruler-row")
                    .flex()
                    .flex_row()
                    .h(px(RULER_HEIGHT))
                    .w_full()
                    .border_b_1()
                    .border_color(BorderColors::PRIMARY)
                    // Track header spacer (aligns with track headers below)
                    .child(
                        div()
                            .w(px(TRACK_HEADER_WIDTH))
                            .h_full()
                            .border_r_1()
                            .border_color(BorderColors::PRIMARY)
                            .bg(Background::RAISED),
                    )
                    // Ruler content
                    .child(
                        div()
                            .id("ruler")
                            .flex_1()
                            .h_full()
                            .overflow_hidden()
                            .bg(Background::RAISED)
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_end()
                                    .h_full()
                                    .px(px(Spacing::XS))
                                    .text_color(Text::MUTED)
                                    .child(ruler_timecodes(total_frames, fps, zoom)),
                            ),
                    ),
            )
            // ── Track area ──
            .child(
                div()
                    .id("track-area")
                    .flex()
                    .flex_row()
                    .flex_1()
                    .overflow_hidden()
                    // Track header column
                    .child(
                        div()
                            .id("track-headers")
                            .w(px(TRACK_HEADER_WIDTH))
                            .h_full()
                            .flex()
                            .flex_col()
                            .border_r_1()
                            .border_color(BorderColors::PRIMARY)
                            .bg(Background::RAISED)
                            .children(tracks.iter().map(|track| {
                                let color = match track.kind {
                                    TrackKind::Video => TrackColor::VIDEO,
                                    TrackKind::Audio => TrackColor::AUDIO,
                                };
                                div()
                                    .id(format!("header-{}", track.id))
                                    .w_full()
                                    .h(px(track.height))
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .border_b_1()
                                    .border_color(BorderColors::SUBTLE)
                                    // Color strip (left 4px)
                                    .child(
                                        div()
                                            .w(px(4.0))
                                            .h_full()
                                            .bg(color),
                                    )
                                    // Track label
                                    .child(
                                        div()
                                            .flex_1()
                                            .px(px(Spacing::XS))
                                            .text_color(Text::SECONDARY)
                                            .child(track.label.clone()),
                                    )
                                    // Mute/hide buttons placeholder
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .gap(px(Spacing::XXS))
                                            .px(px(Spacing::XXS))
                                            .text_color(Text::MUTED)
                                            .child("M")
                                            .child("H"),
                                    )
                            })),
                    )
                    // Scrollable clip area
                    .child(
                        div()
                            .id("clip-canvas")
                            .flex_1()
                            .h_full()
                            .overflow_hidden()
                            .bg(Background::SURFACE)
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .w_full()
                                    .children(tracks.iter().map(|track| {
                                        div()
                                            .id(format!("track-lane-{}", track.id))
                                            .w_full()
                                            .h(px(track.height))
                                            .border_b_1()
                                            .border_color(BorderColors::SUBTLE)
                                            .bg(Background::SURFACE)
                                            // Playhead indicator placeholder
                                            .child(
                                                div()
                                                    .absolute()
                                                    .top(px(0.0))
                                                    .left(px(playhead_frame as f32 * zoom))
                                                    .w(px(BorderWidth::THIN))
                                                    .h_full()
                                                    .bg(crate::theme::Accent::TIMECODE),
                                            )
                                    })),
                            ),
                    ),
            )
    }
}

/// Generate ruler tick labels for visible timecodes.
fn ruler_timecodes(total_frames: i64, fps: i64, zoom: f32) -> impl IntoElement {
    let fps = fps.max(1);
    // Show a tick every N frames such that ticks are ~80px apart
    let tick_spacing_px = 80.0_f32;
    let frames_per_tick = ((tick_spacing_px / zoom).round() as i64).max(1);

    // Round to a nice interval (1, 5, 10, 30, 60, 120, ...)
    let frames_per_tick = round_to_nice_interval(frames_per_tick, fps);

    let mut root = div().id("ruler-ticks").flex().flex_row().items_end().h_full();

    let mut frame = 0i64;
    while frame <= total_frames.max(0) {
        let x = frame as f32 * zoom;
        let secs = frame / fps;
        let mins = secs / 60;
        let remaining_secs = secs % 60;
        let label = format!("{mins}:{remaining_secs:02}");

        root = root.child(
            div()
                .absolute()
                .left(px(x))
                .bottom(px(2.0))
                .text_color(Text::MUTED)
                .child(label),
        );
        frame += frames_per_tick;
    }
    root
}

fn round_to_nice_interval(raw: i64, fps: i64) -> i64 {
    let candidates = [1, 5, 10, 30, fps, fps * 2, fps * 5, fps * 10, fps * 30, fps * 60];
    *candidates.iter().find(|&&c| c >= raw).unwrap_or(&raw)
}

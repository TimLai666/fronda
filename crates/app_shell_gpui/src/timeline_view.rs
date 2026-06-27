//! Timeline panel gpui view — ruler, track headers, and clip area.
//!
//! Covers UIX-009 (track sizes), UIX-010 (layout constants), from spec 07.

use crate::theme::{
    Accent, Background, BorderColors, BorderWidth, FontSize, Radius, Spacing, Text, TrackColor,
};
use crate::timeline_model::{
    TimelineState, TrackKind, RULER_HEIGHT, TRACK_HEADER_WIDTH,
};
use gpui::{
    div, prelude::*, px, svg, App, Context, FocusHandle, Focusable, IntoElement,
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
        let clips = self.state.clips.clone();
        let zoom = self.state.zoom_scale;
        let total_frames = self.state.total_frames;
        let fps = self.state.fps;
        let playhead_frame = self.state.playhead_frame;
        let scroll_x = self.state.scroll_x;

        let playhead_x = playhead_frame as f32 * zoom - scroll_x;
        let snap_x = self.state.snap_x_frame.map(|f| f as f32 * zoom - scroll_x);
        let has_video = tracks.iter().any(|t| t.kind == TrackKind::Video);
        let has_audio = tracks.iter().any(|t| t.kind == TrackKind::Audio);
        let show_zone_divider = has_video && has_audio;

        div()
            .id("timeline")
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .bg(Background::SURFACE)
            // ── Ruler row ──
            .child(
                div()
                    .id("ruler-row")
                    .flex()
                    .flex_row()
                    .h(px(RULER_HEIGHT))
                    .w_full()
                    .border_b_1()
                    .border_color(BorderColors::PRIMARY)
                    .child(
                        div()
                            .w(px(TRACK_HEADER_WIDTH))
                            .h_full()
                            .border_r_1()
                            .border_color(BorderColors::PRIMARY)
                            .bg(Background::RAISED),
                    )
                    .child(
                        div()
                            .id("ruler")
                            .flex_1()
                            .h_full()
                            .overflow_hidden()
                            .relative()
                            .bg(Background::RAISED)
                            .child(ruler_timecodes(total_frames, fps, zoom, scroll_x))
                            // Playhead in ruler: triangle head + thin line
                            .child(
                                div()
                                    .absolute()
                                    .top_0()
                                    .left(px(playhead_x))
                                    .flex()
                                    .flex_col()
                                    .items_center()
                                    // Triangle head ▼ at top of ruler
                                    .child(
                                        div()
                                            .text_color(Accent::TIMECODE)
                                            .text_size(px(FontSize::XS))
                                            .child("▾"),
                                    )
                                    // Hairline below
                                    .child(
                                        div()
                                            .w(px(BorderWidth::MEDIUM))
                                            .flex_1()
                                            .bg(Accent::TIMECODE),
                                    ),
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
                            .children(tracks.iter().enumerate().map(|(i, track)| {
                                let color = match track.kind {
                                    TrackKind::Video => TrackColor::VIDEO,
                                    TrackKind::Audio => TrackColor::AUDIO,
                                };
                                let is_first_audio = track.kind == TrackKind::Audio
                                    && i > 0
                                    && tracks
                                        .get(i - 1)
                                        .map(|t| t.kind == TrackKind::Video)
                                        .unwrap_or(false);
                                div()
                                    .id(format!("header-{}", track.id))
                                    .w_full()
                                    .h(px(track.height))
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .border_b_1()
                                    .border_color(BorderColors::SUBTLE)
                                    .when(is_first_audio && show_zone_divider, |el| {
                                        el.border_t_2()
                                            .border_color(BorderColors::DIVIDER)
                                    })
                                    // Color strip: 3px to match Swift
                                    .child(div().w(px(3.0)).h_full().bg(color))
                                    .child(
                                        div()
                                            .flex_1()
                                            .px(px(Spacing::XS))
                                            .text_color(Text::SECONDARY)
                                            .text_size(px(FontSize::SM))
                                            .child(track.label.clone()),
                                    )
                                    // Mute / hide icon buttons (Swift: speaker.slash + eye.slash)
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .gap(px(Spacing::XXS))
                                            .px(px(Spacing::XXS))
                                            .child(
                                                svg()
                                                    .path("icons/speaker_slash.svg")
                                                    .w(px(10.0))
                                                    .h(px(10.0))
                                                    .text_color(Text::MUTED),
                                            )
                                            .child(
                                                svg()
                                                    .path("icons/eye_slash.svg")
                                                    .w(px(10.0))
                                                    .h(px(10.0))
                                                    .text_color(Text::MUTED),
                                            ),
                                    )
                            })),
                    )
                    // Clip canvas
                    .child(
                        div()
                            .id("clip-canvas")
                            .flex_1()
                            .h_full()
                            .overflow_hidden()
                            .relative()
                            .bg(Background::SURFACE)
                            .child(
                                div()
                                    .absolute()
                                    .top_0()
                                    .left_0()
                                    .right_0()
                                    .bottom_0()
                                    .flex()
                                    .flex_col()
                                    .children(tracks.iter().enumerate().map(|(i, track)| {
                                        let color = match track.kind {
                                            TrackKind::Video => TrackColor::VIDEO,
                                            TrackKind::Audio => TrackColor::AUDIO,
                                        };
                                        let is_first_audio = track.kind == TrackKind::Audio
                                            && i > 0
                                            && tracks
                                                .get(i - 1)
                                                .map(|t| t.kind == TrackKind::Video)
                                                .unwrap_or(false);
                                        let track_clips: Vec<_> = clips
                                            .iter()
                                            .filter(|c| c.track_id == track.id)
                                            .collect();
                                        let track_height = track.height;
                                        div()
                                            .id(format!("lane-{}", track.id))
                                            .w_full()
                                            .h(px(track_height))
                                            .border_b_1()
                                            .border_color(BorderColors::SUBTLE)
                                            .relative()
                                            .bg(Background::SURFACE)
                                            .when(is_first_audio && show_zone_divider, |el| {
                                                el.border_t_2()
                                                    .border_color(BorderColors::DIVIDER)
                                            })
                                            .children(track_clips.iter().map(|clip| {
                                                let clip_x =
                                                    clip.start_frame as f32 * zoom - scroll_x;
                                                let clip_w =
                                                    (clip.duration_frames as f32 * zoom).max(4.0);
                                                let mut bg = color;
                                                bg.a = 0.22;
                                                div()
                                                    .id(format!("clip-{}", clip.id))
                                                    .absolute()
                                                    .top(px(1.0))
                                                    .left(px(clip_x))
                                                    .w(px(clip_w))
                                                    .h(px(track_height - 2.0))
                                                    .rounded(px(Radius::XS))
                                                    .bg(bg)
                                                    .border_1()
                                                    .border_color(color)
                                                    .overflow_hidden()
                                                    .child(
                                                        div()
                                                            .px(px(Spacing::XS))
                                                            .pt(px(Spacing::XXS))
                                                            .text_size(px(FontSize::SM))
                                                            .text_color(Text::PRIMARY)
                                                            .child(clip.label.clone()),
                                                    )
                                            }))
                                    })),
                            )
                            // Playhead line over clip area
                            .child(
                                div()
                                    .id("playhead-line")
                                    .absolute()
                                    .top_0()
                                    .left(px(playhead_x))
                                    .w(px(BorderWidth::MEDIUM))
                                    .h_full()
                                    .bg(Accent::TIMECODE),
                            )
                            // Snap indicator — dashed yellow vertical line during clip drag
                            // Mirrors Swift SnapIndicatorOverlay (CAShapeLayer with dashes).
                            // Approximated as alternating solid segments (gpui has no native dash).
                            .when_some(snap_x, |el, sx| {
                                el.child(snap_indicator(sx))
                            }),
                    ),
            )
    }
}

/// Dashed yellow snap line: alternating 4px solid / 4px gap segments.
fn snap_indicator(x: f32) -> impl IntoElement {
    const DASH: f32 = 4.0;
    const GAP: f32 = 4.0;
    const TOTAL: usize = 40; // covers up to 320px height
    const YELLOW: gpui::Hsla = gpui::Hsla { h: 0.139, s: 1.0, l: 0.55, a: 1.0 };

    let mut col = div()
        .id("snap-indicator")
        .absolute()
        .top_0()
        .left(px(x))
        .flex()
        .flex_col()
        .w(px(BorderWidth::MEDIUM));

    for i in 0..TOTAL {
        col = col.child(
            div()
                .w_full()
                .h(px(DASH))
                .when(i % 2 == 0, |el| el.bg(YELLOW))
                .when(i % 2 != 0, |el| el.bg(gpui::Hsla { h: 0.0, s: 0.0, l: 0.0, a: 0.0 })),
        );
        col = col.child(div().w_full().h(px(GAP)));
    }
    col
}

/// Generate ruler tick labels for visible timecodes.
fn ruler_timecodes(
    total_frames: i64,
    fps: i64,
    zoom: f32,
    scroll_x: f32,
) -> impl IntoElement {
    let fps = fps.max(1);
    let tick_spacing_px = 80.0_f32;
    let frames_per_tick = ((tick_spacing_px / zoom).round() as i64).max(1);
    let frames_per_tick = round_to_nice_interval(frames_per_tick, fps);

    let start_frame = (scroll_x / zoom) as i64;
    let start_tick = (start_frame / frames_per_tick).max(0) * frames_per_tick;

    let mut root = div()
        .id("ruler-ticks")
        .relative()
        .w_full()
        .h_full();

    let visible_frames = (1200.0_f32 / zoom) as i64 + frames_per_tick * 2;
    let mut frame = start_tick;
    while frame <= (start_tick + visible_frames).min(total_frames + frames_per_tick) {
        let x = frame as f32 * zoom - scroll_x;
        let secs = frame / fps;
        let mins = secs / 60;
        let rem = secs % 60;
        let label = format!("{mins}:{rem:02}");
        root = root.child(
            div()
                .absolute()
                .left(px(x))
                .bottom(px(3.0))
                .text_color(Text::MUTED)
                .text_size(px(FontSize::XS))
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

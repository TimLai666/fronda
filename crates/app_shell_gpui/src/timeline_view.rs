//! Timeline panel gpui view — ruler, track headers, and clip area.
//!
//! Covers UIX-009 (track sizes), UIX-010 (layout constants), from spec 07.

use crate::theme::{
    Accent, Background, BorderColors, BorderWidth, FontSize, Radius, Spacing, Text, TrackColor,
};
use crate::timeline_model::{TimelineState, TrackKind, RULER_HEIGHT, TRACK_HEADER_WIDTH};
use gpui::{
    canvas, div, prelude::*, px, svg, App, Context, DragMoveEvent, FocusHandle, Focusable,
    InteractiveElement, IntoElement, MouseButton, MouseDownEvent, ParentElement, Render, Styled,
    Window,
};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

/// Drag token for clip moves.
#[derive(Debug, Clone)]
struct ClipDragToken;

/// Invisible drag preview.
struct ClipDragPreview;
impl Render for ClipDragPreview {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

/// Timeline panel gpui entity.
pub struct TimelineView {
    pub state: TimelineState,
    focus_handle: FocusHandle,
    /// Last seen shared-state revision; project loads and MCP mutations
    /// trigger a data rebuild.
    state_revision: u64,
    /// Window x of the shared content left edge (ruler / clip canvas),
    /// captured each frame by a zero-size canvas element.
    content_origin_x: Arc<AtomicU32>,
}

impl TimelineView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let mut view = Self {
            state: TimelineState::new(),
            focus_handle: cx.focus_handle(),
            state_revision: u64::MAX,
            content_origin_x: Arc::new(AtomicU32::new(0f32.to_bits())),
        };
        view.sync_from_shared_state();
        view
    }

    fn content_x(&self, window_x: f32) -> f32 {
        window_x - f32::from_bits(self.content_origin_x.load(Ordering::Relaxed))
    }

    fn pointer_frame(&self, window_x: f32) -> i64 {
        self.state
            .frame_for_x(self.state.scroll_x + self.content_x(window_x))
    }

    /// Run a tool on the shared executor; tool errors leave the UI unchanged.
    fn run_shared_tool(tool: &str, args: serde_json::Value) {
        let executor = crate::editor_state_hub::EditorStateHub::global().executor();
        let guard = executor.lock();
        if let Ok(mut exec) = guard {
            if let Err(reason) = exec.execute(tool, &args) {
                eprintln!("{tool} failed: {reason}");
            }
        }
    }

    /// Undo/Redo via the shared executor (menu wiring). Errors (e.g. empty
    /// stack) are normal and silent.
    pub fn run_history_tool(tool: &str) {
        let executor = crate::editor_state_hub::EditorStateHub::global().executor();
        let guard = executor.lock();
        if let Ok(mut exec) = guard {
            let _ = exec.execute(tool, &serde_json::json!({}));
        }
    }

    /// Delete the selected clips through the shared executor.
    pub fn delete_selected(&mut self, cx: &mut Context<Self>) {
        if self.state.selected_clip_ids.is_empty() {
            return;
        }
        let ids = self.state.selected_clip_ids.clone();
        Self::run_shared_tool("remove_clips", serde_json::json!({ "clipIds": ids }));
        self.state.clear_selection();
        cx.notify();
    }

    /// Split each selected clip at the playhead through the shared executor.
    pub fn split_selected_at_playhead(&mut self, cx: &mut Context<Self>) {
        let playhead = self.state.playhead_frame;
        for id in self.state.selected_clip_ids.clone() {
            Self::run_shared_tool(
                "split_clip",
                serde_json::json!({ "clipId": id, "frame": playhead }),
            );
        }
        self.state.clear_selection();
        cx.notify();
    }

    /// Commit a finished clip drag through move_clips (undo-tracked).
    fn commit_clip_drag(&mut self, cx: &mut Context<Self>) {
        let Some((clip_id, to_frame)) = self.state.take_clip_drag() else {
            cx.notify();
            return;
        };
        let Some(to_track) = self.state.track_index_of_clip(&clip_id) else {
            return;
        };
        Self::run_shared_tool(
            "move_clips",
            serde_json::json!({
                "clipIds": [clip_id],
                "toTrack": to_track,
                "toFrame": to_frame,
            }),
        );
        cx.notify();
    }

    /// Rebuild data fields from the shared editor state, preserving
    /// view-only state (zoom, scroll, playhead).
    fn sync_from_shared_state(&mut self) -> bool {
        let hub = crate::editor_state_hub::EditorStateHub::global();
        let revision = hub.revision();
        if revision == self.state_revision {
            return false;
        }
        self.state_revision = revision;

        let executor = hub.executor();
        let Ok(exec) = executor.lock() else {
            return false;
        };
        let mut next = TimelineState::from_core(exec.timeline(), exec.media_manifest());
        next.zoom_scale = self.state.zoom_scale;
        next.scroll_x = self.state.scroll_x;
        next.scroll_y = self.state.scroll_y;
        next.playhead_frame = self.state.playhead_frame;
        self.state = next;
        true
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
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.sync_from_shared_state() {
            cx.notify();
        }
        let tracks = self.state.tracks.clone();
        let clips = self.state.clips.clone();
        let selected_ids = self.state.selected_clip_ids.clone();
        let drag_clip_id = self.state.clip_drag.as_ref().map(|d| d.clip_id.clone());
        let drag_proposed_start = self.state.clip_drag.as_ref().map(|d| d.proposed_start);
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
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, e: &MouseDownEvent, _, cx| {
                                    let x = this.content_x(e.position.x.as_f32());
                                    this.state.scrub_to_content_x(x);
                                    cx.notify();
                                }),
                            )
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
                                        el.border_t_2().border_color(BorderColors::DIVIDER)
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
                            .child({
                                let origin = self.content_origin_x.clone();
                                canvas(
                                    move |bounds, _, _| {
                                        origin.store(
                                            bounds.origin.x.as_f32().to_bits(),
                                            Ordering::Relaxed,
                                        );
                                    },
                                    |_, _, _, _| {},
                                )
                                .absolute()
                                .size_full()
                            })
                            .on_drag_move::<ClipDragToken>(cx.listener(
                                |this, e: &DragMoveEvent<ClipDragToken>, _, cx| {
                                    let content_x =
                                        e.event.position.x.as_f32() - e.bounds.origin.x.as_f32();
                                    let frame =
                                        this.state.frame_for_x(this.state.scroll_x + content_x);
                                    this.state.update_clip_drag(frame);
                                    cx.notify();
                                },
                            ))
                            .on_drop::<ClipDragToken>(cx.listener(|this, _, _, cx| {
                                this.commit_clip_drag(cx);
                            }))
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
                                                el.border_t_2().border_color(BorderColors::DIVIDER)
                                            })
                                            .children(track_clips.iter().map(|clip| {
                                                let is_dragging = drag_clip_id.as_deref()
                                                    == Some(clip.id.as_str());
                                                let start = if is_dragging {
                                                    drag_proposed_start.unwrap_or(clip.start_frame)
                                                } else {
                                                    clip.start_frame
                                                };
                                                let clip_x = start as f32 * zoom - scroll_x;
                                                let clip_w =
                                                    (clip.duration_frames as f32 * zoom).max(4.0);
                                                let is_selected =
                                                    selected_ids.iter().any(|id| id == &clip.id);
                                                let mut bg = color;
                                                bg.a = if is_selected { 0.38 } else { 0.22 };
                                                let clip_id = clip.id.clone();
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
                                                    .border_color(if is_selected {
                                                        Accent::PRIMARY
                                                    } else {
                                                        color
                                                    })
                                                    .overflow_hidden()
                                                    .cursor_pointer()
                                                    .on_mouse_down(
                                                        MouseButton::Left,
                                                        cx.listener(
                                                            move |this,
                                                                  e: &MouseDownEvent,
                                                                  _,
                                                                  cx| {
                                                                this.state
                                                                    .select_only(&clip_id);
                                                                let frame = this.pointer_frame(
                                                                    e.position.x.as_f32(),
                                                                );
                                                                this.state.begin_clip_drag(
                                                                    &clip_id, frame,
                                                                );
                                                                cx.notify();
                                                            },
                                                        ),
                                                    )
                                                    .on_drag(ClipDragToken, |_, _, _, cx| {
                                                        cx.new(|_| ClipDragPreview)
                                                    })
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
                            .when_some(snap_x, |el, sx| el.child(snap_indicator(sx))),
                    ),
            )
    }
}

/// Dashed yellow snap line: alternating 4px solid / 4px gap segments.
fn snap_indicator(x: f32) -> impl IntoElement {
    const DASH: f32 = 4.0;
    const GAP: f32 = 4.0;
    const TOTAL: usize = 40; // covers up to 320px height
    const YELLOW: gpui::Hsla = gpui::Hsla {
        h: 0.139,
        s: 1.0,
        l: 0.55,
        a: 1.0,
    };

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
                .when(i % 2 != 0, |el| {
                    el.bg(gpui::Hsla {
                        h: 0.0,
                        s: 0.0,
                        l: 0.0,
                        a: 0.0,
                    })
                }),
        );
        col = col.child(div().w_full().h(px(GAP)));
    }
    col
}

/// Generate ruler tick labels for visible timecodes.
fn ruler_timecodes(total_frames: i64, fps: i64, zoom: f32, scroll_x: f32) -> impl IntoElement {
    let fps = fps.max(1);
    let tick_spacing_px = 80.0_f32;
    let frames_per_tick = ((tick_spacing_px / zoom).round() as i64).max(1);
    let frames_per_tick = round_to_nice_interval(frames_per_tick, fps);

    let start_frame = (scroll_x / zoom) as i64;
    let start_tick = (start_frame / frames_per_tick).max(0) * frames_per_tick;

    let mut root = div().id("ruler-ticks").relative().w_full().h_full();

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
    let candidates = [
        1,
        5,
        10,
        30,
        fps,
        fps * 2,
        fps * 5,
        fps * 10,
        fps * 30,
        fps * 60,
    ];
    *candidates.iter().find(|&&c| c >= raw).unwrap_or(&raw)
}

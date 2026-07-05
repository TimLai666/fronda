//! Timeline panel gpui view — ruler, track headers, and clip area.
//!
//! Covers UIX-009 (track sizes), UIX-010 (layout constants), from spec 07.

use crate::theme::{
    Accent, Background, BorderColors, BorderWidth, FontSize, Radius, Spacing, Text, TrackColor,
};
use crate::timeline_model::{TimelineState, TrackKind, TrimEdge, RULER_HEIGHT, TRACK_HEADER_WIDTH};
use gpui::{
    canvas, div, prelude::*, px, svg, App, Context, DragMoveEvent, FocusHandle, Focusable,
    InteractiveElement, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, ParentElement,
    Render, Styled, Window,
};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

/// Drag token for clip moves.
#[derive(Debug, Clone)]
struct ClipDragToken;

/// Drag token for clip edge trims.
#[derive(Debug, Clone)]
struct TrimDragToken;

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
    /// True while the transport ticker task loop is alive.
    ticker_running: bool,
    /// Timeline tab being renamed inline: (timeline id, text in progress).
    tab_editing: Option<(String, String)>,
}

impl TimelineView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let mut view = Self {
            state: TimelineState::new(),
            focus_handle: cx.focus_handle(),
            state_revision: u64::MAX,
            content_origin_x: Arc::new(AtomicU32::new(0f32.to_bits())),
            ticker_running: false,
            tab_editing: None,
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
        let Some((clip_id, to_track, to_frame)) = self.state.take_clip_drag() else {
            cx.notify();
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

    /// Apply a boundary change to one clip through the shared executor.
    /// End edge: durationFrames. Start edge: durationFrames then move_clips
    /// (two undo steps — merging them needs a composite tool, an MCP
    /// contract change out of scope here).
    fn apply_trim(&mut self, clip_id: &str, edge: TrimEdge, boundary: i64) {
        let Some(clip) = self.state.clips.iter().find(|c| c.id == clip_id) else {
            return;
        };
        let start = clip.start_frame;
        let end = start + clip.duration_frames;
        match edge {
            TrimEdge::End => {
                Self::run_shared_tool(
                    "set_clip_properties",
                    serde_json::json!({
                        "clipIds": [clip_id],
                        "properties": { "durationFrames": boundary - start },
                    }),
                );
            }
            TrimEdge::Start => {
                let Some(track) = self.state.track_index_of_clip(clip_id) else {
                    return;
                };
                Self::run_shared_tool(
                    "set_clip_properties",
                    serde_json::json!({
                        "clipIds": [clip_id],
                        "properties": { "durationFrames": end - boundary },
                    }),
                );
                Self::run_shared_tool(
                    "move_clips",
                    serde_json::json!({
                        "clipIds": [clip_id],
                        "toTrack": track,
                        "toFrame": boundary,
                    }),
                );
            }
        }
    }

    /// Commit a finished trim drag.
    fn commit_trim_drag(&mut self, cx: &mut Context<Self>) {
        let Some((clip_id, edge, frame)) = self.state.take_trim_drag() else {
            cx.notify();
            return;
        };
        self.apply_trim(&clip_id, edge, frame);
        cx.notify();
    }

    /// Select every clip (menu SelectAll).
    pub fn select_all(&mut self, cx: &mut Context<Self>) {
        self.state.select_all();
        cx.notify();
    }

    /// Trim each selected clip's boundary to the playhead. Clips whose
    /// range does not contain the playhead are skipped.
    pub fn trim_selected_to_playhead(&mut self, edge: TrimEdge, cx: &mut Context<Self>) {
        let playhead = self.state.playhead_frame;
        for id in self.state.selected_clip_ids.clone() {
            let in_range = self.state.clips.iter().any(|c| {
                c.id == id
                    && playhead > c.start_frame
                    && playhead < c.start_frame + c.duration_frames
            });
            if in_range {
                self.apply_trim(&id, edge, playhead);
            }
        }
        cx.notify();
    }

    /// Ripple-delete the selected clips (per-track ranges).
    pub fn ripple_delete_selected(&mut self, cx: &mut Context<Self>) {
        let mut per_track: std::collections::BTreeMap<usize, Vec<serde_json::Value>> =
            std::collections::BTreeMap::new();
        for id in &self.state.selected_clip_ids {
            let Some(track) = self.state.track_index_of_clip(id) else {
                continue;
            };
            let Some(clip) = self.state.clips.iter().find(|c| &c.id == id) else {
                continue;
            };
            per_track.entry(track).or_default().push(serde_json::json!({
                "start": clip.start_frame,
                "end": clip.start_frame + clip.duration_frames,
            }));
        }
        for (track_index, ranges) in per_track {
            Self::run_shared_tool(
                "ripple_delete_ranges",
                serde_json::json!({ "trackIndex": track_index, "ranges": ranges }),
            );
        }
        self.state.clear_selection();
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

    /// Space: toggle playback.
    pub fn transport_toggle_play(&mut self, cx: &mut Context<Self>) {
        self.state.transport.toggle_play();
        self.ensure_ticker(cx);
        cx.notify();
    }

    /// JKL keys: direction -1 (J), 0 (K), 1 (L).
    pub fn transport_jkl(&mut self, direction: i8, cx: &mut Context<Self>) {
        match direction {
            d if d < 0 => self.state.transport.jkl_backward(),
            0 => self.state.transport.jkl_pause(),
            _ => self.state.transport.jkl_forward(),
        }
        self.ensure_ticker(cx);
        cx.notify();
    }

    /// Step/skip: pause and move the playhead by delta frames.
    pub fn transport_step(&mut self, delta: i64, cx: &mut Context<Self>) {
        self.state.step_frames(delta);
        cx.notify();
    }

    /// Spawn the ~30Hz tick loop if playback is active and no loop runs.
    fn ensure_ticker(&mut self, cx: &mut Context<Self>) {
        if self.ticker_running || !self.state.transport.is_playing() {
            return;
        }
        self.ticker_running = true;
        cx.spawn(async move |this, cx| loop {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(33))
                .await;
            let alive = this.update(cx, |view, cx| {
                let moving = view.state.transport_tick(0.033);
                if moving {
                    cx.notify();
                } else {
                    view.ticker_running = false;
                }
                moving
            });
            if !alive.unwrap_or(false) {
                break;
            }
        })
        .detach();
    }

    /// Inline tab rename keystrokes; inert unless a rename is in progress.
    fn handle_key_down(
        &mut self,
        event: &KeyDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((id, text)) = self.tab_editing.as_mut() else {
            return;
        };
        match event.keystroke.key.as_str() {
            "enter" => {
                let trimmed = text.trim().to_string();
                let id = id.clone();
                self.tab_editing = None;
                if !trimmed.is_empty() {
                    Self::run_shared_tool(
                        "rename_media",
                        serde_json::json!({ "mediaId": id, "name": trimmed }),
                    );
                }
            }
            "escape" => {
                self.tab_editing = None;
            }
            "backspace" => {
                text.pop();
            }
            // key_char is None for space on Windows — insert it explicitly.
            "space" => {
                text.push(' ');
            }
            _ => {
                let mods = &event.keystroke.modifiers;
                if !mods.control && !mods.platform && !mods.function {
                    if let Some(ch) = event.keystroke.key_char.as_deref() {
                        if !ch.chars().any(char::is_control) {
                            text.push_str(ch);
                        }
                    }
                }
            }
        }
        cx.stop_propagation();
        cx.notify();
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
        // Timeline tabs (#255): active first, then siblings. Read fresh each
        // render; clicks run the shared timeline tools so every view refreshes
        // through the revision bump.
        let (tab_active_id, tab_list): (String, Vec<(String, String)>) = {
            let exec = crate::editor_state_hub::EditorStateHub::global().executor();
            let guard = exec.lock();
            match guard {
                Ok(ref g) => {
                    let label = |t: &core_model::Timeline| {
                        if t.name.is_empty() {
                            "Timeline".to_string()
                        } else {
                            t.name.clone()
                        }
                    };
                    (
                        g.timeline().id.clone(),
                        std::iter::once((g.timeline().id.clone(), label(g.timeline())))
                            .chain(
                                g.sibling_timelines()
                                    .iter()
                                    .map(|t| (t.id.clone(), label(t))),
                            )
                            .collect(),
                    )
                }
                Err(_) => (String::new(), Vec::new()),
            }
        };
        // Drop a rename whose tab vanished (agent delete / project switch).
        if let Some((editing_id, _)) = &self.tab_editing {
            if !tab_list.iter().any(|(id, _)| id == editing_id) {
                self.tab_editing = None;
            }
        }
        let tab_editing = self.tab_editing.clone();
        let tab_count = tab_list.len();
        let tracks = self.state.tracks.clone();
        let clips = self.state.clips.clone();
        let selected_ids = self.state.selected_clip_ids.clone();
        let drag_clip_id = self.state.clip_drag.as_ref().map(|d| d.clip_id.clone());
        let drag_proposed_start = self.state.clip_drag.as_ref().map(|d| d.proposed_start);
        let drag_proposed_track = self
            .state
            .clip_drag
            .as_ref()
            .map(|d| d.proposed_track_index);
        let trim_drag = self.state.trim_drag.clone();
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
            .track_focus(&self.focus_handle.clone())
            .on_key_down(cx.listener(Self::handle_key_down))
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .bg(Background::SURFACE)
            // ── Timeline tabs (#255 TimelineTabBar) ──
            .child(
                div()
                    .id("timeline-tabs")
                    .flex()
                    .flex_row()
                    .items_center()
                    .h(px(28.0))
                    .w_full()
                    .px(px(Spacing::SM))
                    .gap(px(Spacing::XS))
                    .bg(Background::RAISED)
                    .border_b_1()
                    .border_color(BorderColors::PRIMARY)
                    .children(tab_list.into_iter().enumerate().map(|(i, (id, name))| {
                        let active = id == tab_active_id;
                        let editing = tab_editing
                            .as_ref()
                            .filter(|(eid, _)| *eid == id)
                            .map(|(_, t)| t.clone());
                        let switch_id = id.clone();
                        let rename_id = id.clone();
                        let rename_seed = name.clone();
                        let close_id = id.clone();
                        let is_editing = editing.is_some();
                        let mut tab = div()
                            .id(gpui::SharedString::from(format!("tl-tab-{i}")))
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(Spacing::XS))
                            .px(px(Spacing::SM))
                            .py(px(2.0))
                            .rounded(px(Radius::SM))
                            .text_size(px(FontSize::XS))
                            .cursor_pointer()
                            .child(match editing {
                                Some(text) => format!("{text}\u{258f}"),
                                None => name,
                            });
                        tab = if is_editing {
                            tab.bg(Background::SURFACE)
                                .text_color(Text::PRIMARY)
                                .border_1()
                                .border_color(Accent::PRIMARY)
                        } else if active {
                            tab.bg(Background::SURFACE).text_color(Text::PRIMARY)
                        } else {
                            tab.text_color(Text::SECONDARY)
                        };
                        if !active {
                            tab = tab.on_click(cx.listener(move |_, _, _, cx| {
                                Self::run_shared_tool(
                                    "set_active_timeline",
                                    serde_json::json!({ "timelineId": switch_id }),
                                );
                                cx.notify();
                            }));
                        } else if !is_editing {
                            // Double-click the active tab to rename it inline.
                            tab = tab.on_click(cx.listener(
                                move |this, e: &gpui::ClickEvent, window, cx| {
                                    if e.click_count() == 2 {
                                        this.tab_editing =
                                            Some((rename_id.clone(), rename_seed.clone()));
                                        window.focus(&this.focus_handle, cx);
                                        cx.notify();
                                    }
                                },
                            ));
                        }
                        if tab_count > 1 && !is_editing {
                            tab = tab.child(
                                div()
                                    .id(gpui::SharedString::from(format!("tl-tab-close-{i}")))
                                    .text_size(px(FontSize::XS))
                                    .text_color(Text::SECONDARY)
                                    .cursor_pointer()
                                    .child("×")
                                    .on_click(cx.listener(move |_, _, _, cx| {
                                        Self::run_shared_tool(
                                            "delete_media",
                                            serde_json::json!({ "mediaId": close_id }),
                                        );
                                        cx.notify();
                                    })),
                            );
                        }
                        tab
                    }))
                    .child(
                        div()
                            .id("tl-tab-new")
                            .px(px(Spacing::SM))
                            .py(px(2.0))
                            .rounded(px(Radius::SM))
                            .text_size(px(FontSize::XS))
                            .text_color(Text::SECONDARY)
                            .cursor_pointer()
                            .child("+")
                            .on_click(cx.listener(|_, _, _, cx| {
                                Self::run_shared_tool(
                                    "create_timeline",
                                    serde_json::json!({}),
                                );
                                cx.notify();
                            })),
                    ),
            )
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
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _: &MouseDownEvent, _, cx| {
                                    this.state.clear_selection();
                                    cx.notify();
                                }),
                            )
                            .on_drag_move::<ClipDragToken>(cx.listener(
                                |this, e: &DragMoveEvent<ClipDragToken>, _, cx| {
                                    let content_x =
                                        e.event.position.x.as_f32() - e.bounds.origin.x.as_f32();
                                    let content_y =
                                        e.event.position.y.as_f32() - e.bounds.origin.y.as_f32();
                                    let frame =
                                        this.state.frame_for_x(this.state.scroll_x + content_x);
                                    this.state.update_clip_drag(frame);
                                    this.state
                                        .update_clip_drag_track(this.state.scroll_y + content_y);
                                    cx.notify();
                                },
                            ))
                            .on_drop::<ClipDragToken>(cx.listener(|this, _, _, cx| {
                                this.commit_clip_drag(cx);
                            }))
                            .on_drag_move::<TrimDragToken>(cx.listener(
                                |this, e: &DragMoveEvent<TrimDragToken>, _, cx| {
                                    let content_x =
                                        e.event.position.x.as_f32() - e.bounds.origin.x.as_f32();
                                    let frame =
                                        this.state.frame_for_x(this.state.scroll_x + content_x);
                                    this.state.update_trim_drag(frame);
                                    cx.notify();
                                },
                            ))
                            .on_drop::<TrimDragToken>(cx.listener(|this, _, _, cx| {
                                this.commit_trim_drag(cx);
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
                                            .filter(|c| {
                                                if drag_clip_id.as_deref() == Some(c.id.as_str()) {
                                                    drag_proposed_track == Some(i)
                                                } else {
                                                    c.track_id == track.id
                                                }
                                            })
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
                                                let mut start = if is_dragging {
                                                    drag_proposed_start.unwrap_or(clip.start_frame)
                                                } else {
                                                    clip.start_frame
                                                };
                                                let mut end = start + clip.duration_frames;
                                                if let Some(t) = trim_drag
                                                    .as_ref()
                                                    .filter(|t| t.clip_id == clip.id)
                                                {
                                                    match t.edge {
                                                        TrimEdge::Start => start = t.proposed_frame,
                                                        TrimEdge::End => end = t.proposed_frame,
                                                    }
                                                }
                                                let clip_x = start as f32 * zoom - scroll_x;
                                                let clip_w = ((end - start) as f32 * zoom).max(4.0);
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
                                                                cx.stop_propagation();
                                                                let multi = e.modifiers.shift
                                                                    || e.modifiers.platform
                                                                    || e.modifiers.control;
                                                                if multi {
                                                                    this.state
                                                                        .toggle_select(&clip_id);
                                                                } else {
                                                                    this.state
                                                                        .select_only(&clip_id);
                                                                }
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
                                                    .child(trim_handle(
                                                        cx,
                                                        clip.id.clone(),
                                                        TrimEdge::Start,
                                                    ))
                                                    .child(trim_handle(
                                                        cx,
                                                        clip.id.clone(),
                                                        TrimEdge::End,
                                                    ))
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

/// 6px trim hot zone on a clip edge.
fn trim_handle(
    cx: &mut Context<TimelineView>,
    clip_id: String,
    edge: TrimEdge,
) -> gpui::Stateful<gpui::Div> {
    let base = div()
        .id(format!(
            "trim-{}-{}",
            match edge {
                TrimEdge::Start => "l",
                TrimEdge::End => "r",
            },
            clip_id
        ))
        .absolute()
        .top_0()
        .bottom_0()
        .w(px(6.0))
        .cursor_ew_resize()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                cx.stop_propagation();
                this.state.begin_trim_drag(&clip_id, edge);
                cx.notify();
            }),
        )
        .on_drag(TrimDragToken, |_, _, _, cx| cx.new(|_| ClipDragPreview));
    match edge {
        TrimEdge::Start => base.left_0(),
        TrimEdge::End => base.right_0(),
    }
}

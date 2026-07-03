//! Timeline panel model — pure state for the timeline view.
//!
//! Covers UIX-008 (autoscroll), UIX-009 (track sizes), UIX-010 (layout constants).

/// Track type visible in the timeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrackKind {
    Video,
    Audio,
}

impl TrackKind {
    pub fn label(&self) -> &'static str {
        match self {
            TrackKind::Video => "Video",
            TrackKind::Audio => "Audio",
        }
    }
}

/// A single clip on a track — positional model for rendering.
#[derive(Debug, Clone)]
pub struct ClipSlot {
    pub id: String,
    pub track_id: String,
    pub start_frame: i64,
    pub duration_frames: i64,
    pub label: String,
}

impl ClipSlot {
    pub fn new(
        id: impl Into<String>,
        track_id: impl Into<String>,
        start_frame: i64,
        duration_frames: i64,
        label: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            track_id: track_id.into(),
            start_frame,
            duration_frames,
            label: label.into(),
        }
    }
}

/// A single timeline track row (header model).
#[derive(Debug, Clone)]
pub struct TrackRow {
    pub id: String,
    pub kind: TrackKind,
    pub label: String,
    /// Height in pixels (UIX-009: min 32, max 200, default 50).
    pub height: f32,
    pub muted: bool,
    pub hidden: bool,
}

impl TrackRow {
    pub fn new(id: impl Into<String>, kind: TrackKind, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            kind,
            label: label.into(),
            height: DEFAULT_TRACK_HEIGHT,
            muted: false,
            hidden: false,
        }
    }

    /// Clamp height to UIX-009 bounds.
    pub fn set_height(&mut self, h: f32) {
        self.height = h.clamp(MIN_TRACK_HEIGHT, MAX_TRACK_HEIGHT);
    }
}

/// UIX-010 layout constants.
pub const RULER_HEIGHT: f32 = 24.0;
pub const TRACK_HEADER_WIDTH: f32 = 100.0;
pub const DEFAULT_TRACK_HEIGHT: f32 = 50.0;
pub const MIN_TRACK_HEIGHT: f32 = 32.0;
pub const MAX_TRACK_HEIGHT: f32 = 200.0;
pub const TIMELINE_MIN_HEIGHT: f32 = 100.0;
pub const TIMELINE_MAX_HEIGHT: f32 = 700.0;

/// UIX-004: default pixels per frame.
pub const DEFAULT_PIXELS_PER_FRAME: f32 = 4.0;

/// UIX-007: zoom bounds.
pub const ZOOM_MIN: f32 = 0.05;
pub const ZOOM_MAX: f32 = 40.0;

/// Timeline view state.
#[derive(Debug, Clone)]
pub struct TimelineState {
    pub tracks: Vec<TrackRow>,
    pub clips: Vec<ClipSlot>,
    /// Current zoom (pixels per frame). UIX-007.
    pub zoom_scale: f32,
    /// Horizontal scroll offset in pixels.
    pub scroll_x: f32,
    /// Vertical scroll offset in pixels.
    pub scroll_y: f32,
    /// Current playhead position in project frames.
    pub playhead_frame: i64,
    /// Total project duration in frames.
    pub total_frames: i64,
    /// Project FPS.
    pub fps: i64,
    /// Snap indicator frame — Some(frame) shows the yellow dashed snap line during clip drag.
    /// Mirrors Swift SnapIndicatorOverlay's CAShapeLayer positioning.
    pub snap_x_frame: Option<i64>,
    /// View-only clip selection (not written back to core state).
    pub selected_clip_ids: Vec<String>,
    /// Active clip drag session, if any.
    pub clip_drag: Option<ClipDrag>,
    /// Active trim drag session, if any.
    pub trim_drag: Option<TrimDrag>,
}

/// In-flight clip drag: proposed position tracked in project frames.
#[derive(Debug, Clone)]
pub struct ClipDrag {
    pub clip_id: String,
    /// Pointer frame minus clip start at grab time.
    pub grab_offset_frames: i64,
    /// Original start frame (no-op detection).
    pub original_start: i64,
    /// Clip duration, used as the trailing snap probe.
    pub duration_frames: i64,
    /// Proposed (possibly snapped) new start frame.
    pub proposed_start: i64,
    /// Track index the drag started on.
    pub origin_track_index: usize,
    /// Proposed target track (same-kind tracks only).
    pub proposed_track_index: usize,
    snap_state: timeline_core::SnapState,
}

/// Which clip edge a trim drag operates on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrimEdge {
    Start,
    End,
}

/// In-flight trim drag on a clip edge.
#[derive(Debug, Clone)]
pub struct TrimDrag {
    pub clip_id: String,
    pub edge: TrimEdge,
    pub original_start: i64,
    pub original_end: i64,
    /// Proposed boundary frame (clamped so the clip keeps >= 1 frame).
    pub proposed_frame: i64,
}

impl TimelineState {
    pub fn new() -> Self {
        Self {
            tracks: Vec::new(),
            clips: Vec::new(),
            zoom_scale: DEFAULT_PIXELS_PER_FRAME,
            scroll_x: 0.0,
            scroll_y: 0.0,
            playhead_frame: 30,
            total_frames: 600,
            fps: 30,
            snap_x_frame: None,
            selected_clip_ids: Vec::new(),
            clip_drag: None,
            trim_drag: None,
        }
    }

    pub fn with_default_tracks(mut self) -> Self {
        self.tracks = vec![
            TrackRow::new("video-1", TrackKind::Video, "Video 1"),
            TrackRow::new("audio-1", TrackKind::Audio, "Audio 1"),
        ];
        self.clips = vec![
            ClipSlot::new("clip-1", "video-1", 0, 150, "Scene 01"),
            ClipSlot::new("clip-2", "video-1", 160, 120, "Interview"),
            ClipSlot::new("clip-3", "video-1", 290, 90, "B-Roll"),
            ClipSlot::new("clip-4", "audio-1", 0, 270, "Music Track"),
            ClipSlot::new("clip-5", "audio-1", 280, 200, "Voice Over"),
        ];
        self
    }

    /// Select exactly one clip.
    pub fn select_only(&mut self, clip_id: &str) {
        self.selected_clip_ids = vec![clip_id.to_string()];
    }

    pub fn toggle_select(&mut self, clip_id: &str) {
        if let Some(pos) = self.selected_clip_ids.iter().position(|id| id == clip_id) {
            self.selected_clip_ids.remove(pos);
        } else {
            self.selected_clip_ids.push(clip_id.to_string());
        }
    }

    pub fn clear_selection(&mut self) {
        self.selected_clip_ids.clear();
    }

    /// Select every clip.
    pub fn select_all(&mut self) {
        self.selected_clip_ids = self.clips.iter().map(|c| c.id.clone()).collect();
    }

    /// Track index under a content-area y position (row heights accumulate).
    pub fn track_index_at_y(&self, content_y: f32) -> Option<usize> {
        let mut top = 0.0f32;
        for (i, track) in self.tracks.iter().enumerate() {
            let bottom = top + track.height;
            if content_y >= top && content_y < bottom {
                return Some(i);
            }
            top = bottom;
        }
        None
    }

    /// Move the playhead to the frame under a content-area x position.
    pub fn scrub_to_content_x(&mut self, content_x: f32) {
        self.playhead_frame = self.frame_for_x(self.scroll_x + content_x).max(0);
    }

    /// Track index (for move_clips toTrack) of the clip's track.
    pub fn track_index_of_clip(&self, clip_id: &str) -> Option<usize> {
        let track_id = &self.clips.iter().find(|c| c.id == clip_id)?.track_id;
        self.tracks.iter().position(|t| &t.id == track_id)
    }

    /// Start dragging a clip. Returns false if the clip is unknown.
    pub fn begin_clip_drag(&mut self, clip_id: &str, pointer_frame: i64) -> bool {
        let Some(clip) = self.clips.iter().find(|c| c.id == clip_id) else {
            return false;
        };
        let track_index = self
            .tracks
            .iter()
            .position(|t| t.id == clip.track_id)
            .unwrap_or(0);
        self.clip_drag = Some(ClipDrag {
            clip_id: clip_id.to_string(),
            grab_offset_frames: pointer_frame - clip.start_frame,
            original_start: clip.start_frame,
            duration_frames: clip.duration_frames,
            proposed_start: clip.start_frame,
            origin_track_index: track_index,
            proposed_track_index: track_index,
            snap_state: timeline_core::SnapState::default(),
        });
        true
    }

    /// Propose a target track from the pointer y. Only tracks of the same
    /// kind as the origin track are accepted; otherwise the previous
    /// proposal is kept.
    pub fn update_clip_drag_track(&mut self, content_y: f32) {
        let Some(candidate) = self.track_index_at_y(content_y) else {
            return;
        };
        let Some(drag) = self.clip_drag.as_ref() else {
            return;
        };
        let origin_kind = match self.tracks.get(drag.origin_track_index) {
            Some(t) => t.kind.clone(),
            None => return,
        };
        if self
            .tracks
            .get(candidate)
            .is_some_and(|t| t.kind == origin_kind)
        {
            if let Some(drag) = self.clip_drag.as_mut() {
                drag.proposed_track_index = candidate;
            }
        }
    }

    /// Start a trim drag on a clip edge. Returns false for unknown clips.
    pub fn begin_trim_drag(&mut self, clip_id: &str, edge: TrimEdge) -> bool {
        let Some(clip) = self.clips.iter().find(|c| c.id == clip_id) else {
            return false;
        };
        let end = clip.start_frame + clip.duration_frames;
        self.trim_drag = Some(TrimDrag {
            clip_id: clip_id.to_string(),
            edge,
            original_start: clip.start_frame,
            original_end: end,
            proposed_frame: match edge {
                TrimEdge::Start => clip.start_frame,
                TrimEdge::End => end,
            },
        });
        true
    }

    /// Update the trim proposal, clamping so the clip keeps >= 1 frame.
    pub fn update_trim_drag(&mut self, pointer_frame: i64) {
        let Some(drag) = self.trim_drag.as_mut() else {
            return;
        };
        drag.proposed_frame = match drag.edge {
            TrimEdge::Start => pointer_frame.clamp(0, drag.original_end - 1),
            TrimEdge::End => pointer_frame.max(drag.original_start + 1),
        };
    }

    /// Finish the trim drag; None when the boundary did not change.
    pub fn take_trim_drag(&mut self) -> Option<(String, TrimEdge, i64)> {
        let drag = self.trim_drag.take()?;
        let unchanged = match drag.edge {
            TrimEdge::Start => drag.proposed_frame == drag.original_start,
            TrimEdge::End => drag.proposed_frame == drag.original_end,
        };
        if unchanged {
            return None;
        }
        Some((drag.clip_id, drag.edge, drag.proposed_frame))
    }

    /// Update the drag proposal from the current pointer frame, snapping to
    /// other clips' edges and the playhead.
    pub fn update_clip_drag(&mut self, pointer_frame: i64) {
        let zoom = f64::from(self.zoom_scale);
        let playhead = self.playhead_frame;
        let Some(drag) = self.clip_drag.as_mut() else {
            return;
        };
        let mut proposed = (pointer_frame - drag.grab_offset_frames).max(0);

        let mut targets = vec![timeline_core::SnapTarget {
            frame: playhead,
            kind: timeline_core::SnapTargetKind::Playhead,
        }];
        for clip in &self.clips {
            if clip.id == drag.clip_id {
                continue;
            }
            for frame in [clip.start_frame, clip.start_frame + clip.duration_frames] {
                targets.push(timeline_core::SnapTarget {
                    frame,
                    kind: timeline_core::SnapTargetKind::ClipEdge,
                });
            }
        }
        targets.sort_by_key(|t| t.frame);

        let snap = timeline_core::find_snap(
            proposed,
            &[0, drag.duration_frames],
            &targets,
            &mut drag.snap_state,
            timeline_core::THRESHOLD_PIXELS,
            zoom,
        );
        if let Some(result) = snap {
            proposed = (result.frame - result.probe_offset).max(0);
            self.snap_x_frame = Some(result.frame);
        } else {
            self.snap_x_frame = None;
        }
        drag.proposed_start = proposed;
    }

    /// Finish the drag. Returns Some((clip_id, to_track, to_frame)) when
    /// the clip actually moved; None for a same-track zero-distance drop.
    pub fn take_clip_drag(&mut self) -> Option<(String, usize, i64)> {
        self.snap_x_frame = None;
        let drag = self.clip_drag.take()?;
        if drag.proposed_start == drag.original_start
            && drag.proposed_track_index == drag.origin_track_index
        {
            return None;
        }
        Some((drag.clip_id, drag.proposed_track_index, drag.proposed_start))
    }

    /// Build view state from the shared core timeline (project data path).
    /// View-only fields (zoom, scroll, playhead) keep `new()` defaults —
    /// callers preserving an existing view copy them back afterward.
    pub fn from_core(
        timeline: &core_model::Timeline,
        manifest: &core_model::MediaManifest,
    ) -> Self {
        let mut state = Self::new();
        state.fps = timeline.fps;

        let mut video_count = 0usize;
        let mut audio_count = 0usize;
        let mut max_end = 0i64;

        for track in &timeline.tracks {
            let kind = if track.r#type == core_model::ClipType::Audio {
                audio_count += 1;
                TrackKind::Audio
            } else {
                video_count += 1;
                TrackKind::Video
            };
            let number = match kind {
                TrackKind::Video => video_count,
                TrackKind::Audio => audio_count,
            };
            let mut row = TrackRow::new(
                track.id.clone(),
                kind.clone(),
                format!("{} {}", kind.label(), number),
            );
            row.muted = track.muted;
            row.hidden = track.hidden;
            state.tracks.push(row);

            for clip in &track.clips {
                let label = manifest
                    .entry_for(&clip.media_ref)
                    .map(|e| e.name.clone())
                    .unwrap_or_else(|| clip.media_ref.clone());
                max_end = max_end.max(clip.start_frame + clip.duration_frames);
                state.clips.push(ClipSlot::new(
                    clip.id.clone(),
                    track.id.clone(),
                    clip.start_frame,
                    clip.duration_frames,
                    label,
                ));
            }
        }

        state.total_frames = max_end.max(state.total_frames);
        state
    }

    /// X position (in pixels) for a given frame at current zoom.
    pub fn x_for_frame(&self, frame: i64) -> f32 {
        frame as f32 * self.zoom_scale
    }

    /// Frame at a given X pixel position.
    pub fn frame_for_x(&self, x: f32) -> i64 {
        (x / self.zoom_scale).round() as i64
    }

    /// Total timeline width in pixels at current zoom.
    pub fn total_width(&self) -> f32 {
        self.total_frames as f32 * self.zoom_scale
    }

    /// Clamp zoom to UIX-007 bounds.
    pub fn set_zoom(&mut self, scale: f32) {
        self.zoom_scale = scale.clamp(ZOOM_MIN, ZOOM_MAX);
    }

    pub fn zoom_in(&mut self) {
        self.set_zoom(self.zoom_scale * 1.5);
    }

    pub fn zoom_out(&mut self) {
        self.set_zoom(self.zoom_scale / 1.5);
    }

    /// Scroll the timeline to bring the playhead into view (UIX-008).
    pub fn ensure_playhead_visible(&mut self, visible_width: f32) {
        let px = self.x_for_frame(self.playhead_frame);
        let edge_zone = 56.0; // UIX-008: edge zone width
        if px < self.scroll_x + edge_zone {
            self.scroll_x = (px - edge_zone).max(0.0);
        } else if px > self.scroll_x + visible_width - edge_zone {
            self.scroll_x = px - visible_width + edge_zone;
        }
    }

    /// Total height of all track rows in pixels.
    pub fn total_tracks_height(&self) -> f32 {
        self.tracks.iter().map(|t| t.height).sum()
    }
}

impl Default for TimelineState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeline_default_zoom_is_4_pixels_per_frame() {
        let s = TimelineState::new();
        assert!((s.zoom_scale - DEFAULT_PIXELS_PER_FRAME).abs() < 1e-6);
    }

    #[test]
    fn timeline_x_for_frame() {
        let s = TimelineState::new(); // 4px/frame
        assert!((s.x_for_frame(10) - 40.0).abs() < 1e-6);
    }

    #[test]
    fn timeline_frame_for_x() {
        let s = TimelineState::new(); // 4px/frame
        assert_eq!(s.frame_for_x(40.0), 10);
    }

    #[test]
    fn timeline_zoom_clamped() {
        let mut s = TimelineState::new();
        s.set_zoom(0.0); // below min
        assert!((s.zoom_scale - ZOOM_MIN).abs() < 1e-6);
        s.set_zoom(1000.0); // above max
        assert!((s.zoom_scale - ZOOM_MAX).abs() < 1e-6);
    }

    #[test]
    fn track_height_clamped() {
        let mut row = TrackRow::new("t1", TrackKind::Video, "V1");
        row.set_height(0.0); // below min
        assert!((row.height - MIN_TRACK_HEIGHT).abs() < 1e-6);
        row.set_height(9999.0); // above max
        assert!((row.height - MAX_TRACK_HEIGHT).abs() < 1e-6);
    }

    #[test]
    fn track_height_in_range_unchanged() {
        let mut row = TrackRow::new("t1", TrackKind::Video, "V1");
        row.set_height(80.0);
        assert!((row.height - 80.0).abs() < 1e-6);
    }

    #[test]
    fn timeline_total_tracks_height() {
        let mut s = TimelineState::new().with_default_tracks();
        // 2 tracks × 50px default = 100px
        assert!((s.total_tracks_height() - 100.0).abs() < 1e-6);
        s.tracks[0].set_height(80.0);
        assert!((s.total_tracks_height() - 130.0).abs() < 1e-6);
    }

    #[test]
    fn timeline_ensure_playhead_visible_scrolls_right() {
        let mut s = TimelineState::new();
        s.total_frames = 1000;
        s.playhead_frame = 500; // at 4px/frame = 2000px
        s.scroll_x = 0.0;
        s.ensure_playhead_visible(800.0);
        // playhead at 2000, visible_width=800, edge_zone=56 → should scroll
        assert!(s.scroll_x > 0.0, "should have scrolled: {}", s.scroll_x);
    }

    #[test]
    fn timeline_with_default_tracks_has_two_tracks() {
        let s = TimelineState::new().with_default_tracks();
        assert_eq!(s.tracks.len(), 2);
        assert_eq!(s.tracks[0].kind, TrackKind::Video);
        assert_eq!(s.tracks[1].kind, TrackKind::Audio);
    }

    #[test]
    fn timeline_zoom_in_out() {
        let mut s = TimelineState::new();
        let orig = s.zoom_scale;
        s.zoom_in();
        assert!(s.zoom_scale > orig, "zoom in increases zoom");
        s.zoom_out();
        // Should be close to original
        assert!(
            (s.zoom_scale - orig).abs() < 1e-3,
            "zoom out returns close to original"
        );
    }

    #[test]
    fn track_kind_labels() {
        assert_eq!(TrackKind::Video.label(), "Video");
        assert_eq!(TrackKind::Audio.label(), "Audio");
    }

    #[test]
    fn ruler_and_header_constants_match_spec() {
        // UIX-010
        assert!((RULER_HEIGHT - 24.0).abs() < 1e-6);
        assert!((TRACK_HEADER_WIDTH - 100.0).abs() < 1e-6);
        assert!((DEFAULT_TRACK_HEIGHT - 50.0).abs() < 1e-6);
    }

    // ── from_core mapping (project-load spec) ──────────────────────────

    fn core_timeline(json: &str) -> core_model::Timeline {
        serde_json::from_str(json).unwrap()
    }

    fn core_manifest(json: &str) -> core_model::MediaManifest {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn from_core_maps_track_kinds_and_clip_labels() {
        let timeline = core_timeline(
            r#"{"fps":30,"tracks":[
                {"id":"t1","type":"video","clips":[
                    {"id":"c1","mediaRef":"m1","mediaType":"video","sourceClipType":"video","startFrame":0,"durationFrames":150},
                    {"id":"c2","mediaRef":"m2","mediaType":"video","sourceClipType":"video","startFrame":160,"durationFrames":50}
                ]},
                {"id":"t2","type":"audio","clips":[]}
            ]}"#,
        );
        let manifest = core_manifest(
            r#"{"version":1,"entries":[
                {"id":"m1","name":"Interview.mp4","type":"video","source":{"project":{"relativePath":"media/interview.mp4"}},"duration":5.0}
            ]}"#,
        );
        let s = TimelineState::from_core(&timeline, &manifest);
        assert_eq!(s.tracks.len(), 2);
        assert_eq!(s.tracks[0].kind, TrackKind::Video);
        assert_eq!(s.tracks[0].label, "Video 1");
        assert_eq!(s.tracks[1].kind, TrackKind::Audio);
        assert_eq!(s.tracks[1].label, "Audio 1");
        assert_eq!(s.clips.len(), 2);
        assert_eq!(s.clips[0].label, "Interview.mp4");
        assert_eq!(s.clips[0].start_frame, 0);
        assert_eq!(s.clips[0].duration_frames, 150);
        assert_eq!(
            s.clips[1].label, "m2",
            "no manifest entry falls back to media_ref"
        );
    }

    #[test]
    fn from_core_total_frames_floor_and_max() {
        // Clips ending at 290 and 480 → floor of 600.
        let short = core_timeline(
            r#"{"fps":30,"tracks":[{"id":"t1","type":"video","clips":[
                {"id":"c1","mediaRef":"m","mediaType":"video","sourceClipType":"video","startFrame":200,"durationFrames":90},
                {"id":"c2","mediaRef":"m","mediaType":"video","sourceClipType":"video","startFrame":400,"durationFrames":80}
            ]}]}"#,
        );
        let manifest = core_model::MediaManifest::default();
        assert_eq!(
            TimelineState::from_core(&short, &manifest).total_frames,
            600
        );

        // Clips ending at 290 and 720 → 720.
        let long = core_timeline(
            r#"{"fps":30,"tracks":[{"id":"t1","type":"video","clips":[
                {"id":"c1","mediaRef":"m","mediaType":"video","sourceClipType":"video","startFrame":200,"durationFrames":90},
                {"id":"c2","mediaRef":"m","mediaType":"video","sourceClipType":"video","startFrame":600,"durationFrames":120}
            ]}]}"#,
        );
        assert_eq!(TimelineState::from_core(&long, &manifest).total_frames, 720);
    }

    // Interaction logic (timeline-interactions spec)

    fn state_with_two_clips() -> TimelineState {
        let mut s = TimelineState::new();
        s.tracks = vec![
            TrackRow::new("t1", TrackKind::Video, "Video 1"),
            TrackRow::new("t2", TrackKind::Audio, "Audio 1"),
        ];
        s.clips = vec![
            ClipSlot::new("c1", "t1", 0, 100, "A"),
            ClipSlot::new("c2", "t1", 300, 100, "B"),
        ];
        s
    }

    #[test]
    fn scrub_clamps_below_zero_and_maps_frames() {
        let mut s = state_with_two_clips();
        s.scroll_x = 0.0;
        s.scrub_to_content_x(480.0); // 480px / 4px-per-frame = frame 120
        assert_eq!(s.playhead_frame, 120);
        s.scroll_x = 40.0;
        s.scrub_to_content_x(-80.0);
        assert_eq!(s.playhead_frame, 0, "negative frames clamp to 0");
    }

    #[test]
    fn selection_moves_between_clips() {
        let mut s = state_with_two_clips();
        s.select_only("c1");
        assert_eq!(s.selected_clip_ids, vec!["c1".to_string()]);
        s.select_only("c2");
        assert_eq!(s.selected_clip_ids, vec!["c2".to_string()]);
        s.clear_selection();
        assert!(s.selected_clip_ids.is_empty());
    }

    #[test]
    fn drag_proposal_clamps_to_zero() {
        let mut s = state_with_two_clips();
        assert!(s.begin_clip_drag("c1", 10)); // grab offset 10
        s.update_clip_drag(-500);
        assert_eq!(s.clip_drag.as_ref().unwrap().proposed_start, 0);
    }

    #[test]
    fn drag_snaps_to_neighbor_edge_and_sets_indicator() {
        let mut s = state_with_two_clips();
        s.playhead_frame = -1000; // keep playhead out of range
        assert!(s.begin_clip_drag("c1", 0)); // grab at clip start
                                             // c2 starts at 300; dragging c1 (duration 100) so its end nears 300:
                                             // pointer 199 → proposed 199, trailing probe 299, within 2 frames of 300.
        s.update_clip_drag(199);
        let drag = s.clip_drag.as_ref().unwrap();
        assert_eq!(drag.proposed_start, 200, "trailing edge snapped to 300");
        assert_eq!(s.snap_x_frame, Some(300));
    }

    #[test]
    fn zero_distance_drop_returns_none() {
        let mut s = state_with_two_clips();
        assert!(s.begin_clip_drag("c1", 50));
        s.update_clip_drag(50);
        assert!(s.take_clip_drag().is_none());
        assert!(s.snap_x_frame.is_none());
        assert!(s.clip_drag.is_none());
    }

    #[test]
    fn moved_drop_returns_target() {
        let mut s = state_with_two_clips();
        assert!(s.begin_clip_drag("c1", 0));
        s.update_clip_drag(90);
        assert_eq!(s.take_clip_drag(), Some(("c1".to_string(), 0, 90)));
    }

    #[test]
    fn select_all_selects_every_clip() {
        let mut s = state_with_two_clips();
        s.select_all();
        assert_eq!(s.selected_clip_ids.len(), 2);
        s.clear_selection();
        assert!(s.selected_clip_ids.is_empty());
    }

    fn state_with_two_video_tracks() -> TimelineState {
        let mut s = TimelineState::new();
        s.tracks = vec![
            TrackRow::new("v1", TrackKind::Video, "Video 1"),
            TrackRow::new("v2", TrackKind::Video, "Video 2"),
            TrackRow::new("a1", TrackKind::Audio, "Audio 1"),
        ];
        s.clips = vec![ClipSlot::new("c1", "v1", 0, 100, "A")];
        s
    }

    #[test]
    fn track_index_at_y_uses_row_heights() {
        let s = state_with_two_video_tracks();
        // Default heights 50 each: rows at [0,50), [50,100), [100,150).
        assert_eq!(s.track_index_at_y(10.0), Some(0));
        assert_eq!(s.track_index_at_y(60.0), Some(1));
        assert_eq!(s.track_index_at_y(120.0), Some(2));
        assert_eq!(s.track_index_at_y(999.0), None);
    }

    #[test]
    fn cross_track_drag_accepts_same_kind_only() {
        let mut s = state_with_two_video_tracks();
        assert!(s.begin_clip_drag("c1", 0));
        // Over the second video track: accepted.
        s.update_clip_drag_track(60.0);
        assert_eq!(s.clip_drag.as_ref().unwrap().proposed_track_index, 1);
        // Over the audio track: rejected, keeps previous proposal.
        s.update_clip_drag_track(120.0);
        assert_eq!(s.clip_drag.as_ref().unwrap().proposed_track_index, 1);
        // Cross-track drop reports the new track even at the same frame.
        assert_eq!(s.take_clip_drag(), Some(("c1".to_string(), 1, 0)));
    }

    #[test]
    fn same_track_zero_distance_still_none() {
        let mut s = state_with_two_video_tracks();
        assert!(s.begin_clip_drag("c1", 0));
        s.update_clip_drag(0);
        s.update_clip_drag_track(10.0); // stays on track 0
        assert!(s.take_clip_drag().is_none());
    }

    #[test]
    fn trim_end_clamps_to_start_plus_one() {
        let mut s = state_with_two_clips();
        assert!(s.begin_trim_drag("c1", TrimEdge::End)); // clip 0..100
        s.update_trim_drag(-50);
        assert_eq!(s.trim_drag.as_ref().unwrap().proposed_frame, 1);
        s.update_trim_drag(70);
        assert_eq!(
            s.take_trim_drag(),
            Some(("c1".to_string(), TrimEdge::End, 70))
        );
    }

    #[test]
    fn trim_start_clamps_between_zero_and_end_minus_one() {
        let mut s = state_with_two_clips();
        assert!(s.begin_trim_drag("c1", TrimEdge::Start)); // clip 0..100
        s.update_trim_drag(-10);
        assert_eq!(s.trim_drag.as_ref().unwrap().proposed_frame, 0);
        s.update_trim_drag(500);
        assert_eq!(s.trim_drag.as_ref().unwrap().proposed_frame, 99);
    }

    #[test]
    fn trim_without_change_returns_none() {
        let mut s = state_with_two_clips();
        assert!(s.begin_trim_drag("c1", TrimEdge::Start));
        s.update_trim_drag(0);
        assert!(s.take_trim_drag().is_none());
        assert!(s.trim_drag.is_none());
    }

    #[test]
    fn track_index_lookup() {
        let mut s = state_with_two_clips();
        s.clips.push(ClipSlot::new("c3", "t2", 0, 50, "M"));
        assert_eq!(s.track_index_of_clip("c1"), Some(0));
        assert_eq!(s.track_index_of_clip("c3"), Some(1));
        assert_eq!(s.track_index_of_clip("nope"), None);
    }

    #[test]
    fn from_core_empty_project_keeps_default_extent() {
        let empty = core_timeline(r#"{"fps":24,"tracks":[]}"#);
        let s = TimelineState::from_core(&empty, &core_model::MediaManifest::default());
        assert!(s.tracks.is_empty());
        assert!(s.clips.is_empty());
        assert_eq!(s.total_frames, 600);
        assert_eq!(s.fps, 24);
    }
}

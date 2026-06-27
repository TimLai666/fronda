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
        assert!((s.zoom_scale - orig).abs() < 1e-3, "zoom out returns close to original");
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
}

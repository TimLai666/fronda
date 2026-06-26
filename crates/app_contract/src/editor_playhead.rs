//! Independent playhead state for timeline and source-media tabs (EDT-005).

use serde::{Deserialize, Serialize};

/// EDT-005: Independent playhead position for each preview tab.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PreviewTab {
    /// Timeline preview (composited timeline).
    Timeline,
    /// Source-media preview (raw source).
    SourceMedia,
}

/// EDT-005: Independent playhead state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayheadState {
    /// Current tab.
    pub active_tab: PreviewTab,
    /// Playhead position in frames for timeline preview.
    pub timeline_frame: i64,
    /// Playhead position in frames for source-media preview.
    pub source_media_frame: i64,
    /// Whether playback is active.
    pub playing: bool,
}

impl Default for PlayheadState {
    fn default() -> Self {
        Self {
            active_tab: PreviewTab::Timeline,
            timeline_frame: 0,
            source_media_frame: 0,
            playing: false,
        }
    }
}

impl PlayheadState {
    /// Create a new playhead state for a specific tab.
    pub fn new(active_tab: PreviewTab) -> Self {
        Self {
            active_tab,
            ..Default::default()
        }
    }

    /// Switch to timeline preview tab.
    pub fn switch_to_timeline(&mut self) {
        self.active_tab = PreviewTab::Timeline;
    }

    /// Switch to source-media preview tab.
    pub fn switch_to_source_media(&mut self) {
        self.active_tab = PreviewTab::SourceMedia;
    }

    /// Get the current frame position based on active tab.
    pub fn current_frame(&self) -> i64 {
        match self.active_tab {
            PreviewTab::Timeline => self.timeline_frame,
            PreviewTab::SourceMedia => self.source_media_frame,
        }
    }

    /// Set the current frame position (updates the correct tab's position).
    pub fn set_current_frame(&mut self, frame: i64) {
        match self.active_tab {
            PreviewTab::Timeline => self.timeline_frame = frame,
            PreviewTab::SourceMedia => self.source_media_frame = frame,
        }
    }

    /// Set frame for a specific tab without switching.
    pub fn set_frame_for_tab(&mut self, tab: PreviewTab, frame: i64) {
        match tab {
            PreviewTab::Timeline => self.timeline_frame = frame,
            PreviewTab::SourceMedia => self.source_media_frame = frame,
        }
    }

    /// Toggle playback.
    pub fn toggle_playback(&mut self) {
        self.playing = !self.playing;
    }

    /// Start playback.
    pub fn play(&mut self) {
        self.playing = true;
    }

    /// Stop playback.
    pub fn stop(&mut self) {
        self.playing = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edt_005_default_is_timeline() {
        let p = PlayheadState::default();
        assert_eq!(p.active_tab, PreviewTab::Timeline);
        assert_eq!(p.timeline_frame, 0);
        assert_eq!(p.source_media_frame, 0);
        assert!(!p.playing);
    }

    #[test]
    fn edt_005_new_with_source_media() {
        let p = PlayheadState::new(PreviewTab::SourceMedia);
        assert_eq!(p.active_tab, PreviewTab::SourceMedia);
    }

    #[test]
    fn edt_005_switch_tabs() {
        let mut p = PlayheadState::default();
        p.switch_to_source_media();
        assert_eq!(p.active_tab, PreviewTab::SourceMedia);

        p.timeline_frame = 100;
        p.source_media_frame = 50;
        assert_eq!(p.current_frame(), 50);

        p.switch_to_timeline();
        assert_eq!(p.current_frame(), 100);
    }

    #[test]
    fn edt_005_set_current_frame_respects_tab() {
        let mut p = PlayheadState::default();
        p.set_current_frame(120);
        assert_eq!(p.timeline_frame, 120);
        assert_eq!(p.source_media_frame, 0);

        p.switch_to_source_media();
        p.set_current_frame(60);
        assert_eq!(p.source_media_frame, 60);
        assert_eq!(p.timeline_frame, 120); // unchanged
    }

    #[test]
    fn edt_005_set_frame_for_tab_independent() {
        let mut p = PlayheadState::default();
        p.set_frame_for_tab(PreviewTab::Timeline, 200);
        p.set_frame_for_tab(PreviewTab::SourceMedia, 80);
        assert_eq!(p.timeline_frame, 200);
        assert_eq!(p.source_media_frame, 80);
        assert_eq!(p.active_tab, PreviewTab::Timeline); // tab unchanged
    }

    #[test]
    fn edt_005_playback_toggle() {
        let mut p = PlayheadState::default();
        assert!(!p.playing);
        p.toggle_playback();
        assert!(p.playing);
        p.toggle_playback();
        assert!(!p.playing);
    }

    #[test]
    fn edt_005_play_stop() {
        let mut p = PlayheadState::default();
        p.play();
        assert!(p.playing);
        p.stop();
        assert!(!p.playing);
        // idempotent
        p.stop();
        assert!(!p.playing);
    }

    #[test]
    fn edt_005_serde_roundtrip() {
        let p = PlayheadState {
            active_tab: PreviewTab::SourceMedia,
            timeline_frame: 300,
            source_media_frame: 25,
            playing: true,
        };
        let json = serde_json::to_string(&p).unwrap();
        let restored: PlayheadState = serde_json::from_str(&json).unwrap();
        assert_eq!(p, restored);
    }
}

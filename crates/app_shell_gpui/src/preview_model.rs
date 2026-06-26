/// Preview panel tab selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreviewTab {
    Timeline,
}

/// Playback state for the preview transport — pure model, no UI dependency.
#[derive(Debug, Clone)]
pub struct PlaybackState {
    pub is_playing: bool,
    pub active_frame: i64,
    pub total_frames: i64,
    pub fps: i64,
    pub canvas_zoom: f64,
}

impl Default for PlaybackState {
    fn default() -> Self {
        Self::new()
    }
}

impl PlaybackState {
    pub fn new() -> Self {
        Self {
            is_playing: false,
            active_frame: 0,
            total_frames: 0,
            fps: 30,
            canvas_zoom: 1.0,
        }
    }

    pub fn play(&mut self) {
        self.is_playing = true;
    }

    pub fn pause(&mut self) {
        self.is_playing = false;
    }

    pub fn toggle_play(&mut self) {
        self.is_playing = !self.is_playing;
    }

    pub fn seek_to(&mut self, frame: i64) {
        let max = (self.total_frames - 1).max(0);
        self.active_frame = frame.clamp(0, max);
    }

    pub fn step_forward(&mut self) {
        self.seek_to(self.active_frame + 1);
    }

    pub fn step_backward(&mut self) {
        self.seek_to(self.active_frame - 1);
    }

    pub fn go_to_start(&mut self) {
        self.seek_to(0);
    }

    pub fn go_to_end(&mut self) {
        self.seek_to(self.total_frames - 1);
    }

    /// Returns 0.0..=1.0 position of the playhead.
    pub fn playhead_fraction(&self) -> f64 {
        if self.total_frames <= 1 {
            return 0.0;
        }
        self.active_frame as f64 / (self.total_frames - 1) as f64
    }

    /// Returns current position as HH:MM:SS:FF timecode string.
    pub fn format_timecode(&self) -> String {
        let fps = self.fps.max(1);
        let frame = self.active_frame;
        let ff = frame % fps;
        let total_seconds = frame / fps;
        let ss = total_seconds % 60;
        let mm = (total_seconds / 60) % 60;
        let hh = total_seconds / 3600;
        format!("{:02}:{:02}:{:02}:{:02}", hh, mm, ss, ff)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_state() {
        let s = PlaybackState::new();
        assert!(!s.is_playing);
        assert_eq!(s.active_frame, 0);
        assert_eq!(s.total_frames, 0);
        assert_eq!(s.fps, 30);
        assert!((s.canvas_zoom - 1.0).abs() < 1e-10);
    }

    #[test]
    fn play_and_pause() {
        let mut s = PlaybackState::new();
        s.play();
        assert!(s.is_playing);
        s.pause();
        assert!(!s.is_playing);
    }

    #[test]
    fn toggle_play() {
        let mut s = PlaybackState::new();
        assert!(!s.is_playing);
        s.toggle_play();
        assert!(s.is_playing);
        s.toggle_play();
        assert!(!s.is_playing);
    }

    #[test]
    fn seek_clamping_lower() {
        let mut s = PlaybackState::new();
        s.total_frames = 100;
        s.seek_to(-10);
        assert_eq!(s.active_frame, 0);
    }

    #[test]
    fn seek_clamping_upper() {
        let mut s = PlaybackState::new();
        s.total_frames = 100;
        s.seek_to(200);
        assert_eq!(s.active_frame, 99);
    }

    #[test]
    fn seek_zero_total_frames() {
        let mut s = PlaybackState::new();
        s.total_frames = 0;
        s.seek_to(5);
        assert_eq!(s.active_frame, 0);
    }

    #[test]
    fn go_to_start() {
        let mut s = PlaybackState::new();
        s.total_frames = 60;
        s.active_frame = 30;
        s.go_to_start();
        assert_eq!(s.active_frame, 0);
    }

    #[test]
    fn go_to_end() {
        let mut s = PlaybackState::new();
        s.total_frames = 60;
        s.go_to_end();
        assert_eq!(s.active_frame, 59);
    }

    #[test]
    fn step_forward_and_backward() {
        let mut s = PlaybackState::new();
        s.total_frames = 10;
        s.active_frame = 5;
        s.step_forward();
        assert_eq!(s.active_frame, 6);
        s.step_backward();
        assert_eq!(s.active_frame, 5);
    }

    #[test]
    fn playhead_fraction_zero_when_empty() {
        let s = PlaybackState::new();
        assert!((s.playhead_fraction() - 0.0).abs() < 1e-10);
    }

    #[test]
    fn playhead_fraction_midpoint() {
        let mut s = PlaybackState::new();
        s.total_frames = 101;
        s.active_frame = 50;
        let f = s.playhead_fraction();
        assert!((f - 0.5).abs() < 1e-6);
    }

    #[test]
    fn timecode_format_zero() {
        let s = PlaybackState::new();
        assert_eq!(s.format_timecode(), "00:00:00:00");
    }

    #[test]
    fn timecode_format_known_values() {
        let mut s = PlaybackState::new();
        s.fps = 30;
        s.total_frames = 10000;
        // 1 hour = 108000 frames at 30fps
        s.active_frame = 108000 + 30 * 61 + 15; // 01:01:01:15
        assert_eq!(s.format_timecode(), "01:01:01:15");
    }
}

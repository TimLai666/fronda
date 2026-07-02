//! Toolbar model — pure state, no UI dependency.
//!
//! Covers UIX-001 (38px height), UIX-002 (button set),
//! UIX-003 (Pointer/Razor shortcuts), UIX-007 (zoom bounds).

/// Editing tool mode. UIX-003: Pointer = V, Razor = C.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum ToolMode {
    #[default]
    Pointer,
    Razor,
}

/// Zoom bounds from UIX-007.
pub const ZOOM_MIN: f32 = 0.05;
pub const ZOOM_MAX: f32 = 40.0;
pub const ZOOM_DEFAULT: f32 = 1.0;

const _: () = {
    assert!(ZOOM_MIN < ZOOM_DEFAULT);
    assert!(ZOOM_DEFAULT < ZOOM_MAX);
};

/// Toolbar state — pure model, testable without gpui.
#[derive(Debug, Clone)]
pub struct ToolbarState {
    pub tool_mode: ToolMode,
    pub zoom_scale: f32,
    pub can_undo: bool,
    pub can_redo: bool,
}

impl ToolbarState {
    pub fn new() -> Self {
        Self {
            tool_mode: ToolMode::default(),
            zoom_scale: ZOOM_DEFAULT,
            can_undo: false,
            can_redo: false,
        }
    }

    pub fn set_tool_mode(&mut self, mode: ToolMode) {
        self.tool_mode = mode;
    }

    pub fn set_zoom(&mut self, scale: f32) {
        self.zoom_scale = Self::clamp_zoom(scale);
    }

    /// Clamp to UIX-007 bounds without mutating.
    pub fn clamp_zoom(scale: f32) -> f32 {
        scale.clamp(ZOOM_MIN, ZOOM_MAX)
    }

    pub fn set_undo_redo(&mut self, can_undo: bool, can_redo: bool) {
        self.can_undo = can_undo;
        self.can_redo = can_redo;
    }
}

impl Default for ToolbarState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toolbar_default_mode_is_pointer() {
        assert_eq!(ToolMode::default(), ToolMode::Pointer);
    }

    #[test]
    fn toolbar_set_tool_mode() {
        let mut s = ToolbarState::new();
        s.set_tool_mode(ToolMode::Razor);
        assert_eq!(s.tool_mode, ToolMode::Razor);
        s.set_tool_mode(ToolMode::Pointer);
        assert_eq!(s.tool_mode, ToolMode::Pointer);
    }

    #[test]
    fn toolbar_zoom_clamp_below_min() {
        assert!((ToolbarState::clamp_zoom(0.0) - ZOOM_MIN).abs() < 1e-6);
    }

    #[test]
    fn toolbar_zoom_clamp_above_max() {
        assert!((ToolbarState::clamp_zoom(100.0) - ZOOM_MAX).abs() < 1e-6);
    }

    #[test]
    fn toolbar_zoom_in_range_unchanged() {
        let v = 5.0_f32;
        assert!((ToolbarState::clamp_zoom(v) - v).abs() < 1e-6);
    }

    #[test]
    fn toolbar_set_zoom_clamps() {
        let mut s = ToolbarState::new();
        s.set_zoom(0.0);
        assert!((s.zoom_scale - ZOOM_MIN).abs() < 1e-6);
        s.set_zoom(1000.0);
        assert!((s.zoom_scale - ZOOM_MAX).abs() < 1e-6);
    }

    #[test]
    fn toolbar_undo_redo_start_false() {
        let s = ToolbarState::new();
        assert!(!s.can_undo);
        assert!(!s.can_redo);
    }

    #[test]
    fn toolbar_set_undo_redo() {
        let mut s = ToolbarState::new();
        s.set_undo_redo(true, false);
        assert!(s.can_undo);
        assert!(!s.can_redo);
        s.set_undo_redo(false, true);
        assert!(!s.can_undo);
        assert!(s.can_redo);
    }

    #[test]
    fn toolbar_zoom_bounds_ordered() {
        assert_eq!(ZOOM_MIN, 0.05);
        assert_eq!(ZOOM_DEFAULT, 1.0);
        assert_eq!(ZOOM_MAX, 40.0);
    }
}

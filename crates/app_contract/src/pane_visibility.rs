//! Pane visibility state (EDT-003).
//!
//! Persisted across launches so the editor restores which panes were visible.

use serde::{Deserialize, Serialize};

/// EDT-003: Visibility state for each editor pane.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PaneVisibilityState {
    pub media_pane_visible: bool,
    pub inspector_pane_visible: bool,
    pub agent_pane_visible: bool,
}

impl Default for PaneVisibilityState {
    fn default() -> Self {
        Self {
            media_pane_visible: true,
            inspector_pane_visible: true,
            agent_pane_visible: true,
        }
    }
}

impl PaneVisibilityState {
    /// All panes are visible.
    pub fn all_visible() -> Self {
        Self::default()
    }

    /// Only the media pane is visible (for tight layouts).
    pub fn media_only() -> Self {
        Self {
            media_pane_visible: true,
            inspector_pane_visible: false,
            agent_pane_visible: false,
        }
    }

    /// Toggle media pane visibility.
    pub fn toggle_media(&mut self) {
        self.media_pane_visible = !self.media_pane_visible;
    }

    /// Toggle inspector pane visibility.
    pub fn toggle_inspector(&mut self) {
        self.inspector_pane_visible = !self.inspector_pane_visible;
    }

    /// Toggle agent pane visibility.
    pub fn toggle_agent(&mut self) {
        self.agent_pane_visible = !self.agent_pane_visible;
    }

    /// Returns the number of visible panes.
    pub fn visible_count(&self) -> usize {
        [
            self.media_pane_visible,
            self.inspector_pane_visible,
            self.agent_pane_visible,
        ]
        .iter()
        .filter(|&&v| v)
        .count()
    }

    /// Returns true if all non-timeline/preview panes are collapsed.
    pub fn is_maximized(&self) -> bool {
        self.visible_count() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edt_003_default_all_visible() {
        let s = PaneVisibilityState::default();
        assert!(s.media_pane_visible);
        assert!(s.inspector_pane_visible);
        assert!(s.agent_pane_visible);
        assert_eq!(s.visible_count(), 3);
    }

    #[test]
    fn edt_003_media_only() {
        let s = PaneVisibilityState::media_only();
        assert!(s.media_pane_visible);
        assert!(!s.inspector_pane_visible);
        assert!(!s.agent_pane_visible);
        assert_eq!(s.visible_count(), 1);
    }

    #[test]
    fn edt_003_toggle_media() {
        let mut s = PaneVisibilityState::default();
        s.toggle_media();
        assert!(!s.media_pane_visible);
        s.toggle_media();
        assert!(s.media_pane_visible);
    }

    #[test]
    fn edt_003_toggle_inspector() {
        let mut s = PaneVisibilityState::default();
        s.toggle_inspector();
        assert!(!s.inspector_pane_visible);
    }

    #[test]
    fn edt_003_toggle_agent() {
        let mut s = PaneVisibilityState::default();
        s.toggle_agent();
        assert!(!s.agent_pane_visible);
    }

    #[test]
    fn edt_003_maximized_when_all_collapsed() {
        let mut s = PaneVisibilityState::default();
        s.toggle_media();
        s.toggle_inspector();
        s.toggle_agent();
        assert!(s.is_maximized());
        assert_eq!(s.visible_count(), 0);
    }

    #[test]
    fn edt_003_serde_roundtrip() {
        let s = PaneVisibilityState::media_only();
        let json = serde_json::to_string(&s).unwrap();
        let restored: PaneVisibilityState = serde_json::from_str(&json).unwrap();
        assert_eq!(s, restored);
    }
}

//! Media panel model — pure state, no UI dependency.
//!
//! Covers UIX-011 (media panel width constants) and
//! the three-tab structure from MediaPanelView.

/// The three tabs in the media panel.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MediaPanelTab {
    Media,
    Captions,
    Music,
}

impl MediaPanelTab {
    pub fn all() -> [MediaPanelTab; 3] {
        [
            MediaPanelTab::Media,
            MediaPanelTab::Captions,
            MediaPanelTab::Music,
        ]
    }

    /// SF Symbol icon name for the tab button.
    pub fn icon_name(&self) -> &'static str {
        match self {
            MediaPanelTab::Media => "folder",
            MediaPanelTab::Captions => "captions.bubble",
            MediaPanelTab::Music => "music.note",
        }
    }

    /// Display label.
    pub fn label(&self) -> &'static str {
        match self {
            MediaPanelTab::Media => "Media",
            MediaPanelTab::Captions => "Captions",
            MediaPanelTab::Music => "Music",
        }
    }
}

/// Media panel state — pure model, testable without gpui.
#[derive(Debug, Clone)]
pub struct MediaPanelState {
    pub active_tab: MediaPanelTab,
}

impl MediaPanelState {
    pub fn new() -> Self {
        Self {
            active_tab: MediaPanelTab::Media,
        }
    }

    pub fn select_tab(&mut self, tab: MediaPanelTab) {
        self.active_tab = tab;
    }

    pub fn is_active(&self, tab: &MediaPanelTab) -> bool {
        &self.active_tab == tab
    }
}

impl Default for MediaPanelState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_panel_default_tab_is_media() {
        let s = MediaPanelState::new();
        assert_eq!(s.active_tab, MediaPanelTab::Media);
    }

    #[test]
    fn media_panel_select_captions() {
        let mut s = MediaPanelState::new();
        s.select_tab(MediaPanelTab::Captions);
        assert_eq!(s.active_tab, MediaPanelTab::Captions);
    }

    #[test]
    fn media_panel_select_music() {
        let mut s = MediaPanelState::new();
        s.select_tab(MediaPanelTab::Music);
        assert_eq!(s.active_tab, MediaPanelTab::Music);
    }

    #[test]
    fn media_panel_all_tabs_count() {
        assert_eq!(MediaPanelTab::all().len(), 3);
    }

    #[test]
    fn media_panel_all_tabs_order() {
        let tabs = MediaPanelTab::all();
        assert_eq!(tabs[0], MediaPanelTab::Media);
        assert_eq!(tabs[1], MediaPanelTab::Captions);
        assert_eq!(tabs[2], MediaPanelTab::Music);
    }

    #[test]
    fn media_panel_is_active() {
        let mut s = MediaPanelState::new();
        assert!(s.is_active(&MediaPanelTab::Media));
        assert!(!s.is_active(&MediaPanelTab::Captions));
        s.select_tab(MediaPanelTab::Captions);
        assert!(!s.is_active(&MediaPanelTab::Media));
        assert!(s.is_active(&MediaPanelTab::Captions));
    }

    #[test]
    fn media_panel_tab_icons_defined() {
        for tab in MediaPanelTab::all() {
            assert!(!tab.icon_name().is_empty());
        }
    }

    #[test]
    fn media_panel_tab_labels_defined() {
        for tab in MediaPanelTab::all() {
            assert!(!tab.label().is_empty());
        }
    }
}

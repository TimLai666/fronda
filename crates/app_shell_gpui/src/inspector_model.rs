/// Inspector panel tabs matching Swift InspectorView.ClipTab.
///
/// Swift shows exactly 4 clip tabs: Text / Video / Audio / AI Edit.
/// Speed and Transform are collapsible sections within Video tab, not separate tabs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InspectorTab {
    /// Swift ClipTab.text — text clip properties
    Text,
    /// Swift ClipTab.video — visual clip: Volume, Transform, Playback
    Video,
    /// Swift ClipTab.audio — audio clip: Volume, Speed
    Audio,
    /// Swift ClipTab.ai — AI editing
    AiEdit,
}

impl InspectorTab {
    pub fn label(&self) -> &'static str {
        match self {
            InspectorTab::Text => "Text",
            InspectorTab::Video => "Video",
            InspectorTab::Audio => "Audio",
            InspectorTab::AiEdit => "AI Edit",
        }
    }

    pub fn all_tabs() -> &'static [InspectorTab] {
        &[
            InspectorTab::Text,
            InspectorTab::Video,
            InspectorTab::Audio,
            InspectorTab::AiEdit,
        ]
    }
}

/// Inspector panel state — pure model, testable without gpui.
#[derive(Debug, Clone)]
pub struct InspectorState {
    pub active_tab: InspectorTab,
    pub transform_expanded: bool,
    pub volume_expanded: bool,
    pub speed_expanded: bool,
}

impl Default for InspectorState {
    fn default() -> Self {
        Self::new()
    }
}

impl InspectorState {
    pub fn new() -> Self {
        Self {
            active_tab: InspectorTab::Video,
            transform_expanded: true,
            volume_expanded: true,
            speed_expanded: false,
        }
    }

    pub fn select_tab(&mut self, tab: InspectorTab) {
        self.active_tab = tab;
    }

    pub fn toggle_transform(&mut self) {
        self.transform_expanded = !self.transform_expanded;
    }

    pub fn toggle_volume(&mut self) {
        self.volume_expanded = !self.volume_expanded;
    }

    pub fn toggle_speed(&mut self) {
        self.speed_expanded = !self.speed_expanded;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_tab_is_video() {
        let s = InspectorState::new();
        assert_eq!(s.active_tab, InspectorTab::Video);
    }

    #[test]
    fn select_tab_changes_active() {
        let mut s = InspectorState::new();
        s.select_tab(InspectorTab::Audio);
        assert_eq!(s.active_tab, InspectorTab::Audio);
        s.select_tab(InspectorTab::AiEdit);
        assert_eq!(s.active_tab, InspectorTab::AiEdit);
    }

    #[test]
    fn toggle_transform_expands_and_collapses() {
        let mut s = InspectorState::new();
        let initial = s.transform_expanded;
        s.toggle_transform();
        assert_eq!(s.transform_expanded, !initial);
        s.toggle_transform();
        assert_eq!(s.transform_expanded, initial);
    }

    #[test]
    fn toggle_volume_expands_and_collapses() {
        let mut s = InspectorState::new();
        let initial = s.volume_expanded;
        s.toggle_volume();
        assert_eq!(s.volume_expanded, !initial);
        s.toggle_volume();
        assert_eq!(s.volume_expanded, initial);
    }

    #[test]
    fn toggle_speed_expands_and_collapses() {
        let mut s = InspectorState::new();
        let initial = s.speed_expanded;
        s.toggle_speed();
        assert_eq!(s.speed_expanded, !initial);
        s.toggle_speed();
        assert_eq!(s.speed_expanded, initial);
    }

    #[test]
    fn all_tabs_count() {
        // Swift ClipTab has exactly 4 tabs: Text / Video / Audio / AI Edit
        assert_eq!(InspectorTab::all_tabs().len(), 4);
    }

    #[test]
    fn all_tabs_labels_non_empty() {
        for tab in InspectorTab::all_tabs() {
            assert!(!tab.label().is_empty());
        }
    }

    #[test]
    fn all_tabs_order() {
        let tabs = InspectorTab::all_tabs();
        assert_eq!(tabs[0], InspectorTab::Text);
        assert_eq!(tabs[3], InspectorTab::AiEdit);
    }
}

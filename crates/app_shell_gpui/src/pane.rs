/// Pane identifiers matching the 5 functional panes (EDT-001).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneId {
    Media,
    Preview,
    Inspector,
    Timeline,
    Agent,
}

/// Layout presets (EDT-002).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LayoutPreset {
    Default,
    Media,
    Vertical,
}

/// Visibility state for each pane.
#[derive(Debug, Clone, PartialEq)]
pub struct PaneVisibility {
    pub media: bool,
    pub preview: bool,
    pub inspector: bool,
    pub timeline: bool,
    pub agent: bool,
}

impl PaneVisibility {
    pub fn all_visible() -> Self {
        Self {
            media: true,
            preview: true,
            inspector: true,
            timeline: true,
            agent: true,
        }
    }

    pub fn default_visibility() -> Self {
        Self::all_visible()
    }
}

/// Pane layout state.
#[derive(Debug, Clone)]
pub struct PaneLayout {
    pub visibility: PaneVisibility,
    pub preset: LayoutPreset,
    pub maximized_pane: Option<PaneId>,
    /// Persistent visibility state before maximize (EDT-004).
    pub pre_maximize_visibility: Option<PaneVisibility>,
}

impl PaneLayout {
    pub fn new() -> Self {
        Self {
            visibility: PaneVisibility::default_visibility(),
            preset: LayoutPreset::Default,
            maximized_pane: None,
            pre_maximize_visibility: None,
        }
    }

    pub fn apply_preset(&mut self, preset: LayoutPreset) {
        // Swift rebuilds the split tree on preset change but keeps each pane's
        // visibility flag; only explicit toggles/maximize touch visibility.
        self.preset = preset;
    }

    pub fn toggle_pane(&mut self, pane: PaneId) {
        if self.is_maximized() {
            return;
        }
        match pane {
            PaneId::Media => self.visibility.media = !self.visibility.media,
            PaneId::Preview => self.visibility.preview = !self.visibility.preview,
            PaneId::Inspector => self.visibility.inspector = !self.visibility.inspector,
            PaneId::Timeline => self.visibility.timeline = !self.visibility.timeline,
            PaneId::Agent => self.visibility.agent = !self.visibility.agent,
        }
    }

    pub fn maximize(&mut self, pane: PaneId) {
        if self.is_maximized() {
            return;
        }
        self.pre_maximize_visibility = Some(self.visibility.clone());
        self.maximized_pane = Some(pane);
        self.visibility = PaneVisibility {
            media: pane == PaneId::Media,
            preview: pane == PaneId::Preview,
            inspector: pane == PaneId::Inspector,
            timeline: pane == PaneId::Timeline,
            agent: pane == PaneId::Agent,
        };
    }

    pub fn unmaximize(&mut self) {
        if let Some(saved) = self.pre_maximize_visibility.take() {
            self.visibility = saved;
        }
        self.maximized_pane = None;
    }

    pub fn is_maximized(&self) -> bool {
        self.maximized_pane.is_some()
    }

    pub fn is_visible(&self, pane: PaneId) -> bool {
        match pane {
            PaneId::Media => self.visibility.media,
            PaneId::Preview => self.visibility.preview,
            PaneId::Inspector => self.visibility.inspector,
            PaneId::Timeline => self.visibility.timeline,
            PaneId::Agent => self.visibility.agent,
        }
    }

    pub fn visible_count(&self) -> usize {
        let mut count = 0;
        if self.visibility.media {
            count += 1;
        }
        if self.visibility.preview {
            count += 1;
        }
        if self.visibility.inspector {
            count += 1;
        }
        if self.visibility.timeline {
            count += 1;
        }
        if self.visibility.agent {
            count += 1;
        }
        count
    }
}

impl Default for PaneLayout {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edt_001_five_panes() {
        // All 5 pane IDs exist and are distinct
        let ids = [
            PaneId::Media,
            PaneId::Preview,
            PaneId::Inspector,
            PaneId::Timeline,
            PaneId::Agent,
        ];
        assert_eq!(ids.len(), 5);
        for i in 0..ids.len() {
            for j in (i + 1)..ids.len() {
                assert_ne!(ids[i], ids[j], "pane variants must be distinct");
            }
        }
    }

    #[test]
    fn edt_002_default_layout_preset() {
        let layout = PaneLayout::new();
        assert_eq!(layout.preset, LayoutPreset::Default);
        assert!(layout.visibility.media);
        assert!(layout.visibility.preview);
        assert!(layout.visibility.inspector);
        assert!(layout.visibility.timeline);
        assert!(layout.visibility.agent);
    }

    #[test]
    fn edt_002_media_preset() {
        let mut layout = PaneLayout::new();
        layout.apply_preset(LayoutPreset::Media);
        assert_eq!(layout.preset, LayoutPreset::Media);
        // Preset switches rearrange panes but never touch visibility (Swift parity).
        assert!(layout.visibility.media);
        assert!(layout.visibility.preview);
        assert!(layout.visibility.inspector);
        assert!(layout.visibility.timeline);
        assert!(layout.visibility.agent);
    }

    #[test]
    fn preset_switch_preserves_visibility() {
        let mut layout = PaneLayout::new();
        layout.toggle_pane(PaneId::Agent);
        layout.toggle_pane(PaneId::Inspector);
        let before = layout.visibility.clone();
        for preset in [
            LayoutPreset::Media,
            LayoutPreset::Vertical,
            LayoutPreset::Default,
        ] {
            layout.apply_preset(preset);
            assert_eq!(layout.visibility, before, "visibility changed by {preset:?}");
        }
    }

    #[test]
    fn edt_003_pane_visibility_toggle() {
        let mut layout = PaneLayout::new();
        assert!(layout.visibility.media);
        layout.toggle_pane(PaneId::Media);
        assert!(!layout.visibility.media);
        layout.toggle_pane(PaneId::Media);
        assert!(layout.visibility.media);
    }

    #[test]
    fn edt_004_maximize_collapses_others() {
        let mut layout = PaneLayout::new();
        // Toggle agent off first so pre-maximize state is non-trivial
        layout.toggle_pane(PaneId::Agent);
        assert!(!layout.visibility.agent);

        layout.maximize(PaneId::Timeline);
        assert_eq!(layout.maximized_pane, Some(PaneId::Timeline));
        assert!(layout.visibility.timeline);
        assert!(!layout.visibility.media);
        assert!(!layout.visibility.preview);
        assert!(!layout.visibility.inspector);
        assert!(!layout.visibility.agent);

        // pre-maximize snapshot captures agent=false
        let saved = layout.pre_maximize_visibility.as_ref().unwrap();
        assert!(!saved.agent);

        layout.unmaximize();
        assert!(layout.maximized_pane.is_none());
        assert!(layout.visibility.media);
        assert!(layout.visibility.preview);
        assert!(layout.visibility.inspector);
        assert!(layout.visibility.timeline);
        assert!(!layout.visibility.agent);
    }

    #[test]
    fn edt_004_maximize_no_op_when_already_maximized() {
        let mut layout = PaneLayout::new();
        layout.maximize(PaneId::Preview);
        let saved_state = layout.pre_maximize_visibility.clone();

        // Second maximize is a no-op
        layout.maximize(PaneId::Timeline);
        assert_eq!(layout.maximized_pane, Some(PaneId::Preview));
        assert_eq!(layout.pre_maximize_visibility, saved_state);
    }

    #[test]
    fn edt_004_maximize_then_toggle_pane() {
        let mut layout = PaneLayout::new();
        layout.maximize(PaneId::Media);
        // Toggle is blocked while maximized
        assert!(layout.visibility.media);
        layout.toggle_pane(PaneId::Media);
        assert!(layout.visibility.media);
    }
}

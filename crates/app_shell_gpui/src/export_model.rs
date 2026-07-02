//! Export panel model for Fronda's gpui UI (Issue #166).
//!
//! Mirrors Swift ExportMode / VideoCodec / ExportView state without any
//! platform dependencies. The view reads this and mutates it via actions.

use generation_core::export_panel::{ExportPanelState, ExportStage};
use render_core::{ExportFormat, ExportResolution};
use serde::{Deserialize, Serialize};

/// Export output mode — what gets written to disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ExportMode {
    #[default]
    Video,
    /// XMEML timeline interchange (for FCP, Premiere, DaVinci).
    Xml,
    /// `.palmier` project bundle.
    PalmierProject,
}

impl ExportMode {
    pub fn label(self) -> &'static str {
        match self {
            ExportMode::Video => "Video (.mp4 / .mov)",
            ExportMode::Xml => "Timeline (.xml)",
            ExportMode::PalmierProject => "Palmier Project (.palmier)",
        }
    }

    pub fn all() -> &'static [ExportMode] {
        &[
            ExportMode::Video,
            ExportMode::Xml,
            ExportMode::PalmierProject,
        ]
    }
}

/// Complete export panel state used by the gpui view.
///
/// Wraps `ExportPanelState` (the pure state machine from generation_core)
/// and adds UI-only fields like the selected mode and the thumbnail url.
#[derive(Debug, Clone, Default)]
pub struct ExportViewModel {
    pub mode: ExportMode,
    pub panel: ExportPanelState,
    /// Thumbnail data key/path — resolved by platform adapter.
    pub thumbnail_asset_key: Option<String>,
    /// Whether the settings panel is expanded (vs. collapsed to progress view).
    pub settings_expanded: bool,
    /// Number of project media files not found on disk (PalmierProject mode).
    /// Mirrors Swift ExportView's `palmierSummary.missing` red warning.
    pub missing_file_count: usize,
}

impl ExportViewModel {
    pub fn new() -> Self {
        Self {
            mode: ExportMode::Video,
            panel: ExportPanelState::new(),
            thumbnail_asset_key: None,
            settings_expanded: true,
            missing_file_count: 0,
        }
    }

    pub fn set_mode(&mut self, mode: ExportMode) {
        self.mode = mode;
    }

    pub fn set_resolution(&mut self, resolution: ExportResolution) {
        self.panel.settings.resolution = resolution;
    }

    pub fn set_format(&mut self, format: ExportFormat) {
        self.panel.settings.format = format;
    }

    pub fn can_start_export(&self) -> bool {
        self.panel.settings_valid() && self.panel.stage != ExportStage::Exporting
    }

    pub fn start(&mut self) {
        self.panel.start_export();
        self.settings_expanded = false;
    }

    pub fn progress_fraction(&self) -> f64 {
        self.panel.progress
    }

    pub fn status_text(&self) -> Option<&str> {
        self.panel.status_message.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_model_default_mode_is_video() {
        let vm = ExportViewModel::new();
        assert_eq!(vm.mode, ExportMode::Video);
        assert!(vm.settings_expanded);
    }

    #[test]
    fn export_mode_all_has_three_entries() {
        assert_eq!(ExportMode::all().len(), 3);
    }

    #[test]
    fn export_mode_labels_non_empty() {
        for m in ExportMode::all() {
            assert!(!m.label().is_empty(), "{m:?} has no label");
        }
    }

    #[test]
    fn can_start_export_initially_true_for_valid_settings() {
        let vm = ExportViewModel::new();
        assert!(
            vm.can_start_export(),
            "default settings (H264+SDR) are valid"
        );
    }

    #[test]
    fn start_transitions_to_exporting() {
        let mut vm = ExportViewModel::new();
        vm.start();
        assert!(!vm.settings_expanded);
        assert_eq!(vm.panel.stage, ExportStage::Exporting);
        assert!(
            !vm.can_start_export(),
            "already exporting — can't start again"
        );
    }

    #[test]
    fn set_mode_updates_mode() {
        let mut vm = ExportViewModel::new();
        vm.set_mode(ExportMode::Xml);
        assert_eq!(vm.mode, ExportMode::Xml);
    }
}

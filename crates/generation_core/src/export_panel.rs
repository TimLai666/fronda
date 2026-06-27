//! Export panel state model (Issue #166).
//!
//! Tracks the export job pipeline: user configures settings, panel shows
//! progress, then completion/error state. No platform or file-system calls.

use render_core::{ColorSpace, ExportFormat, ExportResolution};
use serde::{Deserialize, Serialize};

/// The current stage of the export panel workflow (Issue #166).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExportStage {
    /// User is configuring export settings (default).
    Configure,
    /// Export is running — `progress` (0.0..=1.0) and `message` available.
    Exporting,
    /// Export completed successfully.
    Done,
    /// Export failed.
    Failed,
}

impl Default for ExportStage {
    fn default() -> Self {
        ExportStage::Configure
    }
}

/// User-configurable export settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExportSettings {
    pub resolution: ExportResolution,
    pub format: ExportFormat,
    pub color_space: ColorSpace,
    /// Target output file path (set by a file-dialog adapter; not chosen here).
    pub output_path: Option<String>,
}

impl Default for ExportSettings {
    fn default() -> Self {
        Self {
            resolution: ExportResolution::R1080p,
            format: ExportFormat::H264,
            color_space: ColorSpace::Sdr,
            output_path: None,
        }
    }
}

/// Export panel view-model (Issue #166).
///
/// Pure state — no side effects. The platform adapter drives transitions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportPanelState {
    pub settings: ExportSettings,
    pub stage: ExportStage,
    /// Export progress, 0.0–1.0. Meaningful only in `Exporting` stage.
    pub progress: f64,
    /// Human-readable status message shown below the progress bar.
    pub status_message: Option<String>,
    /// Error description. Meaningful only in `Failed` stage.
    pub error: Option<String>,
}

impl Default for ExportPanelState {
    fn default() -> Self {
        Self {
            settings: ExportSettings::default(),
            stage: ExportStage::Configure,
            progress: 0.0,
            status_message: None,
            error: None,
        }
    }
}

impl ExportPanelState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Begin an export with the current settings.
    pub fn start_export(&mut self) {
        self.stage = ExportStage::Exporting;
        self.progress = 0.0;
        self.status_message = Some("Preparing…".into());
        self.error = None;
    }

    /// Update progress (0.0..=1.0) and optional status string.
    pub fn update_progress(&mut self, fraction: f64, message: Option<String>) {
        self.progress = fraction.clamp(0.0, 1.0);
        self.status_message = message;
    }

    /// Mark the export as complete.
    pub fn finish(&mut self) {
        self.stage = ExportStage::Done;
        self.progress = 1.0;
        self.status_message = Some("Export complete.".into());
    }

    /// Mark the export as failed with a reason.
    pub fn fail(&mut self, reason: impl Into<String>) {
        self.stage = ExportStage::Failed;
        self.error = Some(reason.into());
        self.status_message = None;
    }

    /// Reset to Configure stage for a new export.
    pub fn reset(&mut self) {
        self.stage = ExportStage::Configure;
        self.progress = 0.0;
        self.status_message = None;
        self.error = None;
    }

    /// Whether the current settings are valid (format ↔ color_space match).
    pub fn settings_valid(&self) -> bool {
        render_core::validate_export_color_space(
            self.settings.format,
            self.settings.color_space,
        )
        .is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn issue_166_default_is_configure_stage() {
        let panel = ExportPanelState::new();
        assert_eq!(panel.stage, ExportStage::Configure);
        assert!((panel.progress - 0.0).abs() < 1e-9);
        assert!(panel.error.is_none());
    }

    #[test]
    fn issue_166_start_export_transitions_to_exporting() {
        let mut panel = ExportPanelState::new();
        panel.start_export();
        assert_eq!(panel.stage, ExportStage::Exporting);
        assert!((panel.progress - 0.0).abs() < 1e-9);
        assert!(panel.status_message.is_some());
    }

    #[test]
    fn issue_166_update_progress_clamps() {
        let mut panel = ExportPanelState::new();
        panel.start_export();
        panel.update_progress(1.5, Some("Almost done".into()));
        assert!((panel.progress - 1.0).abs() < 1e-9, "clamped to 1.0");
        panel.update_progress(-0.5, None);
        assert!((panel.progress - 0.0).abs() < 1e-9, "clamped to 0.0");
    }

    #[test]
    fn issue_166_finish_sets_done_stage() {
        let mut panel = ExportPanelState::new();
        panel.start_export();
        panel.update_progress(0.5, None);
        panel.finish();
        assert_eq!(panel.stage, ExportStage::Done);
        assert!((panel.progress - 1.0).abs() < 1e-9);
        assert!(panel.status_message.is_some());
        assert!(panel.error.is_none());
    }

    #[test]
    fn issue_166_fail_sets_failed_stage() {
        let mut panel = ExportPanelState::new();
        panel.start_export();
        panel.fail("Disk full");
        assert_eq!(panel.stage, ExportStage::Failed);
        assert_eq!(panel.error.as_deref(), Some("Disk full"));
        assert!(panel.status_message.is_none());
    }

    #[test]
    fn issue_166_reset_returns_to_configure() {
        let mut panel = ExportPanelState::new();
        panel.start_export();
        panel.finish();
        panel.reset();
        assert_eq!(panel.stage, ExportStage::Configure);
        assert!(panel.error.is_none());
        assert!((panel.progress - 0.0).abs() < 1e-9);
    }

    #[test]
    fn issue_166_settings_valid_h264_sdr() {
        let panel = ExportPanelState::new();
        assert!(
            panel.settings_valid(),
            "H264 + SDR must be valid"
        );
    }

    #[test]
    fn issue_166_settings_invalid_h264_hdr() {
        let mut panel = ExportPanelState::new();
        panel.settings.format = ExportFormat::H264;
        panel.settings.color_space = ColorSpace::Hlg;
        assert!(
            !panel.settings_valid(),
            "H264 + HLG must be invalid (H264 is SDR-only)"
        );
    }

    #[test]
    fn issue_166_settings_valid_h265hdr_pq() {
        let mut panel = ExportPanelState::new();
        panel.settings.format = ExportFormat::H265Hdr;
        panel.settings.color_space = ColorSpace::Pq;
        assert!(panel.settings_valid(), "H265Hdr + PQ must be valid");
    }

    #[test]
    fn issue_166_serde_roundtrip() {
        let mut panel = ExportPanelState::new();
        panel.start_export();
        panel.update_progress(0.42, Some("Encoding…".into()));
        let json = serde_json::to_string(&panel).unwrap();
        let restored: ExportPanelState = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.stage, ExportStage::Exporting);
        assert!((restored.progress - 0.42).abs() < 1e-6);
    }

    #[test]
    fn issue_166_default_export_settings() {
        let s = ExportSettings::default();
        assert_eq!(s.resolution, ExportResolution::R1080p);
        assert_eq!(s.format, ExportFormat::H264);
        assert_eq!(s.color_space, ColorSpace::Sdr);
        assert!(s.output_path.is_none());
    }
}

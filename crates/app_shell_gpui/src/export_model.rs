//! Export panel model for Fronda's gpui UI (Issue #166).
//!
//! Mirrors Swift ExportMode / VideoCodec / ExportView state without any
//! platform dependencies. The view reads this and mutates it via actions.

use core_model::{MediaManifest, Timeline};
use generation_core::export_panel::{ExportPanelState, ExportStage};
use render_core::fcpxml_export::{FcpxmlExport, FcpxmlTarget};
use render_core::xml_export::XmlExport;
use render_core::{ExportFormat, ExportResolution};
use serde::{Deserialize, Serialize};

/// Export output mode — what gets written to disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ExportMode {
    #[default]
    Video,
    /// XMEML timeline interchange (for FCP 7, Premiere, DaVinci).
    Xml,
    /// FCPXML 1.10 timeline interchange (for Final Cut Pro X, DaVinci Resolve).
    Fcpxml,
    /// `.palmier` project bundle.
    PalmierProject,
}

impl ExportMode {
    pub fn label(self) -> &'static str {
        match self {
            ExportMode::Video => "Video (.mp4 / .mov)",
            ExportMode::Xml => "Timeline (.xml)",
            ExportMode::Fcpxml => "Final Cut Pro (.fcpxml)",
            ExportMode::PalmierProject => "Palmier Project (.palmier)",
        }
    }

    pub fn all() -> &'static [ExportMode] {
        &[
            ExportMode::Video,
            ExportMode::Xml,
            ExportMode::Fcpxml,
            ExportMode::PalmierProject,
        ]
    }

    /// File extension for the text interchange modes; `None` for Video and
    /// the `.palmier` bundle (handled by their own writers).
    pub fn interchange_extension(self) -> Option<&'static str> {
        match self {
            ExportMode::Xml => Some("xml"),
            ExportMode::Fcpxml => Some("fcpxml"),
            ExportMode::Video | ExportMode::PalmierProject => None,
        }
    }
}

/// Generate the interchange-file text for the interchange export modes.
/// Returns `None` for modes that do not produce a text interchange file.
pub fn interchange_content(
    mode: ExportMode,
    timeline: &Timeline,
    manifest: &MediaManifest,
    timelines: &std::collections::HashMap<String, Timeline>,
    fcpxml_target: FcpxmlTarget,
) -> Option<String> {
    match mode {
        ExportMode::Xml => Some(XmlExport::export_with_manifest_and_timelines(
            timeline, manifest, timelines,
        )),
        ExportMode::Fcpxml => Some(FcpxmlExport::export_with_target_and_timelines(
            timeline,
            manifest,
            fcpxml_target,
            timelines,
        )),
        ExportMode::Video | ExportMode::PalmierProject => None,
    }
}

/// Generate and write the interchange file for `mode` to `path`.
/// Errors if `mode` is not a text interchange mode or the write fails.
pub fn write_interchange(
    mode: ExportMode,
    timeline: &Timeline,
    manifest: &MediaManifest,
    timelines: &std::collections::HashMap<String, Timeline>,
    path: &std::path::Path,
    fcpxml_target: FcpxmlTarget,
) -> Result<(), String> {
    let content = interchange_content(mode, timeline, manifest, timelines, fcpxml_target)
        .ok_or_else(|| format!("{mode:?} is not a text interchange mode"))?;
    std::fs::write(path, content).map_err(|e| format!("Failed to write {}: {e}", path.display()))
}

/// Write a `.palmier` project bundle to `root` (creating it), containing
/// `project.json` and `media.json` for the current timeline + manifest.
/// This is an export — it does not change the currently open project.
pub fn write_palmier_bundle(
    root: &std::path::Path,
    timeline: &Timeline,
    manifest: &MediaManifest,
) -> Result<(), String> {
    project_io::save_project_state(root, timeline, manifest).map_err(|e| e.to_string())
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
    /// Which NLE an FCPXML export is calibrated for (#254). Resolve is the default; the export
    /// dialog surfaces a "DaVinci Resolve / Final Cut Pro" selector for the Fcpxml mode.
    pub fcpxml_target: FcpxmlTarget,
    /// 10-bit HDR video export (Issue #138): when set and the codec is H.265,
    /// the encoder writes HEVC Main10 with BT.2020 + HLG tags instead of 8-bit
    /// SDR. Ignored for non-HEVC codecs (H.264 / ProRes).
    pub hdr: bool,
}

impl ExportViewModel {
    pub fn new() -> Self {
        Self {
            mode: ExportMode::Video,
            panel: ExportPanelState::new(),
            thumbnail_asset_key: None,
            settings_expanded: true,
            missing_file_count: 0,
            fcpxml_target: FcpxmlTarget::Resolve,
            hdr: false,
        }
    }

    pub fn set_mode(&mut self, mode: ExportMode) {
        self.mode = mode;
    }

    pub fn set_fcpxml_target(&mut self, target: FcpxmlTarget) {
        self.fcpxml_target = target;
    }

    /// Toggle 10-bit HDR video export (Issue #138). Only affects H.265 output.
    pub fn set_hdr(&mut self, hdr: bool) {
        self.hdr = hdr;
    }

    /// The effective video codec for the current format + HDR toggle: an
    /// HDR-enabled H.265 selection resolves to HEVC Main10 (Issue #138).
    pub fn effective_video_codec(&self) -> render_core::ExportFormat {
        match self.panel.settings.format {
            render_core::ExportFormat::H265 | render_core::ExportFormat::H265Hdr => {
                if self.hdr {
                    render_core::ExportFormat::H265Hdr
                } else {
                    render_core::ExportFormat::H265
                }
            }
            other => other,
        }
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

    /// Record the outcome of an interchange (Xml/Fcpxml) export so the view can
    /// show success or failure instead of silently finishing.
    pub fn set_interchange_result(&mut self, result: Result<std::path::PathBuf, String>) {
        match result {
            Ok(path) => {
                self.panel.stage = ExportStage::Done;
                self.panel.status_message = Some(format!("Exported to {}", path.display()));
            }
            Err(reason) => {
                self.panel.stage = ExportStage::Failed;
                self.panel.status_message = Some(format!("Export failed: {reason}"));
            }
        }
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
    fn export_mode_all_has_four_entries() {
        assert_eq!(ExportMode::all().len(), 4);
    }

    #[test]
    fn interchange_content_matches_mode() {
        let tl = Timeline::default();
        let m = MediaManifest::default();
        let xml = interchange_content(
            ExportMode::Xml,
            &tl,
            &m,
            &Default::default(),
            FcpxmlTarget::Resolve,
        )
        .unwrap();
        assert!(xml.contains("<xmeml"), "Xml mode produces XMEML");
        let fcp = interchange_content(
            ExportMode::Fcpxml,
            &tl,
            &m,
            &Default::default(),
            FcpxmlTarget::Resolve,
        )
        .unwrap();
        assert!(
            fcp.contains("<fcpxml version=\"1.10\">"),
            "Fcpxml mode produces FCPXML"
        );
        // The Fcp target is reachable through the model and produces a valid FCPXML too.
        let fcp2 = interchange_content(
            ExportMode::Fcpxml,
            &tl,
            &m,
            &Default::default(),
            FcpxmlTarget::Fcp,
        )
        .unwrap();
        assert!(fcp2.contains("<fcpxml version=\"1.10\">"));
        assert!(interchange_content(
            ExportMode::Video,
            &tl,
            &m,
            &Default::default(),
            FcpxmlTarget::Resolve
        )
        .is_none());
        assert!(interchange_content(
            ExportMode::PalmierProject,
            &tl,
            &m,
            &Default::default(),
            FcpxmlTarget::Resolve
        )
        .is_none());
    }

    #[test]
    fn export_view_model_defaults_to_resolve_target() {
        let vm = ExportViewModel::new();
        assert_eq!(vm.fcpxml_target, FcpxmlTarget::Resolve);
        let mut vm = vm;
        vm.set_fcpxml_target(FcpxmlTarget::Fcp);
        assert_eq!(vm.fcpxml_target, FcpxmlTarget::Fcp);
    }

    #[test]
    fn interchange_extensions() {
        assert_eq!(ExportMode::Xml.interchange_extension(), Some("xml"));
        assert_eq!(ExportMode::Fcpxml.interchange_extension(), Some("fcpxml"));
        assert_eq!(ExportMode::Video.interchange_extension(), None);
    }

    #[test]
    fn write_interchange_writes_fcpxml_file() {
        let dir = std::env::temp_dir().join("fronda-export-model-tests");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("timeline.fcpxml");
        let _ = std::fs::remove_file(&path);

        write_interchange(
            ExportMode::Fcpxml,
            &Timeline::default(),
            &MediaManifest::default(),
            &Default::default(),
            &path,
            FcpxmlTarget::Resolve,
        )
        .unwrap();

        let written = std::fs::read_to_string(&path).unwrap();
        assert!(written.contains("<fcpxml version=\"1.10\">"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn set_interchange_result_reports_success_and_failure() {
        let mut vm = ExportViewModel::new();
        vm.set_interchange_result(Ok(std::path::PathBuf::from("/tmp/Timeline.fcpxml")));
        assert_eq!(vm.panel.stage, ExportStage::Done);
        assert!(vm.status_text().unwrap().contains("Exported to"));
        assert!(vm.status_text().unwrap().contains("Timeline.fcpxml"));

        vm.set_interchange_result(Err("disk full".into()));
        assert_eq!(vm.panel.stage, ExportStage::Failed);
        assert!(vm
            .status_text()
            .unwrap()
            .contains("Export failed: disk full"));
    }

    #[test]
    fn write_palmier_bundle_writes_project_files() {
        let dir = std::env::temp_dir().join("fronda-export-model-tests/bundle.palmier");
        let _ = std::fs::remove_dir_all(&dir);
        write_palmier_bundle(&dir, &Timeline::default(), &MediaManifest::default()).unwrap();
        assert!(dir.join("project.json").is_file(), "project.json written");
        assert!(dir.join("media.json").is_file(), "media.json written");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_interchange_rejects_video_mode() {
        let path = std::env::temp_dir().join("fronda-export-should-not-exist.bin");
        let err = write_interchange(
            ExportMode::Video,
            &Timeline::default(),
            &MediaManifest::default(),
            &Default::default(),
            &path,
            FcpxmlTarget::Resolve,
        )
        .unwrap_err();
        assert!(err.contains("not a text interchange"), "err={err}");
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

    #[test]
    fn hdr_toggle_upgrades_h265_to_main10() {
        let mut vm = ExportViewModel::new();
        assert!(!vm.hdr, "HDR off by default");
        vm.set_format(ExportFormat::H265);
        assert_eq!(
            vm.effective_video_codec(),
            ExportFormat::H265,
            "SDR H.265 stays 8-bit"
        );
        vm.set_hdr(true);
        assert_eq!(
            vm.effective_video_codec(),
            ExportFormat::H265Hdr,
            "HDR upgrades H.265 → HEVC Main10"
        );
    }

    #[test]
    fn hdr_toggle_ignored_for_non_hevc_codecs() {
        let mut vm = ExportViewModel::new();
        vm.set_hdr(true);
        vm.set_format(ExportFormat::H264);
        assert_eq!(
            vm.effective_video_codec(),
            ExportFormat::H264,
            "H.264 ignores the HDR toggle"
        );
        vm.set_format(ExportFormat::ProRes);
        assert_eq!(vm.effective_video_codec(), ExportFormat::ProRes);
    }
}

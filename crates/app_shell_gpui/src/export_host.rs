//! Host `ExportHost` for the `export_project` agent tool: performs the actual
//! render/write with the same pipelines as the Export dialog. Video renders on
//! a background thread and reports `Started`; interchange and package writes
//! finish inline.

use agent_contract::{ExportHost, ExportOutcome, ExportRequest};
use render_core::fcpxml_export::FcpxmlTarget;
use render_core::ExportResolution;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;

pub struct AgentExportHost {
    project_root: PathBuf,
}

impl AgentExportHost {
    pub fn new(project_root: PathBuf) -> Self {
        Self { project_root }
    }

    fn project_name(&self) -> String {
        self.project_root
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Project")
            .to_string()
    }

    /// Resolve the destination: an explicit path (mode extension appended when
    /// missing) or a unique project-named file in Downloads.
    fn resolve_output(&self, request: &ExportRequest, ext: &str) -> Result<PathBuf, String> {
        if let Some(raw) = &request.output_path {
            let mut path = PathBuf::from(raw);
            if path.extension().is_none() {
                path.set_extension(ext);
            }
            if !request.overwrite && path.exists() {
                return Err(format!(
                    "export_project: '{}' already exists and overwrite=false.",
                    path.display()
                ));
            }
            if let Some(parent) = path.parent() {
                if !parent.as_os_str().is_empty() && !parent.is_dir() {
                    return Err(format!(
                        "export_project: destination folder '{}' does not exist.",
                        parent.display()
                    ));
                }
            }
            return Ok(path);
        }
        let downloads = std::env::home_dir()
            .map(|h| h.join("Downloads"))
            .filter(|d| d.is_dir())
            .unwrap_or_else(std::env::temp_dir);
        let base = self.project_name();
        let mut candidate = downloads.join(format!("{base}.{ext}"));
        let mut n = 2;
        while candidate.exists() {
            candidate = downloads.join(format!("{base} {n}.{ext}"));
            n += 1;
        }
        Ok(candidate)
    }
}

impl ExportHost for AgentExportHost {
    fn export(&self, request: ExportRequest) -> Result<ExportOutcome, String> {
        let timelines = timeline_core::timeline_resolver(&request.sibling_timelines);
        match request.mode.as_str() {
            "xml" | "fcpxml" => {
                let ext = if request.mode == "xml" {
                    "xml"
                } else {
                    "fcpxml"
                };
                let path = self.resolve_output(&request, ext)?;
                let target = if request.fcpxml_target == "fcp" {
                    FcpxmlTarget::Fcp
                } else {
                    FcpxmlTarget::Resolve
                };
                let mode = if request.mode == "xml" {
                    crate::export_model::ExportMode::Xml
                } else {
                    crate::export_model::ExportMode::Fcpxml
                };
                crate::export_model::write_interchange(
                    mode,
                    &request.timeline,
                    &request.manifest,
                    &timelines,
                    &path,
                    target,
                )?;
                Ok(ExportOutcome::Completed {
                    path: path.display().to_string(),
                })
            }
            "palmier" => {
                // resolve_output already enforced overwrite=false on an existing path.
                let path = self.resolve_output(&request, "palmier")?;
                project_io::save_project_state_with_siblings(
                    &path,
                    &request.timeline,
                    &request.sibling_timelines,
                    &request.manifest,
                )
                .map_err(|e| e.to_string())?;
                Ok(ExportOutcome::Completed {
                    path: path.display().to_string(),
                })
            }
            "video" => {
                let ext = if request.codec == "ProRes" {
                    "mov"
                } else {
                    "mp4"
                };
                let path = self.resolve_output(&request, ext)?;
                let codec = match request.codec.as_str() {
                    "H.265" => crate::video_export::VideoCodec::H265,
                    "ProRes" => crate::video_export::VideoCodec::ProRes,
                    _ => crate::video_export::VideoCodec::H264,
                };
                let resolution = match request.resolution.as_str() {
                    "720p" => ExportResolution::R720p,
                    "1080p" => ExportResolution::R1080p,
                    "2K" => ExportResolution::R1440p,
                    "4K" => ExportResolution::R4K,
                    _ => ExportResolution::MatchTimeline,
                };
                let size = resolution.render_size(&request.timeline);
                let (w, h) = (size.width.max(2) as u32, size.height.max(2) as u32);
                let root = self.project_root.clone();
                let out = path.clone();
                std::thread::spawn(move || {
                    let progress = AtomicU64::new(0);
                    if let Err(reason) = crate::audio_export::export_project_with_audio(
                        &request.timeline,
                        &request.manifest,
                        &timelines,
                        &root,
                        &out,
                        w,
                        h,
                        codec,
                        0,
                        &progress,
                    ) {
                        eprintln!("agent export failed: {reason}");
                        let _ = std::fs::remove_file(&out);
                    }
                });
                Ok(ExportOutcome::Started {
                    path: path.display().to_string(),
                })
            }
            other => Err(format!("export_project: unknown mode '{other}'.")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_model::{MediaManifest, Timeline};

    fn request(mode: &str, output: Option<PathBuf>) -> ExportRequest {
        ExportRequest {
            mode: mode.into(),
            codec: "H.264".into(),
            resolution: "Match Timeline".into(),
            output_path: output.map(|p| p.display().to_string()),
            overwrite: true,
            fcpxml_target: "resolve".into(),
            timeline: Timeline::default(),
            sibling_timelines: Vec::new(),
            manifest: MediaManifest::default(),
        }
    }

    #[test]
    fn xml_export_writes_inline_and_appends_extension() {
        let dir = std::env::temp_dir().join("fronda-agent-export-host");
        let _ = std::fs::create_dir_all(&dir);
        let host = AgentExportHost::new(dir.clone());
        let out_no_ext = dir.join("agent-timeline");
        let _ = std::fs::remove_file(dir.join("agent-timeline.xml"));

        let outcome = host.export(request("xml", Some(out_no_ext))).unwrap();
        let ExportOutcome::Completed { path } = outcome else {
            panic!("xml completes inline");
        };
        assert!(path.ends_with(".xml"), "extension appended: {path}");
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("<xmeml"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn overwrite_false_refuses_existing() {
        let dir = std::env::temp_dir().join("fronda-agent-export-host");
        let _ = std::fs::create_dir_all(&dir);
        let host = AgentExportHost::new(dir.clone());
        let existing = dir.join("agent-existing.xml");
        std::fs::write(&existing, "x").unwrap();

        let mut req = request("xml", Some(existing.clone()));
        req.overwrite = false;
        let err = host.export(req).unwrap_err();
        assert!(err.contains("already exists"), "{err}");
        let _ = std::fs::remove_file(&existing);
    }

    #[test]
    fn palmier_export_writes_projectfile_package() {
        let dir = std::env::temp_dir().join("fronda-agent-export-host");
        let _ = std::fs::create_dir_all(&dir);
        let host = AgentExportHost::new(dir.clone());
        let out = dir.join("agent-pkg.palmier");
        let _ = std::fs::remove_dir_all(&out);

        let outcome = host.export(request("palmier", Some(out.clone()))).unwrap();
        assert!(matches!(outcome, ExportOutcome::Completed { .. }));
        assert!(out.join("project.json").is_file());
        let _ = std::fs::remove_dir_all(&out);
    }
}

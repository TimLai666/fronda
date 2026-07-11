//! Import a timeline from an FCP7 XMEML / FCPXML file into the shared project.
//!
//! Reuses the pure `render_core::xml_import` parser, relinks each referenced
//! file to the media library (match an existing entry by name, else register
//! the file path via the `import_media` tool so a missing file still shows
//! offline), remaps every clip's `media_ref` to the resolved manifest id, and
//! adopts the result as a new active timeline (the old one becomes a sibling —
//! import never overwrites current work).

use std::collections::HashMap;
use std::path::PathBuf;

use render_core::xml_import::{self, XmlImportFormat};

/// What an import produced, for a user-facing summary / logging.
pub struct ImportOutcome {
    pub timeline_id: String,
    pub timeline_name: String,
    pub clip_count: usize,
    /// Files relinked to already-imported library media (by filename).
    pub relinked: usize,
    /// Files newly registered into the manifest (path recorded; may be offline).
    pub registered: usize,
    /// Non-fatal parser notes (retimed clips, skipped nests/titles, …).
    pub notes: Vec<String>,
}

/// Read an XML/FCPXML file and import it into the shared executor as a new
/// active timeline. Detects the format from content (falling back to the file
/// extension). Returns a summary or an error string.
pub fn import_timeline_file_into_shared_state(path: &PathBuf) -> Result<ImportOutcome, String> {
    let content =
        std::fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let format = detect_format(&content, path)
        .ok_or_else(|| "unrecognized XML format (not XMEML or FCPXML)".to_string())?;

    let executor = crate::editor_state_hub::EditorStateHub::global().executor();
    let mut exec = executor
        .lock()
        .map_err(|_| "editor state is locked".to_string())?;
    import_timeline_from_xml(&mut exec, &content, format)
}

/// Detect the import format from the document content, falling back to the
/// file extension when the content sniff is inconclusive.
pub fn detect_format(content: &str, path: &std::path::Path) -> Option<XmlImportFormat> {
    XmlImportFormat::from_xml_content(content).or_else(|| {
        path.extension()
            .and_then(|e| e.to_str())
            .and_then(XmlImportFormat::from_extension)
    })
}

/// Parse XML content and adopt it as a new active timeline in `exec`, relinking
/// media by filename. Pure over an executor — unit-testable without gpui.
pub fn import_timeline_from_xml(
    exec: &mut agent_contract::ToolExecutor,
    content: &str,
    format: XmlImportFormat,
) -> Result<ImportOutcome, String> {
    let imported = xml_import::import_xml(content, format).map_err(|e| e.to_string())?;
    let mut timeline = imported.timeline;

    // Resolve each referenced file to a manifest id, relinking against the
    // existing library by name first, else registering the path. The parser
    // sets `clip.media_ref` to the file id (XMEML) OR the filename (FCPXML), so
    // the lookup is keyed by BOTH the file id and the name.
    let mut key_to_id: HashMap<String, String> = HashMap::new();
    let mut relinked = 0usize;
    let mut registered = 0usize;
    for file in &imported.files {
        if key_to_id.contains_key(&file.file_id) {
            continue;
        }
        let resolved = if let Some(existing) = exec
            .media_manifest()
            .entries
            .iter()
            .find(|e| e.name == file.name)
        {
            relinked += 1;
            existing.id.clone()
        } else {
            // Register the referenced path (records it even if the file is
            // missing → the clip shows offline rather than being dropped). The
            // new entry is appended last; read its id back from the manifest so
            // this doesn't depend on the tool's response envelope shape.
            let local = strip_file_url(&file.path);
            let before = exec.media_manifest().entries.len();
            match exec.execute(
                "import_media",
                &serde_json::json!({
                    "source": { "path": local },
                    "name": file.name,
                }),
            ) {
                Ok(_) if exec.media_manifest().entries.len() > before => {
                    registered += 1;
                    exec.media_manifest().entries.last().unwrap().id.clone()
                }
                Ok(_) => continue,
                Err(reason) => {
                    eprintln!("Import relink skipped {}: {reason}", file.name);
                    continue;
                }
            }
        };
        key_to_id.insert(file.file_id.clone(), resolved.clone());
        key_to_id.entry(file.name.clone()).or_insert(resolved);
    }

    // Remap every clip's media_ref to the resolved manifest id.
    let mut clip_count = 0usize;
    for track in &mut timeline.tracks {
        for clip in &mut track.clips {
            clip_count += 1;
            if let Some(id) = key_to_id.get(&clip.media_ref) {
                clip.media_ref = id.clone();
            }
        }
    }

    let timeline_name = timeline.name.clone();
    let timeline_id = exec.adopt_timeline(timeline);
    Ok(ImportOutcome {
        timeline_id,
        timeline_name,
        clip_count,
        relinked,
        registered,
        notes: imported.notes,
    })
}

/// Strip a `file://` / `file://localhost` URL prefix to a plain filesystem path.
/// Handles the Windows `file:///C:/…` form (leading slash before a drive).
fn strip_file_url(src: &str) -> String {
    let s = src
        .strip_prefix("file://localhost")
        .or_else(|| src.strip_prefix("file://"))
        .unwrap_or(src);
    // `file:///C:/x` → `/C:/x`; drop the leading slash before a drive letter.
    let bytes = s.as_bytes();
    if s.starts_with('/') && bytes.len() >= 3 && bytes[2] == b':' && bytes[1].is_ascii_alphabetic()
    {
        s[1..].to_string()
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_contract::ToolExecutor;
    use core_model::{
        Clip, ClipType, Crop, Interpolation, MediaManifest, MediaManifestEntry, MediaSource,
        Timeline, Track, Transform,
    };
    use render_core::xml_export::XmlExport;

    fn clip(id: &str, media: &str, kind: ClipType, start: i64, dur: i64) -> Clip {
        Clip {
            id: id.into(),
            media_ref: media.into(),
            media_type: kind,
            source_clip_type: kind,
            start_frame: start,
            duration_frames: dur,
            trim_start_frame: 0,
            trim_end_frame: dur,
            speed: 1.0,
            volume: 1.0,
            fade_in_frames: 0,
            fade_out_frames: 0,
            fade_in_interpolation: Interpolation::Linear,
            fade_out_interpolation: Interpolation::Linear,
            opacity: 1.0,
            transform: Transform::default(),
            crop: Crop::default(),
            link_group_id: None,
            caption_group_id: None,
            text_content: None,
            text_style: None,
            text_animation: None,
            word_timings: None,
            opacity_track: None,
            position_track: None,
            scale_track: None,
            rotation_track: None,
            crop_track: None,
            volume_track: None,
            effects: None,
            shape_style: None,
            stroke_progress_track: None,
            compound_timeline_id: None,
            blend_mode: core_model::BlendMode::Normal,
            chroma_key: None,
            multicam_group_id: None,
        }
    }

    fn entry(id: &str, name: &str, kind: ClipType) -> MediaManifestEntry {
        MediaManifestEntry {
            id: id.into(),
            name: name.into(),
            r#type: kind,
            source: MediaSource::External {
                absolute_path: format!("/lib/{name}"),
            },
            duration: 12.0,
            generation_input: None,
            source_width: Some(1920),
            source_height: Some(1080),
            source_fps: Some(30.0),
            has_audio: Some(kind == ClipType::Audio),
            folder_id: None,
            cached_remote_url: None,
            cached_remote_url_expires_at: None,
            source_timecode_frame: None,
            source_timecode_quanta: None,
            source_timecode_drop_frame: None,
            ai_tags: None,
            ai_description: None,
            ai_label_status: None,
            generation_status: None,
        }
    }

    /// Build an XMEML string whose asset filenames are `top.mp4` / `music.wav`.
    fn sample_xmeml() -> String {
        let mut tl = Timeline {
            name: "Imported".into(),
            fps: 30,
            ..Default::default()
        };
        tl.tracks = vec![
            Track {
                id: "v".into(),
                r#type: ClipType::Video,
                muted: false,
                hidden: false,
                sync_locked: false,
                display_height: 50.0,
                clips: vec![clip("cv", "top.mp4", ClipType::Video, 0, 90)],
            },
            Track {
                id: "a".into(),
                r#type: ClipType::Audio,
                muted: false,
                hidden: false,
                sync_locked: false,
                display_height: 50.0,
                clips: vec![clip("ca", "music.wav", ClipType::Audio, 0, 120)],
            },
        ];
        XmlExport::export(&tl)
    }

    #[test]
    fn import_adds_new_active_timeline_without_dropping_current() {
        let mut exec = ToolExecutor::new(Timeline::default(), MediaManifest::default());
        let rev_before = exec.revision();
        let out = import_timeline_from_xml(&mut exec, &sample_xmeml(), XmlImportFormat::Xmeml)
            .expect("import");
        // Old (empty) timeline preserved as a sibling; imported one is active.
        assert_eq!(exec.sibling_timelines().len(), 1);
        assert_eq!(exec.timeline().id, out.timeline_id);
        assert_eq!(out.timeline_name, "Imported");
        assert_eq!(out.clip_count, 2);
        assert!(exec.revision() > rev_before);
    }

    #[test]
    fn import_relinks_existing_media_and_registers_missing() {
        // Library already has top.mp4; music.wav is unknown and gets registered.
        let mut manifest = MediaManifest::default();
        manifest
            .entries
            .push(entry("existing-top-id", "top.mp4", ClipType::Video));
        let mut exec = ToolExecutor::new(Timeline::default(), manifest);

        let out = import_timeline_from_xml(&mut exec, &sample_xmeml(), XmlImportFormat::Xmeml)
            .expect("import");
        assert_eq!(out.relinked, 1, "top.mp4 relinked to existing entry");
        assert_eq!(out.registered, 1, "music.wav newly registered");

        // The imported video clip now references the EXISTING library id, so it
        // resolves against the media panel rather than duplicating the asset.
        let video_clip = &exec.timeline().tracks[0].clips[0];
        assert_eq!(video_clip.media_ref, "existing-top-id");
        assert!(exec
            .media_manifest()
            .entry_for(&video_clip.media_ref)
            .is_some());

        // The audio clip references a freshly-registered manifest entry.
        let audio_clip = &exec.timeline().tracks[1].clips[0];
        assert!(exec
            .media_manifest()
            .entry_for(&audio_clip.media_ref)
            .is_some());
    }

    #[test]
    fn strip_file_url_forms() {
        assert_eq!(strip_file_url("file:///media/a.mp4"), "/media/a.mp4");
        assert_eq!(
            strip_file_url("file://localhost/media/a.mp4"),
            "/media/a.mp4"
        );
        assert_eq!(strip_file_url("file:///C:/media/a.mp4"), "C:/media/a.mp4");
        assert_eq!(strip_file_url("/plain/path.mp4"), "/plain/path.mp4");
    }

    #[test]
    fn detect_format_prefers_content_then_extension() {
        let fcp = r#"<?xml version="1.0"?><fcpxml version="1.10"></fcpxml>"#;
        assert_eq!(
            detect_format(fcp, std::path::Path::new("x.xml")),
            Some(XmlImportFormat::Fcpxml)
        );
        let plain = "<sequence></sequence>";
        assert_eq!(
            detect_format(plain, std::path::Path::new("edit.fcpxml")),
            Some(XmlImportFormat::Fcpxml)
        );
    }
}

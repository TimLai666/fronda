//! XML timeline import model for professional NLE interchange (Issue #154).
//!
//! Supports parsing XMEML (FCP7/FCPX legacy), FCPXML, Premiere XML, and
//! DaVinci Resolve XML into the Fronda timeline model.

use serde::{Deserialize, Serialize};

/// The XML format to import from (Issue #154).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum XmlImportFormat {
    /// XMEML 4 / Final Cut Pro 7 XML (same format as our export).
    Xmeml,
    /// Final Cut Pro X XML (FCPXML 1.x).
    Fcpxml,
    /// Adobe Premiere Pro XML (via File → Export → Final Cut Pro XML).
    PremiereXml,
    /// DaVinci Resolve XML (via Timeline → Export → AAF/XML).
    DavinciXml,
}

impl XmlImportFormat {
    /// Infer the format from the file extension.
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.trim_start_matches('.').to_lowercase().as_str() {
            "xml" => Some(XmlImportFormat::Xmeml), // default for .xml
            "fcpxml" => Some(XmlImportFormat::Fcpxml),
            _ => None,
        }
    }

    /// Infer the format from XML content heuristics (root element / namespace).
    pub fn from_xml_content(content: &str) -> Option<Self> {
        if content.contains("<fcpxml") {
            Some(XmlImportFormat::Fcpxml)
        } else if content.contains("<xmeml") {
            Some(XmlImportFormat::Xmeml)
        } else if content.contains("PremiereData") || content.contains("Premiere") {
            Some(XmlImportFormat::PremiereXml)
        } else if content.contains("DaVinci") || content.contains("davinci") {
            Some(XmlImportFormat::DavinciXml)
        } else {
            None
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            XmlImportFormat::Xmeml => "XMEML (FCP7)",
            XmlImportFormat::Fcpxml => "FCPXML (FCP X)",
            XmlImportFormat::PremiereXml => "Premiere Pro XML",
            XmlImportFormat::DavinciXml => "DaVinci Resolve XML",
        }
    }
}

/// Request to import an XML timeline file (Issue #154).
#[derive(Debug, Clone, PartialEq)]
pub struct XmlImportRequest {
    /// Path to the XML file.
    pub path: String,
    /// Detected or user-specified format.
    pub format: XmlImportFormat,
    /// Whether to preserve the original project FPS (true) or adopt
    /// the imported timeline's FPS (false).
    pub preserve_project_fps: bool,
}

impl XmlImportRequest {
    /// Create an import request, inferring the format from the file extension.
    pub fn from_path(path: impl Into<String>) -> Self {
        let path = path.into();
        let ext = path.rsplit('.').next().unwrap_or("");
        let format = XmlImportFormat::from_extension(ext).unwrap_or(XmlImportFormat::Xmeml);
        Self {
            path,
            format,
            preserve_project_fps: false,
        }
    }
}

/// Error types for XML import (Issue #154).
#[derive(Debug, Clone, PartialEq)]
pub enum XmlImportError {
    /// The file could not be read.
    FileReadError { path: String, reason: String },
    /// The XML could not be parsed.
    ParseError { reason: String },
    /// The format was not recognized.
    UnknownFormat,
    /// The format is recognized but import is not yet implemented.
    NotImplemented { format: XmlImportFormat },
}

impl std::fmt::Display for XmlImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            XmlImportError::FileReadError { path, reason } => {
                write!(f, "Could not read '{path}': {reason}")
            }
            XmlImportError::ParseError { reason } => {
                write!(f, "XML parse error: {reason}")
            }
            XmlImportError::UnknownFormat => {
                write!(f, "Could not determine XML format from file content")
            }
            XmlImportError::NotImplemented { format } => {
                write!(f, "{} import is not yet implemented", format.display_name())
            }
        }
    }
}

/// Validate an XML import request without performing the actual import.
///
/// Returns `Ok(())` if the request is valid, or an error describing
/// why the import would fail.
pub fn validate_xml_import(request: &XmlImportRequest) -> Result<(), XmlImportError> {
    if request.path.is_empty() {
        return Err(XmlImportError::FileReadError {
            path: request.path.clone(),
            reason: "path must not be empty".into(),
        });
    }
    Ok(())
}

// ── XMEML parser (Issue #154) ────────────────────────────────────────────────
//
// A focused, dependency-free reader for the XMEML we emit in `xml_export`
// (and the plain FCP7-style exports that share its schema). It reverses the
// exporter: sequence name + rate, video/audio tracks (un-reversing the
// exporter's `.rev()` on video), and media clipitems' timing/speed/link/file.
// FCPXML / Premiere / Resolve keep their `NotImplemented` status — their
// namespaces, CDATA and entity handling warrant a real XML crate, a separate
// change.

use core_model::{Clip, ClipType, Crop, Interpolation, Timeline, Track, Transform};

/// A `<file>` reference discovered during import. The host relinks these to
/// real assets by filename/path (like every NLE's "relink media" step).
#[derive(Debug, Clone, PartialEq)]
pub struct ReferencedFile {
    /// The `<file id="…">` attribute — clips referencing the same id share media.
    pub file_id: String,
    /// The `<name>` (display filename).
    pub name: String,
    /// The `<pathurl>` (may be a `file://` URL or a bare path).
    pub path: String,
}

/// The result of a successful XML import: the reconstructed timeline plus the
/// external files it references (for host relink).
#[derive(Debug, Clone, PartialEq)]
pub struct ImportedTimeline {
    pub timeline: Timeline,
    pub files: Vec<ReferencedFile>,
    /// Non-fatal notes (e.g. skipped nest carriers, dropped text overlays).
    pub notes: Vec<String>,
}

/// Reverse of `xml_export::xml_escape`.
fn xml_unescape(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        // `&amp;` last so "&amp;lt;" round-trips to "&lt;", not "<".
        .replace("&amp;", "&")
}

/// True when the char can legally follow a tag name (so `<track` doesn't match
/// inside `<trackindex>`).
fn is_tag_boundary(c: Option<char>) -> bool {
    matches!(c, Some('>') | Some('/') | Some(' ') | Some('\t') | Some('\n') | Some('\r'))
}

/// Every top-level `<tag …>…</tag>` block in `hay`, depth-aware for nested
/// same-name tags. Returns `(opening_tag_full, inner_content)` pairs;
/// self-closing `<tag/>` yields an empty inner slice.
fn xml_blocks<'a>(hay: &'a str, tag: &str) -> Vec<(&'a str, &'a str)> {
    let open_lt = format!("<{tag}");
    let close = format!("</{tag}>");
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(rel) = hay[i..].find(&open_lt) {
        let start = i + rel;
        let name_end = start + open_lt.len();
        if !is_tag_boundary(hay[name_end..].chars().next()) {
            i = name_end;
            continue;
        }
        let Some(g) = hay[start..].find('>') else { break };
        let gt = start + g;
        let open_full = &hay[start..=gt];
        if open_full.ends_with("/>") {
            out.push((open_full, ""));
            i = gt + 1;
            continue;
        }
        let inner_start = gt + 1;
        let mut depth = 1usize;
        let mut j = inner_start;
        loop {
            let next_open = hay[j..].find(&open_lt).map(|r| j + r);
            let next_close = hay[j..].find(&close).map(|r| j + r);
            match (next_open, next_close) {
                (Some(o), Some(c)) if o < c => {
                    let ne = o + open_lt.len();
                    if is_tag_boundary(hay[ne..].chars().next()) {
                        // A self-closing nested open doesn't raise depth.
                        let selfclose = hay[o..]
                            .find('>')
                            .map(|g2| hay[o..=o + g2].ends_with("/>"))
                            .unwrap_or(false);
                        if !selfclose {
                            depth += 1;
                        }
                    }
                    j = ne;
                }
                (_, Some(c)) => {
                    depth -= 1;
                    if depth == 0 {
                        out.push((open_full, &hay[inner_start..c]));
                        i = c + close.len();
                        break;
                    }
                    j = c + close.len();
                }
                _ => return out, // malformed: stop cleanly
            }
        }
    }
    out
}

/// The first `<tag>…</tag>` block's inner content, or None.
fn first_inner<'a>(hay: &'a str, tag: &str) -> Option<&'a str> {
    xml_blocks(hay, tag).into_iter().next().map(|(_, inner)| inner)
}

/// The first `<tag>text</tag>`'s unescaped, trimmed text.
fn first_text(hay: &str, tag: &str) -> Option<String> {
    first_inner(hay, tag).map(|inner| xml_unescape(inner.trim()))
}

fn first_i64(hay: &str, tag: &str) -> Option<i64> {
    first_text(hay, tag).and_then(|s| s.trim().parse().ok())
}

/// Value of `attr="…"` in an opening tag.
fn attr(open_tag: &str, name: &str) -> Option<String> {
    let key = format!("{name}=\"");
    let start = open_tag.find(&key)? + key.len();
    let end = open_tag[start..].find('"')? + start;
    Some(xml_unescape(&open_tag[start..end]))
}

/// Parse XMEML content into a timeline + referenced files.
pub fn parse_xmeml(content: &str) -> Result<ImportedTimeline, XmlImportError> {
    let (_, seq) = xml_blocks(content, "sequence")
        .into_iter()
        .next()
        .ok_or_else(|| XmlImportError::ParseError {
            reason: "no <sequence> element".into(),
        })?;

    let name = first_text(seq, "name").unwrap_or_else(|| "Imported Timeline".into());
    // The sequence <rate><timebase> is the project fps.
    let fps = first_inner(seq, "rate")
        .and_then(|r| first_i64(r, "timebase"))
        .filter(|f| *f > 0)
        .unwrap_or(30);

    let media = first_inner(seq, "media").ok_or_else(|| XmlImportError::ParseError {
        reason: "sequence has no <media>".into(),
    })?;

    let mut timeline = Timeline {
        name,
        fps,
        ..Default::default()
    };
    timeline.tracks.clear();
    let mut files: Vec<ReferencedFile> = Vec::new();
    let mut notes: Vec<String> = Vec::new();

    // Video first: the exporter emits video tracks reversed (`.rev()`), so the
    // parsed order is bottom→top; collect then reverse to restore tracks[0]=top.
    let mut idx = 0usize;
    if let Some(video) = first_inner(media, "video") {
        let mut vtracks: Vec<Track> = Vec::new();
        for (_, track_inner) in xml_blocks(video, "track") {
            vtracks.push(parse_track(
                track_inner,
                ClipType::Video,
                idx,
                &mut files,
                &mut notes,
            ));
            idx += 1;
        }
        vtracks.reverse();
        timeline.tracks.extend(vtracks);
    }
    if let Some(audio) = first_inner(media, "audio") {
        for (_, track_inner) in xml_blocks(audio, "track") {
            timeline.tracks.push(parse_track(
                track_inner,
                ClipType::Audio,
                idx,
                &mut files,
                &mut notes,
            ));
            idx += 1;
        }
    }

    Ok(ImportedTimeline {
        timeline,
        files,
        notes,
    })
}

fn parse_track(
    track_inner: &str,
    kind: ClipType,
    index: usize,
    files: &mut Vec<ReferencedFile>,
    notes: &mut Vec<String>,
) -> Track {
    let enabled = first_text(track_inner, "enabled")
        .map(|v| !v.eq_ignore_ascii_case("false"))
        .unwrap_or(true);
    let locked = first_text(track_inner, "locked")
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    let mut track = Track {
        id: format!("import-track-{}-{index}", kind.name()),
        r#type: kind,
        muted: kind == ClipType::Audio && !enabled,
        hidden: !enabled,
        sync_locked: locked,
        display_height: 50.0,
        clips: Vec::new(),
    };

    for (open, clip_inner) in xml_blocks(track_inner, "clipitem") {
        // Nest carriers (clipitem wrapping a <sequence>) aren't reconstructed
        // in v1 — record and skip rather than mis-import.
        if xml_blocks(clip_inner, "sequence").into_iter().next().is_some() {
            notes.push(format!(
                "Skipped nested-sequence clip '{}' (nest import not yet supported)",
                first_text(clip_inner, "name").unwrap_or_default()
            ));
            continue;
        }
        if let Some(clip) = parse_clipitem(open, clip_inner, kind, files) {
            track.clips.push(clip);
        }
    }
    track
}

fn parse_clipitem(
    open: &str,
    inner: &str,
    kind: ClipType,
    files: &mut Vec<ReferencedFile>,
) -> Option<Clip> {
    let id = attr(open, "id").unwrap_or_default();
    let start = first_i64(inner, "start").unwrap_or(0).max(0);
    let trim_start = first_i64(inner, "in").unwrap_or(0).max(0);
    let trim_end = first_i64(inner, "out").unwrap_or(0).max(0);
    // Prefer the explicit <duration>; fall back to out-in.
    let duration = first_i64(inner, "duration")
        .filter(|d| *d > 0)
        .unwrap_or_else(|| (trim_end - trim_start).max(1));

    let speed = first_inner(inner, "speed")
        .and_then(|s| first_text(s, "value"))
        .and_then(|v| v.parse::<f64>().ok())
        .filter(|s| *s > 0.0)
        .unwrap_or(1.0);

    let link_group_id = first_inner(inner, "link").and_then(|l| first_text(l, "linkclipref"));

    // <file id="…"> may be self-closing (a dedup back-reference) or full.
    let (file_id, media_ref) = match xml_blocks(inner, "file").into_iter().next() {
        Some((file_open, file_inner)) => {
            let fid = attr(file_open, "id").unwrap_or_default();
            if !file_inner.trim().is_empty() {
                let fname = first_text(file_inner, "name").unwrap_or_else(|| fid.clone());
                let path = first_text(file_inner, "pathurl").unwrap_or_default();
                if !files.iter().any(|f| f.file_id == fid) {
                    files.push(ReferencedFile {
                        file_id: fid.clone(),
                        name: fname,
                        path,
                    });
                }
            }
            (fid.clone(), fid)
        }
        None => (String::new(), String::new()),
    };
    let _ = file_id;
    // Our export always writes clipitem ids; synthesize a deterministic one
    // only for malformed third-party input.
    let clip_id = if id.is_empty() {
        format!("import-{}-{}-{}", kind.name(), start, trim_start)
    } else {
        id
    };

    Some(Clip {
        id: clip_id,
        media_ref,
        media_type: kind,
        source_clip_type: kind,
        start_frame: start,
        duration_frames: duration,
        trim_start_frame: trim_start,
        trim_end_frame: trim_end,
        speed,
        volume: 1.0,
        fade_in_frames: 0,
        fade_out_frames: 0,
        fade_in_interpolation: Interpolation::Linear,
        fade_out_interpolation: Interpolation::Linear,
        opacity: 1.0,
        transform: Transform::default(),
        crop: Crop::default(),
        link_group_id,
        caption_group_id: None,
        text_content: None,
        text_style: None,
        text_animation: None,
        word_timings: None,
        // Keyframe/annotation tracks aren't reconstructed in v1 (the exporter's
        // XML-012/013 emission is partial); timing + structure + files are.
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
    })
}

/// Import an XML file's content per its detected format. Only XMEML parses;
/// the others honestly report `NotImplemented`.
pub fn import_xml(
    content: &str,
    format: XmlImportFormat,
) -> Result<ImportedTimeline, XmlImportError> {
    match format {
        XmlImportFormat::Xmeml => parse_xmeml(content),
        other => Err(XmlImportError::NotImplemented { format: other }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xml_import_format_from_extension() {
        assert_eq!(
            XmlImportFormat::from_extension("xml"),
            Some(XmlImportFormat::Xmeml)
        );
        assert_eq!(
            XmlImportFormat::from_extension("fcpxml"),
            Some(XmlImportFormat::Fcpxml)
        );
        assert_eq!(XmlImportFormat::from_extension("pdf"), None);
    }

    #[test]
    fn xml_import_format_from_content_fcpxml() {
        let content = r#"<?xml version="1.0"?><fcpxml version="1.10">"#;
        assert_eq!(
            XmlImportFormat::from_xml_content(content),
            Some(XmlImportFormat::Fcpxml)
        );
    }

    #[test]
    fn xml_import_format_from_content_xmeml() {
        let content = r#"<?xml version="1.0"?><xmeml version="4">"#;
        assert_eq!(
            XmlImportFormat::from_xml_content(content),
            Some(XmlImportFormat::Xmeml)
        );
    }

    #[test]
    fn xml_import_format_from_content_unknown() {
        assert_eq!(XmlImportFormat::from_xml_content("<html>"), None);
    }

    #[test]
    fn xml_import_request_infers_format() {
        let req = XmlImportRequest::from_path("/project.fcpxml");
        assert_eq!(req.format, XmlImportFormat::Fcpxml);
        assert!(!req.preserve_project_fps);
    }

    #[test]
    fn xml_import_request_xml_extension_defaults_xmeml() {
        let req = XmlImportRequest::from_path("/export.xml");
        assert_eq!(req.format, XmlImportFormat::Xmeml);
    }

    #[test]
    fn validate_xml_import_empty_path() {
        let req = XmlImportRequest {
            path: String::new(),
            format: XmlImportFormat::Xmeml,
            preserve_project_fps: false,
        };
        let err = validate_xml_import(&req).unwrap_err();
        assert!(err.to_string().contains("path must not be empty"));
    }

    #[test]
    fn validate_xml_import_valid_path() {
        let req = XmlImportRequest::from_path("/some/file.xml");
        assert!(validate_xml_import(&req).is_ok());
    }

    #[test]
    fn xml_import_error_display() {
        let err = XmlImportError::NotImplemented {
            format: XmlImportFormat::Fcpxml,
        };
        assert!(err.to_string().contains("FCPXML"));
        assert!(err.to_string().contains("not yet implemented"));
    }

    #[test]
    fn xml_import_format_display_names_non_empty() {
        for fmt in [
            XmlImportFormat::Xmeml,
            XmlImportFormat::Fcpxml,
            XmlImportFormat::PremiereXml,
            XmlImportFormat::DavinciXml,
        ] {
            assert!(
                !fmt.display_name().is_empty(),
                "{fmt:?} has no display name"
            );
        }
    }

    // ── Issue #154: XML import model ─────────────────────────────────────────

    #[test]
    fn issue_154_xmeml_detected_from_extension() {
        assert!(matches!(
            XmlImportFormat::from_extension("xml"),
            Some(XmlImportFormat::Xmeml)
        ));
    }

    #[test]
    fn issue_154_fcpxml_detected_from_extension() {
        assert!(matches!(
            XmlImportFormat::from_extension("fcpxml"),
            Some(XmlImportFormat::Fcpxml)
        ));
    }

    #[test]
    fn issue_154_unknown_extension_returns_none() {
        assert!(XmlImportFormat::from_extension("mov").is_none());
    }

    #[test]
    fn issue_154_import_request_from_path_defaults() {
        let req = XmlImportRequest::from_path("/proj/edit.xml");
        assert_eq!(req.path, "/proj/edit.xml");
        // default: don't override project fps from the imported file
        assert!(!req.preserve_project_fps);
    }

    #[test]
    fn issue_154_validate_empty_path_is_error() {
        let req = XmlImportRequest {
            path: String::new(),
            ..XmlImportRequest::from_path("x.xml")
        };
        assert!(validate_xml_import(&req).is_err());
    }

    #[test]
    fn issue_154_premiere_xml_detected_from_content() {
        let content = r#"<?xml version="1.0"?><PremiereData Version="3"></PremiereData>"#;
        assert!(matches!(
            XmlImportFormat::from_xml_content(content),
            Some(XmlImportFormat::PremiereXml)
        ));
    }

    // ── XMEML parser (Issue #154) ────────────────────────────────────────────

    use crate::xml_export::XmlExport;
    use core_model::{Clip, ClipType, Crop, Interpolation, Timeline, Track, Transform};

    fn test_clip(id: &str, media: &str, kind: ClipType, start: i64, dur: i64) -> Clip {
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

    fn track(id: &str, kind: ClipType, clips: Vec<Clip>) -> Track {
        Track {
            id: id.into(),
            r#type: kind,
            muted: false,
            hidden: false,
            sync_locked: false,
            display_height: 50.0,
            clips,
        }
    }

    #[test]
    fn scanner_is_same_name_depth_aware() {
        // The key guarantee: a nested same-name tag doesn't split the outer
        // block into two siblings (so nested <sequence>/<track> won't confuse
        // the track/clip iteration).
        let xml = "<track>A<track>NESTED</track>B</track><track>C</track>";
        let blocks = xml_blocks(xml, "track");
        assert_eq!(blocks.len(), 2, "two top-level tracks, nested one absorbed");
        assert_eq!(blocks[0].1, "A<track>NESTED</track>B");
        assert_eq!(blocks[1].1, "C");
        // Recursing into the first re-finds the nested one.
        assert_eq!(xml_blocks(blocks[0].1, "track").len(), 1);
    }

    #[test]
    fn scanner_unescapes_and_reads_attrs() {
        let xml = r#"<file id="v-1"><name>A &amp; B &lt;x&gt;</name></file>"#;
        let (open, inner) = xml_blocks(xml, "file").into_iter().next().unwrap();
        assert_eq!(attr(open, "id").as_deref(), Some("v-1"));
        assert_eq!(first_text(inner, "name").as_deref(), Some("A & B <x>"));
    }

    #[test]
    fn scanner_handles_self_closing() {
        let xml = r#"<clipitem><file id="v-1"/></clipitem>"#;
        let (open, inner) = xml_blocks(xml, "file").into_iter().next().unwrap();
        assert_eq!(attr(open, "id").as_deref(), Some("v-1"));
        assert!(inner.is_empty());
    }

    #[test]
    fn roundtrip_two_tracks_timing_and_files() {
        // A V1 (with two clips) + A1 timeline exported then re-imported must
        // preserve fps, track structure/type, and each clip's placement/trims.
        let mut tl = Timeline {
            name: "RT".into(),
            fps: 24,
            ..Default::default()
        };
        tl.tracks = vec![
            track(
                "v1",
                ClipType::Video,
                vec![
                    {
                        let mut c = test_clip("cv1", "beach.mp4", ClipType::Video, 0, 100);
                        c.trim_start_frame = 10;
                        c.trim_end_frame = 110;
                        c
                    },
                    test_clip("cv2", "city.mp4", ClipType::Video, 120, 60),
                ],
            ),
            track(
                "a1",
                ClipType::Audio,
                vec![test_clip("ca1", "music.wav", ClipType::Audio, 0, 200)],
            ),
        ];
        // No manifest: pathurl falls back to media_ref, which still lets us
        // assert structure, timing, and file-reference collection.
        let xml = XmlExport::export(&tl);
        let imported = parse_xmeml(&xml).expect("parse");

        assert_eq!(imported.timeline.fps, 24);
        assert_eq!(imported.timeline.tracks.len(), 2);
        assert_eq!(imported.timeline.tracks[0].r#type, ClipType::Video);
        assert_eq!(imported.timeline.tracks[1].r#type, ClipType::Audio);

        let v = &imported.timeline.tracks[0].clips;
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].start_frame, 0);
        assert_eq!(v[0].trim_start_frame, 10);
        assert_eq!(v[0].trim_end_frame, 110);
        assert_eq!(v[0].duration_frames, 100);
        assert_eq!(v[1].start_frame, 120);
        assert_eq!(v[1].duration_frames, 60);

        assert_eq!(imported.timeline.tracks[1].clips.len(), 1);
        assert_eq!(imported.timeline.tracks[1].clips[0].duration_frames, 200);

        // File references collected for host relink; pathurl round-trips.
        assert!(imported.files.iter().any(|f| f.path.contains("beach.mp4")));
        assert!(imported.files.iter().any(|f| f.path.contains("music.wav")));
    }

    #[test]
    fn roundtrip_preserves_video_track_order() {
        // Exporter reverses video tracks; the parser must un-reverse so
        // tracks[0] stays the top layer.
        let mut tl = Timeline {
            fps: 30,
            ..Default::default()
        };
        tl.tracks = vec![
            track(
                "top",
                ClipType::Video,
                vec![test_clip("t", "top.mp4", ClipType::Video, 0, 30)],
            ),
            track(
                "bottom",
                ClipType::Video,
                vec![test_clip("b", "bot.mp4", ClipType::Video, 0, 30)],
            ),
        ];
        let xml = XmlExport::export(&tl);
        let imported = parse_xmeml(&xml).expect("parse");
        assert_eq!(imported.timeline.tracks.len(), 2);
        // tracks[0]'s clip references the top media (order preserved).
        assert_eq!(imported.timeline.tracks[0].clips[0].media_ref, "top.mp4-v");
    }

    #[test]
    fn roundtrip_speed_and_link() {
        let mut tl = Timeline {
            fps: 30,
            ..Default::default()
        };
        let mut c = test_clip("cv", "clip.mp4", ClipType::Video, 0, 50);
        c.speed = 2.0;
        c.link_group_id = Some("lg-1".into());
        tl.tracks = vec![track("v1", ClipType::Video, vec![c])];
        let xml = XmlExport::export(&tl);
        let imported = parse_xmeml(&xml).expect("parse");
        let clip = &imported.timeline.tracks[0].clips[0];
        assert!((clip.speed - 2.0).abs() < 1e-6, "speed {}", clip.speed);
        assert_eq!(clip.link_group_id.as_deref(), Some("lg-1"));
    }

    #[test]
    fn import_xml_dispatches_only_xmeml() {
        assert!(matches!(
            import_xml("<html>", XmlImportFormat::Fcpxml),
            Err(XmlImportError::NotImplemented { .. })
        ));
        let err = parse_xmeml("<nope/>").unwrap_err();
        assert!(matches!(err, XmlImportError::ParseError { .. }));
    }
}

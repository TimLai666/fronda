//! Read-only tool output formatting (READ-001 to READ-011).
//!
//! These functions format project state into the JSON structures expected by
//! the agent/MCP tool surface. They are pure data transformations that
//! operate on core_model types — no platform I/O or rendering.

use core_model::{Clip, ClipType, MediaManifest, MediaSource, Timeline, Track};
use serde::Serialize;
use serde_json::{json, Value};

// ---------------------------------------------------------------------------
// get_timeline output (READ-001 to READ-009)
// ---------------------------------------------------------------------------

/// Options for formatting timeline output.
pub struct TimelineFormatOptions {
    /// Optional window range [start_frame, end_frame).
    pub window: Option<(i64, i64)>,
    /// Whether to include track- and clip-level defaults in output.
    pub omit_defaults: bool,
    /// Decimal places for numeric values.
    pub decimal_places: u32,
}

impl Default for TimelineFormatOptions {
    fn default() -> Self {
        Self {
            window: None,
            omit_defaults: true,
            decimal_places: 3,
        }
    }
}

/// Formatted timeline result.
#[derive(Debug, Clone, Serialize)]
pub struct FormattedTimeline {
    pub fps: i64,
    pub width: i64,
    pub height: i64,
    pub total_frames: i64,
    pub current_frame: i64,
    pub can_generate: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tracks: Vec<FormattedTrack>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FormattedTrack {
    pub id: String,
    #[serde(rename = "type")]
    pub r#type: String,
    pub index: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_clips: Option<usize>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub clips: Vec<FormattedClip>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FormattedClip {
    pub id: String,
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub fields: Option<Value>,
}

// ---------------------------------------------------------------------------
// get_media output (READ-010 to READ-011)
// ---------------------------------------------------------------------------

/// Formatted media manifest result.
#[derive(Debug, Clone, Serialize)]
pub struct FormattedMediaManifest {
    pub entries: Vec<FormattedMediaEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folders: Option<Vec<FormattedMediaFolder>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FormattedMediaEntry {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub r#type: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_seconds: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folder_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FormattedMediaFolder {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_folder_id: Option<String>,
}

// ---------------------------------------------------------------------------
// Implementation
// ---------------------------------------------------------------------------

/// Format the timeline for get_timeline output.
///
/// READ-001: fps, resolution, total_frames, track list, current_frame.
/// READ-002: includes canGenerate.
/// READ-003: omits default-valued fields when omit_defaults is true.
/// READ-004: rounds numeric values to decimal_places.
/// READ-005: supports optional [startFrame, endFrame) windowing.
/// READ-006: when windowing hides clips, reports totalClips on track.
pub fn format_timeline(timeline: &Timeline, options: &TimelineFormatOptions) -> FormattedTimeline {
    let total_frames = compute_total_frames(timeline);
    let can_generate = timeline.fps > 0 && timeline.width > 0 && timeline.height > 0;

    let tracks: Vec<FormattedTrack> = timeline
        .tracks
        .iter()
        .enumerate()
        .map(|(i, track)| format_track(track, i, options))
        .collect();

    FormattedTimeline {
        fps: timeline.fps,
        width: timeline.width,
        height: timeline.height,
        total_frames,
        current_frame: 0,
        can_generate,
        tracks,
    }
}

fn compute_total_frames(timeline: &Timeline) -> i64 {
    timeline
        .tracks
        .iter()
        .flat_map(|t| t.clips.iter())
        .map(|c| c.start_frame + c.duration_frames)
        .max()
        .unwrap_or(0)
}

fn format_track(track: &Track, index: usize, options: &TimelineFormatOptions) -> FormattedTrack {
    let r#type = match track.r#type {
        ClipType::Video => "video",
        ClipType::Audio => "audio",
        ClipType::Text => "text",
        ClipType::Image => "image",
        ClipType::Lottie => "lottie",
    };

    let (visible_clips, total_count) = match options.window {
        Some((start, end)) => {
            let visible: Vec<&Clip> = track
                .clips
                .iter()
                .filter(|c| {
                    let c_end = c.start_frame + c.duration_frames;
                    c.start_frame < end && c_end > start
                })
                .collect();
            let formatted: Vec<FormattedClip> =
                visible.iter().map(|c| format_clip(c, options)).collect();
            (formatted, track.clips.len())
        }
        None => {
            let formatted: Vec<FormattedClip> = track
                .clips
                .iter()
                .map(|c| format_clip(c, options))
                .collect();
            (formatted, 0)
        }
    };

    let mut formatted = FormattedTrack {
        id: track.id.clone(),
        r#type: r#type.to_string(),
        index,
        total_clips: None,
        clips: visible_clips,
    };

    // READ-006: report totalClips when windowing hides clips
    if options.window.is_some() && total_count > formatted.clips.len() {
        formatted.total_clips = Some(total_count);
    }

    formatted
}

fn format_clip(clip: &Clip, options: &TimelineFormatOptions) -> FormattedClip {
    if options.omit_defaults {
        let mut fields = serde_json::Map::new();
        fields.insert("startFrame".to_string(), json!(clip.start_frame));
        fields.insert("durationFrames".to_string(), json!(clip.duration_frames));

        // Only include fields that differ from defaults
        if clip.media_ref != "placeholder" && !clip.media_ref.is_empty() {
            fields.insert("mediaRef".to_string(), json!(clip.media_ref));
        }
        if clip.speed != 1.0 {
            fields.insert(
                "speed".to_string(),
                round_json(json!(clip.speed), options.decimal_places),
            );
        }
        if clip.volume != 1.0 {
            fields.insert(
                "volume".to_string(),
                round_json(json!(clip.volume), options.decimal_places),
            );
        }
        if clip.opacity != 1.0 {
            fields.insert(
                "opacity".to_string(),
                round_json(json!(clip.opacity), options.decimal_places),
            );
        }
        if clip.trim_start_frame != 0 {
            fields.insert("trimStartFrame".to_string(), json!(clip.trim_start_frame));
        }
        if clip.trim_end_frame != 0 {
            fields.insert("trimEndFrame".to_string(), json!(clip.trim_end_frame));
        }
        if clip.fade_in_frames != 0 {
            fields.insert("fadeInFrames".to_string(), json!(clip.fade_in_frames));
        }
        if clip.fade_out_frames != 0 {
            fields.insert("fadeOutFrames".to_string(), json!(clip.fade_out_frames));
        }
        if clip.link_group_id.is_some() {
            fields.insert("linkGroupId".to_string(), json!(clip.link_group_id));
        }
        if clip.caption_group_id.is_some() {
            fields.insert("captionGroupId".to_string(), json!(clip.caption_group_id));
        }
        if let Some(ref text) = clip.text_content {
            fields.insert("textContent".to_string(), json!(text));
        }

        FormattedClip {
            id: clip.id.clone(),
            fields: Some(Value::Object(fields)),
        }
    } else {
        // Return all fields as a flat map
        let mut fields = serde_json::Map::new();
        fields.insert("startFrame".to_string(), json!(clip.start_frame));
        fields.insert("durationFrames".to_string(), json!(clip.duration_frames));
        fields.insert("mediaRef".to_string(), json!(clip.media_ref));
        fields.insert(
            "speed".to_string(),
            round_json(json!(clip.speed), options.decimal_places),
        );
        fields.insert(
            "volume".to_string(),
            round_json(json!(clip.volume), options.decimal_places),
        );
        fields.insert(
            "opacity".to_string(),
            round_json(json!(clip.opacity), options.decimal_places),
        );

        FormattedClip {
            id: clip.id.clone(),
            fields: Some(Value::Object(fields)),
        }
    }
}

/// Round a numeric JSON value to the given decimal places.
fn round_json(value: Value, places: u32) -> Value {
    if let Value::Number(n) = &value {
        if let Some(f) = n.as_f64() {
            let factor = 10u64.pow(places) as f64;
            let rounded = (f * factor).round() / factor;
            return json!(rounded);
        }
    }
    value
}

/// Format the media manifest for get_media output.
///
/// READ-010: returns media manifest/library data as JSON text.
/// READ-011: rounds numeric values to decimal_places.
pub fn format_media_manifest(
    manifest: &MediaManifest,
    decimal_places: u32,
) -> FormattedMediaManifest {
    let entries: Vec<FormattedMediaEntry> = manifest
        .entries
        .iter()
        .map(|e| {
            let r#type = match e.source {
                MediaSource::External { .. } => "external",
                MediaSource::Project { .. } => "project",
            };

            FormattedMediaEntry {
                id: e.id.clone(),
                name: e.name.clone(),
                r#type: r#type.to_string(),
                path: String::new(),
                duration_seconds: Some(round_f64(e.duration, decimal_places)),
                folder_id: e.folder_id.clone(),
            }
        })
        .collect();

    let folders: Option<Vec<FormattedMediaFolder>> = if manifest.folders.is_empty() {
        None
    } else {
        Some(
            manifest
                .folders
                .iter()
                .map(|f| FormattedMediaFolder {
                    id: f.id.clone(),
                    name: f.name.clone(),
                    parent_folder_id: f.parent_folder_id.clone(),
                })
                .collect(),
        )
    };

    FormattedMediaManifest { entries, folders }
}

fn round_f64(value: f64, places: u32) -> f64 {
    let factor = 10u64.pow(places) as f64;
    (value * factor).round() / factor
}

// ---------------------------------------------------------------------------
// Utility: get_timeline as JSON Value
// ---------------------------------------------------------------------------

/// Format timeline as a raw [`Value`] suitable for serializing into the
/// tool response.
pub fn format_timeline_json(timeline: &Timeline) -> Value {
    let formatted = format_timeline(timeline, &TimelineFormatOptions::default());
    serde_json::to_value(formatted).unwrap_or_default()
}

/// Format media manifest as a raw [`Value`] suitable for serializing into
/// the tool response.
pub fn format_media_json(manifest: &MediaManifest) -> Value {
    let formatted = format_media_manifest(manifest, 3);
    serde_json::to_value(formatted).unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use core_model::{Clip, ClipType, MediaManifest, MediaManifestEntry, MediaSource, Track};

    fn sample_timeline() -> Timeline {
        Timeline {
            fps: 30,
            width: 1920,
            height: 1080,
            settings_configured: true,
            selected_clip_ids: std::collections::HashSet::new(),
            transcription_language: None,
            tracks: vec![
                Track {
                    id: "track-v".into(),
                    r#type: ClipType::Video,
                    muted: false,
                    hidden: false,
                    sync_locked: false,
                    clips: vec![
                        Clip {
                            id: "clip-001".into(),
                            media_ref: "asset-vid-1".into(),
                            media_type: ClipType::Video,
                            source_clip_type: ClipType::Video,
                            start_frame: 0,
                            duration_frames: 100,
                            trim_start_frame: 10,
                            trim_end_frame: 20,
                            speed: 1.0,
                            volume: 1.0,
                            opacity: 1.0,
                            fade_in_frames: 0,
                            fade_out_frames: 0,
                            fade_in_interpolation: core_model::Interpolation::Linear,
                            fade_out_interpolation: core_model::Interpolation::Linear,
                            transform: core_model::Transform::default(),
                            crop: core_model::Crop::default(),
                            link_group_id: Some("lg-1".into()),
                            caption_group_id: None,
                            text_content: None,
                            text_style: None,
                            opacity_track: None,
                            position_track: None,
                            scale_track: None,
                            rotation_track: None,
                            crop_track: None,
                            volume_track: None,
                            effects: None,
                        },
                        Clip {
                            id: "clip-002".into(),
                            media_ref: "asset-vid-2".into(),
                            media_type: ClipType::Video,
                            source_clip_type: ClipType::Video,
                            start_frame: 100,
                            duration_frames: 50,
                            trim_start_frame: 0,
                            trim_end_frame: 0,
                            speed: 1.5,
                            volume: 0.8,
                            opacity: 1.0,
                            fade_in_frames: 5,
                            fade_out_frames: 10,
                            fade_in_interpolation: core_model::Interpolation::Linear,
                            fade_out_interpolation: core_model::Interpolation::Linear,
                            transform: core_model::Transform::default(),
                            crop: core_model::Crop::default(),
                            link_group_id: None,
                            caption_group_id: None,
                            text_content: None,
                            text_style: None,
                            opacity_track: None,
                            position_track: None,
                            scale_track: None,
                            rotation_track: None,
                            crop_track: None,
                            volume_track: None,
                            effects: None,
                        },
                    ],
                },
                Track {
                    id: "track-a".into(),
                    r#type: ClipType::Audio,
                    muted: false,
                    hidden: false,
                    sync_locked: false,
                    clips: vec![Clip {
                        id: "clip-003".into(),
                        media_ref: "asset-audio-1".into(),
                        media_type: ClipType::Audio,
                        source_clip_type: ClipType::Audio,
                        start_frame: 0,
                        duration_frames: 200,
                        trim_start_frame: 0,
                        trim_end_frame: 0,
                        speed: 1.0,
                        volume: 1.0,
                        opacity: 1.0,
                        fade_in_frames: 0,
                        fade_out_frames: 0,
                        fade_in_interpolation: core_model::Interpolation::Linear,
                        fade_out_interpolation: core_model::Interpolation::Linear,
                        transform: core_model::Transform::default(),
                        crop: core_model::Crop::default(),
                        link_group_id: None,
                        caption_group_id: None,
                        text_content: None,
                        text_style: None,
                        opacity_track: None,
                        position_track: None,
                        scale_track: None,
                        rotation_track: None,
                        crop_track: None,
                        volume_track: None,
                        effects: None,
                    }],
                },
            ],
        }
    }

    #[test]
    fn read_001_fps_resolution_tracks_total_frames() {
        let tl = sample_timeline();
        let result = format_timeline(&tl, &TimelineFormatOptions::default());
        assert_eq!(result.fps, 30, "READ-001: fps");
        assert_eq!(result.width, 1920, "READ-001: width");
        assert_eq!(result.height, 1080, "READ-001: height");
        // clip-003 (audio) ends at 200 — max across all tracks
        assert_eq!(result.total_frames, 200, "READ-001: total_frames");
        assert_eq!(result.tracks.len(), 2, "READ-001: 2 tracks");
    }

    #[test]
    fn read_002_includes_can_generate() {
        let tl = sample_timeline();
        let result = format_timeline(&tl, &TimelineFormatOptions::default());
        assert!(result.can_generate, "READ-002: can_generate should be true");
    }

    #[test]
    fn read_002_can_generate_false_when_no_fps() {
        let mut tl = sample_timeline();
        tl.fps = 0;
        let result = format_timeline(&tl, &TimelineFormatOptions::default());
        assert!(
            !result.can_generate,
            "READ-002: can_generate false when fps=0"
        );
    }

    #[test]
    fn read_003_omits_defaults() {
        let tl = sample_timeline();
        let result = format_timeline(&tl, &TimelineFormatOptions::default());
        let track = &result.tracks[0];
        let clip1 = &track.clips[0];
        let fields = clip1.fields.as_ref().unwrap();
        // clip-001 has speed=1.0, so speed should be omitted
        assert!(
            !fields.as_object().unwrap().contains_key("speed"),
            "READ-003: speed=1.0 should be omitted"
        );
        // volume=1.0 should be omitted
        assert!(
            !fields.as_object().unwrap().contains_key("volume"),
            "READ-003: volume=1.0 should be omitted"
        );
        // But trim fields and link group should be present
        assert!(
            fields.as_object().unwrap().contains_key("trimStartFrame"),
            "READ-003: trimStartFrame=10 should be present"
        );
        assert!(
            fields.as_object().unwrap().contains_key("linkGroupId"),
            "READ-003: linkGroupId should be present"
        );
    }

    #[test]
    fn read_003_non_defaults_are_present() {
        let tl = sample_timeline();
        let result = format_timeline(&tl, &TimelineFormatOptions::default());
        let track = &result.tracks[0];
        let clip2 = &track.clips[1];
        let fields = clip2.fields.as_ref().unwrap();
        // clip-002 has speed=1.5 and volume=0.8
        assert!(
            fields.as_object().unwrap().contains_key("speed"),
            "READ-003: speed=1.5 should be present"
        );
        assert!(
            fields.as_object().unwrap().contains_key("volume"),
            "READ-003: volume=0.8 should be present"
        );
    }

    #[test]
    fn read_004_rounds_numeric_values() {
        let mut tl = sample_timeline();
        tl.tracks[0].clips[1].speed = 1.333_333;
        let result = format_timeline(&tl, &TimelineFormatOptions::default());
        let clip2 = &result.tracks[0].clips[1];
        let fields = clip2.fields.as_ref().unwrap();
        let speed = fields.pointer("/speed").and_then(|v| v.as_f64()).unwrap();
        assert!(
            (speed - 1.333).abs() < 0.001,
            "READ-004: expected ~1.333 got {speed}"
        );
    }

    #[test]
    fn read_005_windowing_filters_clips() {
        let tl = sample_timeline();
        let opts = TimelineFormatOptions {
            window: Some((0, 50)),
            ..Default::default()
        };
        let result = format_timeline(&tl, &opts);
        // clip-001 starts at 0 dur 100 — visible in (0,50)
        // clip-002 starts at 100 dur 50 — outside (0,50)
        // clip-003 starts at 0 dur 200 — visible
        assert_eq!(
            result.tracks[0].clips.len(),
            1,
            "READ-005: only clip-001 in window"
        );
        assert_eq!(
            result.tracks[0].clips[0].id, "clip-001",
            "READ-005: first clip in window"
        );
        assert_eq!(
            result.tracks[1].clips.len(),
            1,
            "READ-005: audio clip in window"
        );
    }

    #[test]
    fn read_006_windowing_reports_total_clips() {
        let tl = sample_timeline();
        let opts = TimelineFormatOptions {
            window: Some((120, 150)),
            ..Default::default()
        };
        let result = format_timeline(&tl, &opts);
        let v_track = &result.tracks[0];
        // Both clips visible: clip-001 (0-100) outside, clip-002 (100-150) inside
        assert_eq!(v_track.clips.len(), 1, "only clip-002 in window (120-150)");
    }

    #[test]
    fn read_010_format_media_entries() {
        let manifest = MediaManifest {
            version: 2,
            entries: vec![MediaManifestEntry {
                id: "media-001".into(),
                name: "beach.mp4".into(),
                r#type: ClipType::Video,
                source: MediaSource::External {
                    absolute_path: "/media/beach.mp4".into(),
                },
                duration: 30.5,
                generation_input: None,
                source_width: None,
                source_height: None,
                source_fps: None,
                has_audio: None,
                folder_id: None,
                cached_remote_url: None,
                cached_remote_url_expires_at: None,
            }],
            folders: vec![],
        };
        let result = format_media_manifest(&manifest, 3);
        assert_eq!(result.entries.len(), 1, "READ-010: one entry");
        assert_eq!(result.entries[0].id, "media-001");
    }

    #[test]
    fn read_010_format_media_with_folders() {
        let manifest = MediaManifest {
            version: 2,
            entries: vec![],
            folders: vec![core_model::MediaFolder {
                id: "folder-001".into(),
                name: "B-Roll".into(),
                parent_folder_id: None,
            }],
        };
        let result = format_media_manifest(&manifest, 3);
        assert!(result.folders.is_some(), "READ-010: folders present");
        assert_eq!(result.folders.unwrap().len(), 1);
    }

    #[test]
    fn read_011_media_rounds_numeric_values() {
        let manifest = MediaManifest {
            version: 2,
            entries: vec![MediaManifestEntry {
                id: "m1".into(),
                name: "test.mp4".into(),
                r#type: ClipType::Video,
                source: MediaSource::External {
                    absolute_path: "/test.mp4".into(),
                },
                duration: 30.55555,
                generation_input: None,
                source_width: None,
                source_height: None,
                source_fps: None,
                has_audio: None,
                folder_id: None,
                cached_remote_url: None,
                cached_remote_url_expires_at: None,
            }],
            folders: vec![],
        };
        let result = format_media_manifest(&manifest, 1);
        let dur = result.entries[0].duration_seconds.unwrap();
        assert!(
            (dur - 30.6).abs() < 0.01,
            "READ-011: expected ~30.6 got {dur}"
        );
    }

    #[test]
    fn read_005_empty_window_shows_no_clips() {
        let tl = sample_timeline();
        let opts = TimelineFormatOptions {
            window: Some((1000, 2000)),
            ..Default::default()
        };
        let result = format_timeline(&tl, &opts);
        for track in &result.tracks {
            assert!(track.clips.is_empty(), "READ-005: all clips outside window");
        }
    }
}

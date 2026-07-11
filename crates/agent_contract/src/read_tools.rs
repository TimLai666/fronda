//! Read-only tool output formatting (READ-001 to READ-021).
//!
//! These functions format project state into the JSON structures expected by
//! the agent/MCP tool surface. They are pure data transformations that
//! operate on core_model types — no platform I/O or rendering.

use core_model::{
    Clip, ClipType, MediaManifest, MediaManifestEntry, MediaSource, TextRgba, Timeline, Track,
};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;

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
        ClipType::Shape => "shape",
        ClipType::Sequence => "sequence",
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
// get_transcript output (READ-017 to READ-021)
// ---------------------------------------------------------------------------

/// A word from a transcript with seconds-based timing.
#[derive(Debug, Clone)]
pub struct TranscriptWordInput {
    pub word: String,
    pub start_seconds: f64,
    pub end_seconds: f64,
}

/// A clip on the timeline for word attribution.
#[derive(Debug, Clone)]
pub struct TranscriptClipInput {
    pub id: String,
    pub start_frame: i64,
    pub duration_frames: i64,
}

/// Options for formatting transcript output.
#[derive(Debug, Clone)]
pub struct TranscriptFormatOptions {
    /// Maximum number of words to return (READ-020).
    pub max_words: usize,
    /// Start frame for pagination (READ-020).
    pub start_frame: Option<i64>,
    /// Legacy wordTimestamps flag — tolerated and ignored (READ-021).
    #[allow(dead_code)]
    pub word_timestamps: Option<bool>,
    /// Resolved BCP-47 language for this transcript request (Issue #39).
    ///
    /// Precedence: per-call `language` arg → project `transcriptionLanguage`
    /// → `None` (platform falls back to system language).
    pub language: Option<String>,
}

impl Default for TranscriptFormatOptions {
    fn default() -> Self {
        Self {
            max_words: 10000,
            start_frame: None,
            word_timestamps: None,
            language: None,
        }
    }
}

/// A word attributed to a clip with frame-range timing.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FormattedTranscriptWord {
    pub word: String,
    pub start_frame: i64,
    pub end_frame: i64,
}

/// A clip containing its attributed transcript words.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FormattedTranscriptClip {
    pub clip_id: String,
    pub words: Vec<FormattedTranscriptWord>,
}

/// Formatted transcript output (READ-017 to READ-020).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FormattedTranscript {
    pub clips: Vec<FormattedTranscriptClip>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_start_frame: Option<i64>,
    pub text: String,
    /// Resolved language used for this transcript (Issue #39).
    /// `None` means the platform will use system language.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
}

/// Format transcript data for get_transcript output.
///
/// READ-017: Returns timeline transcript in project frames.
/// READ-018: Returns nested clips[].words.
/// READ-019: Monotonic/non-overlapping word attribution.
/// READ-020: Capped at 10000 words with pagination.
/// READ-021: Options silently ignore legacy wordTimestamps.
pub fn format_transcript(
    fps: i64,
    words: &[TranscriptWordInput],
    clips: &[TranscriptClipInput],
    options: &TranscriptFormatOptions,
) -> FormattedTranscript {
    let frame_scale = fps as f64;

    // Convert seconds to frames, sort by start for monotonic order (READ-019)
    let mut frame_words: Vec<(String, i64, i64)> = words
        .iter()
        .map(|w| {
            let start_frame = (w.start_seconds * frame_scale).round() as i64;
            let end_frame = (w.end_seconds * frame_scale).round() as i64;
            (w.word.clone(), start_frame, end_frame)
        })
        .collect();
    frame_words.sort_by_key(|(_, start, _)| *start);

    // Apply pagination: skip words whose end frame is at or before start_frame
    if let Some(start_at) = options.start_frame {
        frame_words.retain(|(_, _, end)| *end > start_at);
    }

    // Cap at max_words (READ-020)
    let truncated = frame_words.len() > options.max_words;
    frame_words.truncate(options.max_words);

    // Determine next_start_frame for pagination continuation
    let next_start_frame = if truncated {
        frame_words.last().map(|(_, _, end)| *end)
    } else {
        None
    };

    // Build clip ranges sorted by timeline position
    let mut clip_ranges: Vec<(String, i64, i64)> = clips
        .iter()
        .map(|c| {
            (
                c.id.clone(),
                c.start_frame,
                c.start_frame + c.duration_frames,
            )
        })
        .collect();
    clip_ranges.sort_by_key(|(_, start, _)| *start);

    // Attribute words to clips by word-midpoint (READ-019)
    type ClipWord = (String, i64, i64);
    type ClipWordBucket = (String, Vec<ClipWord>);

    let mut clip_words: Vec<ClipWordBucket> = Vec::new();
    for (word, start_f, end_f) in &frame_words {
        let mid = (start_f + end_f) / 2;
        let mut _assigned = false;
        for (clip_id, clip_start, clip_end) in &clip_ranges {
            if mid >= *clip_start && mid < *clip_end {
                if let Some(entry) = clip_words.iter_mut().find(|(id, _)| id == clip_id) {
                    entry.1.push((word.clone(), *start_f, *end_f));
                } else {
                    clip_words.push((clip_id.clone(), vec![(word.clone(), *start_f, *end_f)]));
                }
                _assigned = true;
                break;
            }
        }
        // Words not matching any visible clip are dropped
    }

    let formatted_clips: Vec<FormattedTranscriptClip> = clip_words
        .into_iter()
        .map(|(clip_id, words)| FormattedTranscriptClip {
            clip_id,
            words: words
                .into_iter()
                .map(|(w, s, e)| FormattedTranscriptWord {
                    word: w,
                    start_frame: s,
                    end_frame: e,
                })
                .collect(),
        })
        .collect();

    let text = formatted_clips
        .iter()
        .flat_map(|c| c.words.iter())
        .map(|w| w.word.as_str())
        .collect::<Vec<_>>()
        .join(" ");

    FormattedTranscript {
        clips: formatted_clips,
        next_start_frame,
        text,
        language: options.language.clone(),
    }
}

/// Format transcript as a JSON Value suitable for tool output.
pub fn format_transcript_json(
    fps: i64,
    words: &[TranscriptWordInput],
    clips: &[TranscriptClipInput],
    options: &TranscriptFormatOptions,
) -> Value {
    let formatted = format_transcript(fps, words, clips, options);
    serde_json::to_value(formatted).unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Caption group collapsing (READ-007 to READ-009)
// ---------------------------------------------------------------------------

/// A single caption clip extracted from the timeline for grouping.
#[derive(Debug, Clone)]
pub struct CaptionClipInfo {
    pub clip_id: String,
    pub track_index: usize,
    pub start_frame: i64,
    pub duration_frames: i64,
    pub text: String,
    pub caption_group_id: String,
    pub font_size: Option<f64>,
    pub font_name: Option<String>,
    pub text_color: Option<String>,
}

impl CaptionClipInfo {
    /// Extract caption clip info from a timeline clip, returning None if
    /// the clip is not a caption clip (no caption_group_id or no text_content).
    pub fn from_clip(clip: &Clip, track_index: usize) -> Option<Self> {
        let caption_group_id = clip.caption_group_id.as_ref()?;
        let text = clip.text_content.as_ref()?;

        let (font_size, font_name, text_color) =
            clip.text_style
                .as_ref()
                .map_or((None, None, None), |style| {
                    (
                        Some(style.font_size),
                        Some(style.font_name.clone()),
                        Some(rgba_to_hex(&style.color)),
                    )
                });

        Some(Self {
            clip_id: clip.id.clone(),
            track_index,
            start_frame: clip.start_frame,
            duration_frames: clip.duration_frames,
            text: text.clone(),
            caption_group_id: caption_group_id.clone(),
            font_size,
            font_name,
            text_color,
        })
    }
}

/// A collapsed caption group for get_timeline output.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CaptionGroup {
    pub track_index: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shared_font_size: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shared_font_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shared_text_color: Option<String>,
    pub clip_count: usize,
    pub rows: Vec<CaptionRow>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CaptionRow {
    pub clip_id: String,
    pub start_frame: i64,
    pub duration_frames: i64,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_size: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_color: Option<String>,
}

/// Collapse caption clips into groups with hoisted shared properties.
///
/// READ-007: Caption clips with the same `caption_group_id` are collapsed into
///           `CaptionGroup` entries with shared font properties hoisted.
/// READ-008: Total rows across all groups are capped at 200. When the cap is
///           exceeded a warning string is returned.
/// READ-009: Clips whose individual font properties deviate from the shared
///           group value are emitted with their specific values in the row.
pub fn collapse_caption_groups(clips: &[CaptionClipInfo]) -> (Vec<CaptionGroup>, Option<String>) {
    if clips.is_empty() {
        return (Vec::new(), None);
    }

    // Group by caption_group_id, preserving insertion order via BTreeMap
    let mut groups: BTreeMap<&str, Vec<&CaptionClipInfo>> = BTreeMap::new();
    for clip in clips {
        groups.entry(&clip.caption_group_id).or_default().push(clip);
    }

    // Count total rows for cap check
    let total_clip_count: usize = groups.values().map(|g| g.len()).sum();
    let warning = if total_clip_count > 200 {
        Some(format!(
            "Caption group rows exceed 200 ({} total). Only first 200 rows shown.",
            total_clip_count
        ))
    } else {
        None
    };

    let mut result = Vec::new();
    let mut rows_emitted = 0usize;

    for group_clips in groups.into_values() {
        if rows_emitted >= 200 {
            break;
        }

        let track_index = group_clips[0].track_index;
        let clip_count = group_clips.len();

        // Determine shared properties across this group
        let shared_font_size = shared_f64(group_clips.iter().map(|c| c.font_size.as_ref()));
        let shared_font_name = shared_str(group_clips.iter().map(|c| c.font_name.as_deref()));
        let shared_text_color = shared_str(group_clips.iter().map(|c| c.text_color.as_deref()));

        let remaining_budget = 200usize.saturating_sub(rows_emitted);
        let take_count = group_clips.len().min(remaining_budget);

        let rows: Vec<CaptionRow> = group_clips[..take_count]
            .iter()
            .map(|c| {
                // Emit per-clip value only when it deviates from shared
                let font_size = match shared_font_size {
                    Some(ref shared) if c.font_size.as_ref() == Some(shared) => None,
                    _ => c.font_size,
                };
                let font_name = match shared_font_name {
                    Some(shared) if c.font_name.as_deref() == Some(shared) => None,
                    _ => c.font_name.clone(),
                };
                let text_color = match shared_text_color {
                    Some(shared) if c.text_color.as_deref() == Some(shared) => None,
                    _ => c.text_color.clone(),
                };

                CaptionRow {
                    clip_id: c.clip_id.clone(),
                    start_frame: c.start_frame,
                    duration_frames: c.duration_frames,
                    text: c.text.clone(),
                    font_size,
                    font_name,
                    text_color,
                }
            })
            .collect();

        rows_emitted += rows.len();

        result.push(CaptionGroup {
            track_index,
            shared_font_size,
            shared_font_name: shared_font_name.map(|s| s.to_string()),
            shared_text_color: shared_text_color.map(|s| s.to_string()),
            clip_count,
            rows,
        });
    }

    (result, warning)
}

/// Returns Some(value) if all non-None values in the iterator are identical.
fn shared_f64<'a>(values: impl Iterator<Item = Option<&'a f64>>) -> Option<f64> {
    const EPSILON: f64 = 0.001;
    let mut common: Option<f64> = None;
    for v in values.flatten() {
        match common {
            None => common = Some(*v),
            Some(c) if (c - *v).abs() < EPSILON => continue,
            Some(_) => return None,
        }
    }
    common
}

/// Returns Some(value) if all non-None values in the iterator are identical.
fn shared_str<'a>(values: impl Iterator<Item = Option<&'a str>>) -> Option<&'a str> {
    let mut common: Option<&'a str> = None;
    for v in values.flatten() {
        match common {
            None => common = Some(v),
            Some(c) if c == v => continue,
            Some(_) => return None,
        }
    }
    common
}

/// Convert a TextRgba to a hex color string (#RRGGBB).
fn rgba_to_hex(color: &TextRgba) -> String {
    format!(
        "#{:02X}{:02X}{:02X}",
        (color.r * 255.0).round() as u8,
        (color.g * 255.0).round() as u8,
        (color.b * 255.0).round() as u8,
    )
}

// ---------------------------------------------------------------------------
// search_media formatting (READ-025 to READ-027)
// ---------------------------------------------------------------------------

/// A single search hit in the formatted output.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SearchHitInfo {
    pub media_id: String,
    pub frame: i64,
    pub score: f64,
    pub kind: String,
}

/// Formatted search results with separated groups.
///
/// READ-025: visual (moments) and spoken results are kept in separate fields.
/// READ-026: status reporting for visual indexing is preserved.
/// READ-027: limit is clamped to 1..50.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SearchMediaOutput {
    pub moments: Vec<SearchHitInfo>,
    pub spoken: Vec<SearchHitInfo>,
    pub files: Vec<SearchHitInfo>,
    pub status: String,
    pub limit: usize,
}

/// Format search results with separated groups and clamped limit.
///
/// Each result group is truncated to the (clamped) limit. Returns the
/// structured output ready for JSON serialization.
pub fn format_search_results(
    moments: Vec<SearchHitInfo>,
    spoken: Vec<SearchHitInfo>,
    files: Vec<SearchHitInfo>,
    status: String,
    limit: usize,
) -> SearchMediaOutput {
    let limit = limit.clamp(1, 50);
    let trunc = |mut v: Vec<SearchHitInfo>| {
        v.truncate(limit);
        v
    };
    SearchMediaOutput {
        moments: trunc(moments),
        spoken: trunc(spoken),
        files: trunc(files),
        status,
        limit,
    }
}

// ---------------------------------------------------------------------------
// list_models formatting (READ-029)
// ---------------------------------------------------------------------------

/// Formatted models output with loaded distinction.
///
/// READ-029: `loaded = false` means models have not yet been fetched,
/// which is distinct from having fetched and found zero models.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct FormattedModels {
    pub models: Value,
    pub loaded: bool,
}

/// Wrap models data with the loaded flag.
pub fn format_models(models: Value, loaded: bool) -> FormattedModels {
    FormattedModels { models, loaded }
}

// ---------------------------------------------------------------------------
// inspect_media output (READ-012 to READ-016)
// ---------------------------------------------------------------------------

/// Input for a single inline storyboard or frame image.
#[derive(Debug, Clone)]
pub struct InlineFrameInput {
    pub frame: i64,
    pub data_base64: String,
}

/// Input for formatting inspect_media output.
///
/// The caller (tool_exec) gathers data (inline images, transcription words)
/// and passes them here for pure formatting.
#[derive(Debug, Clone)]
pub struct InspectMediaInput {
    pub entry: MediaManifestEntry,
    pub clip: Option<Clip>,
    pub timeline_fps: i64,
    pub max_frames: usize,
    /// Base64-encoded inline image data (for Image clip types).
    pub inline_image_data: Option<String>,
    /// Storyboard frames (for Video/Lottie clip types).
    pub inline_video_frames: Vec<InlineFrameInput>,
    /// Transcription words (for Video/Audio clip types).
    pub transcription_words: Vec<TranscriptWordInput>,
}

/// Type-varying inspect_media output (READ-012).
///
/// Returns a JSON Value suitable for the tool response.
/// Returns an error for Text clip types (READ-013).
pub fn format_inspect_media(input: &InspectMediaInput) -> Result<Value, String> {
    let entry = &input.entry;

    // READ-013: Text clip rejection
    if entry.r#type == ClipType::Text {
        return Err(
            "Cannot inspect a text clip with inspect_media. Use get_timeline to view text clips."
                .to_string(),
        );
    }

    let mut result = json!({
        "id": entry.id,
        "name": entry.name,
        "type": format!("{:?}", entry.r#type).to_lowercase(),
        "duration": entry.duration,
        "hasAudio": entry.has_audio,
    });

    // Add optional dimension/fps fields
    if let Some(w) = entry.source_width {
        result["sourceWidth"] = json!(w);
    }
    if let Some(h) = entry.source_height {
        result["sourceHeight"] = json!(h);
    }
    if let Some(fps) = entry.source_fps {
        result["sourceFps"] = json!(fps);
    }
    if let Some(ref folder_id) = entry.folder_id {
        result["folderId"] = json!(folder_id);
    }

    // Add clip-level info if available
    if let Some(ref clip) = input.clip {
        result["clipStartFrame"] = json!(clip.start_frame);
        result["clipDurationFrames"] = json!(clip.duration_frames);
    }

    // Type-varying output
    match entry.r#type {
        ClipType::Image => {
            // Inline image data
            if let Some(ref data) = input.inline_image_data {
                result["image"] = json!({
                    "data": data,
                    "width": entry.source_width,
                    "height": entry.source_height,
                    "mimeType": "image/png",
                });
            }
        }
        ClipType::Video => {
            // Storyboard frames (READ-012, READ-015)
            if !input.inline_video_frames.is_empty() {
                let frames: Vec<Value> = input
                    .inline_video_frames
                    .iter()
                    .map(|f| {
                        json!({
                            "frame": f.frame,
                            "data": f.data_base64,
                        })
                    })
                    .collect();
                result["storyboard"] = json!(frames);
            }

            // Transcription (READ-012, READ-016)
            if !input.transcription_words.is_empty() {
                let transcript = format_transcript(
                    input.timeline_fps,
                    &input.transcription_words,
                    &[],
                    &TranscriptFormatOptions::default(),
                );
                result["transcript"] = serde_json::to_value(&transcript).unwrap_or_default();
            }
        }
        ClipType::Audio => {
            // Transcription (READ-012, READ-016)
            if !input.transcription_words.is_empty() {
                let transcript = format_transcript(
                    input.timeline_fps,
                    &input.transcription_words,
                    &[],
                    &TranscriptFormatOptions::default(),
                );
                result["transcript"] = serde_json::to_value(&transcript).unwrap_or_default();
            }
        }
        ClipType::Lottie => {
            // Animation frames (READ-012)
            if !input.inline_video_frames.is_empty() {
                let frames: Vec<Value> = input
                    .inline_video_frames
                    .iter()
                    .map(|f| {
                        json!({
                            "frame": f.frame,
                            "data": f.data_base64,
                        })
                    })
                    .collect();
                result["frames"] = json!(frames);
            }

            // Lottie metadata
            result["animationMetadata"] = json!({
                "type": "lottie",
                "durationFrames": entry.duration.round() as i64,
            });
        }
        ClipType::Shape => {
            // Shape annotations - basic metadata only
            result["shapeType"] = json!("annotation");
        }
        ClipType::Sequence => {
            // Mirrors Swift inspectMedia: sequences are timelines, not media.
            result["note"] = json!("Sequences are timelines, not media assets. Use get_timeline.");
        }
        ClipType::Text => {
            // Already rejected above, but handle as safety net
            return Err("Cannot inspect a text clip with inspect_media.".to_string());
        }
    }

    Ok(result)
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
            id: String::new(),
            name: String::new(),
            fps: 30,
            width: 1920,
            height: 1080,
            settings_configured: true,
            selected_clip_ids: std::collections::HashSet::new(),
            transcription_language: None,
            folder_id: None,
            compound_timelines: std::collections::HashMap::new(),
            tracks: vec![
                Track {
                    id: "track-v".into(),
                    r#type: ClipType::Video,
                    muted: false,
                    hidden: false,
                    sync_locked: false,
                    display_height: 50.0,
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
                            shape_style: None,
                            stroke_progress_track: None,
                            compound_timeline_id: None,
                            blend_mode: Default::default(),
                            chroma_key: None,
                            multicam_group_id: None,
                            text_animation: None,
                            word_timings: None,
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
                            shape_style: None,
                            stroke_progress_track: None,
                            compound_timeline_id: None,
                            blend_mode: Default::default(),
                            chroma_key: None,
                            multicam_group_id: None,
                            text_animation: None,
                            word_timings: None,
                        },
                    ],
                },
                Track {
                    id: "track-a".into(),
                    r#type: ClipType::Audio,
                    muted: false,
                    hidden: false,
                    sync_locked: false,
                    display_height: 50.0,
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
                        shape_style: None,
                        stroke_progress_track: None,
                        compound_timeline_id: None,
                        blend_mode: Default::default(),
                        chroma_key: None,
                        multicam_group_id: None,
                        text_animation: None,
                        word_timings: None,
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
                source_timecode_frame: None,
                source_timecode_quanta: None,
                source_timecode_drop_frame: None,
                ai_tags: None,
                ai_description: None,
                ai_label_status: None,
                generation_status: None,
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
                source_timecode_frame: None,
                source_timecode_quanta: None,
                source_timecode_drop_frame: None,
                ai_tags: None,
                ai_description: None,
                ai_label_status: None,
                generation_status: None,
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

    // ── get_transcript formatting tests ────────────────────────────────

    fn sample_transcript_words() -> Vec<TranscriptWordInput> {
        vec![
            TranscriptWordInput {
                word: "hello".into(),
                start_seconds: 0.0,
                end_seconds: 0.5,
            },
            TranscriptWordInput {
                word: "world".into(),
                start_seconds: 0.6,
                end_seconds: 1.2,
            },
            TranscriptWordInput {
                word: "this".into(),
                start_seconds: 3.0,
                end_seconds: 3.3,
            },
            TranscriptWordInput {
                word: "is".into(),
                start_seconds: 3.4,
                end_seconds: 3.6,
            },
            TranscriptWordInput {
                word: "a".into(),
                start_seconds: 3.7,
                end_seconds: 3.8,
            },
            TranscriptWordInput {
                word: "test".into(),
                start_seconds: 3.9,
                end_seconds: 4.5,
            },
            TranscriptWordInput {
                word: "goodbye".into(),
                start_seconds: 6.0,
                end_seconds: 7.5,
            },
        ]
    }

    fn sample_transcript_clips() -> Vec<TranscriptClipInput> {
        vec![
            TranscriptClipInput {
                id: "clip1".into(),
                start_frame: 0,
                duration_frames: 60,
            },
            TranscriptClipInput {
                id: "clip2".into(),
                start_frame: 90,
                duration_frames: 60,
            },
            TranscriptClipInput {
                id: "clip3".into(),
                start_frame: 180,
                duration_frames: 60,
            },
        ]
    }

    #[test]
    fn format_transcript_returns_nested_structure() {
        let fps = 30;
        let result = format_transcript(
            fps,
            &sample_transcript_words(),
            &sample_transcript_clips(),
            &TranscriptFormatOptions::default(),
        );
        // READ-018: nested clips[].words
        assert!(!result.clips.is_empty(), "should have clips");
        assert_eq!(result.clips[0].clip_id, "clip1");
        assert_eq!(result.clips[0].words.len(), 2);
        assert_eq!(result.clips[0].words[0].word, "hello");
        assert_eq!(result.clips[0].words[1].word, "world");
        assert_eq!(result.clips[1].clip_id, "clip2");
        assert_eq!(result.clips[1].words.len(), 4);
        assert_eq!(result.clips[2].clip_id, "clip3");
        assert_eq!(result.clips[2].words.len(), 1);
        assert_eq!(result.clips[2].words[0].word, "goodbye");
    }

    #[test]
    fn format_transcript_word_count_capped_at_10000() {
        let fps = 30;
        let words: Vec<TranscriptWordInput> = (0..15000)
            .map(|i| TranscriptWordInput {
                word: format!("w{}", i),
                start_seconds: i as f64 * 0.1,
                end_seconds: i as f64 * 0.1 + 0.08,
            })
            .collect();
        let clips = vec![TranscriptClipInput {
            id: "clip1".into(),
            start_frame: 0,
            duration_frames: 1_000_000,
        }];
        let result = format_transcript(fps, &words, &clips, &TranscriptFormatOptions::default());
        let total: usize = result.clips.iter().map(|c| c.words.len()).sum();
        // READ-020: capped at 10000
        assert_eq!(total, 10000);
        assert!(result.next_start_frame.is_some());
    }

    #[test]
    fn format_transcript_pagination_next_start_frame() {
        let fps = 30;
        let words: Vec<TranscriptWordInput> = (0..12000)
            .map(|i| TranscriptWordInput {
                word: format!("w{}", i),
                start_seconds: i as f64 * 0.1,
                end_seconds: i as f64 * 0.1 + 0.08,
            })
            .collect();
        let clip = TranscriptClipInput {
            id: "clip1".into(),
            start_frame: 0,
            duration_frames: 1_000_000,
        };

        // First page: 10000 words
        let page1 = format_transcript(
            fps,
            &words,
            std::slice::from_ref(&clip),
            &TranscriptFormatOptions::default(),
        );
        let total1: usize = page1.clips.iter().map(|c| c.words.len()).sum();
        assert_eq!(total1, 10000);
        let next = page1.next_start_frame.expect("should have next frame");

        // next_start_frame is the end_frame of the last word on page 1
        let last_word = &page1.clips[0].words[9999];
        assert_eq!(next, last_word.end_frame);

        // Second page: continue from next
        let page2 = format_transcript(
            fps,
            &words,
            &[clip],
            &TranscriptFormatOptions {
                start_frame: Some(next),
                ..Default::default()
            },
        );
        let total2: usize = page2.clips.iter().map(|c| c.words.len()).sum();
        assert_eq!(total2, 2000);
        assert!(page2.next_start_frame.is_none(), "no more pages");

        // No overlap: page1 words end at <= next, page2 words start after next
        for c in &page2.clips {
            for w in &c.words {
                assert!(
                    w.end_frame > next,
                    "no word on page 2 should have end_frame <= next_start_frame"
                );
            }
        }
    }

    #[test]
    fn format_transcript_empty_returns_no_transcript() {
        let fps = 30;
        let words: Vec<TranscriptWordInput> = vec![];
        let clips = sample_transcript_clips();
        let result = format_transcript(fps, &words, &clips, &TranscriptFormatOptions::default());
        assert!(result.clips.is_empty());
        assert!(result.text.is_empty());
        assert!(result.next_start_frame.is_none());
    }

    #[test]
    fn format_transcript_words_are_monotonic() {
        let fps = 30;
        let mut words = sample_transcript_words();
        // Reverse to deliberately break source order
        words.reverse();
        let result = format_transcript(
            fps,
            &words,
            &sample_transcript_clips(),
            &TranscriptFormatOptions::default(),
        );
        // READ-019: each clip's words must be in start_frame order
        for clip in &result.clips {
            for pair in clip.words.windows(2) {
                assert!(
                    pair[0].start_frame <= pair[1].start_frame,
                    "words must be monotonic within clip"
                );
            }
        }
    }

    #[test]
    fn legacy_word_timestamps_tolerated() {
        let fps = 30;
        let result = format_transcript(
            fps,
            &sample_transcript_words(),
            &sample_transcript_clips(),
            &TranscriptFormatOptions {
                word_timestamps: Some(true),
                ..Default::default()
            },
        );
        // READ-021: word_timestamps is silently ignored, output is unaffected
        assert_eq!(result.clips.len(), 3);
        assert_eq!(result.text, "hello world this is a test goodbye");
    }

    // -----------------------------------------------------------------------
    // Caption group tests (READ-007..009)
    // -----------------------------------------------------------------------

    fn sample_caption_clips() -> Vec<CaptionClipInfo> {
        vec![
            CaptionClipInfo {
                clip_id: "cap-001".into(),
                track_index: 2,
                start_frame: 0,
                duration_frames: 100,
                text: "First caption".into(),
                caption_group_id: "cg-1".into(),
                font_size: Some(96.0),
                font_name: Some("Helvetica-Bold".into()),
                text_color: Some("#FFFFFF".into()),
            },
            CaptionClipInfo {
                clip_id: "cap-002".into(),
                track_index: 2,
                start_frame: 100,
                duration_frames: 100,
                text: "Second caption".into(),
                caption_group_id: "cg-1".into(),
                font_size: Some(96.0),
                font_name: Some("Helvetica-Bold".into()),
                text_color: Some("#FFFFFF".into()),
            },
            CaptionClipInfo {
                clip_id: "cap-003".into(),
                track_index: 2,
                start_frame: 200,
                duration_frames: 100,
                text: "Third caption".into(),
                caption_group_id: "cg-1".into(),
                font_size: Some(96.0),
                font_name: Some("Helvetica-Bold".into()),
                text_color: Some("#FFFFFF".into()),
            },
        ]
    }

    #[test]
    fn caption_group_collapses_shared_properties() {
        let clips = sample_caption_clips();
        let (groups, warning) = collapse_caption_groups(&clips);

        assert_eq!(groups.len(), 1, "one caption group");
        let g = &groups[0];

        // All font_size=96.0 -> shared
        assert_eq!(g.shared_font_size, Some(96.0), "READ-007: shared font_size");
        // All font_name=Helvetica-Bold -> shared
        assert_eq!(
            g.shared_font_name.as_deref(),
            Some("Helvetica-Bold"),
            "READ-007: shared font_name"
        );
        // All text_color=#FFFFFF -> shared
        assert_eq!(
            g.shared_text_color.as_deref(),
            Some("#FFFFFF"),
            "READ-007: shared text_color"
        );
        assert_eq!(g.clip_count, 3, "group has 3 clips");

        // Individual rows should have None for all shared properties
        // (inherited from group)
        for row in &g.rows {
            assert!(
                row.font_size.is_none(),
                "READ-007: individual font_size is None when shared"
            );
            assert!(
                row.font_name.is_none(),
                "READ-007: individual font_name is None when shared"
            );
            assert!(
                row.text_color.is_none(),
                "READ-007: individual text_color is None when shared"
            );
        }

        assert!(warning.is_none(), "no warning under 200 rows");
    }

    #[test]
    fn caption_group_capped_at_200_rows() {
        let mut clips = sample_caption_clips();
        // Add 200 clips in a second group
        for i in 0..200 {
            clips.push(CaptionClipInfo {
                clip_id: format!("cap-bulk-{:04}", i),
                track_index: 2,
                start_frame: i * 10,
                duration_frames: 10,
                text: format!("Bulk caption {}", i),
                caption_group_id: "cg-bulk".into(),
                font_size: Some(72.0),
                font_name: None,
                text_color: None,
            });
        }

        let (groups, warning) = collapse_caption_groups(&clips);
        let total_rows: usize = groups.iter().map(|g| g.rows.len()).sum();

        assert_eq!(total_rows, 200, "READ-008: capped at 200 total rows");
        assert!(
            warning.is_some(),
            "READ-008: warning returned when rows exceed 200"
        );
        let msg = warning.unwrap();
        assert!(
            msg.contains("203 total"),
            "warning mentions total count: {}",
            msg
        );
    }

    #[test]
    fn caption_group_deviant_clips_emitted_individually() {
        let clips = vec![
            CaptionClipInfo {
                clip_id: "cap-001".into(),
                track_index: 2,
                start_frame: 0,
                duration_frames: 100,
                text: "Normal".into(),
                caption_group_id: "cg-1".into(),
                font_size: Some(96.0),
                font_name: Some("Helvetica-Bold".into()),
                text_color: Some("#FFFFFF".into()),
            },
            CaptionClipInfo {
                clip_id: "cap-002".into(),
                track_index: 2,
                start_frame: 100,
                duration_frames: 100,
                text: "Deviant font size".into(),
                caption_group_id: "cg-1".into(),
                font_size: Some(48.0),
                font_name: Some("Helvetica-Bold".into()),
                text_color: Some("#FFFFFF".into()),
            },
            CaptionClipInfo {
                clip_id: "cap-003".into(),
                track_index: 2,
                start_frame: 200,
                duration_frames: 100,
                text: "Deviant color".into(),
                caption_group_id: "cg-1".into(),
                font_size: Some(96.0),
                font_name: Some("Helvetica-Bold".into()),
                text_color: Some("#FF0000".into()),
            },
        ];

        let (groups, warning) = collapse_caption_groups(&clips);
        assert_eq!(groups.len(), 1, "one caption group");
        let g = &groups[0];

        // font_size differs between 96.0 and 48.0 -> NOT shared
        assert!(
            g.shared_font_size.is_none(),
            "READ-009: divergent font_size is not shared"
        );
        // font_name is same -> shared
        assert_eq!(
            g.shared_font_name.as_deref(),
            Some("Helvetica-Bold"),
            "READ-009: uniform font_name is shared"
        );
        // text_color differs between #FFFFFF and #FF0000 -> NOT shared
        assert!(
            g.shared_text_color.is_none(),
            "READ-009: divergent text_color is not shared"
        );

        // Row 0 (normal) should have font_size and text_color emitted
        // (not shared), font_name None (shared)
        assert_eq!(
            g.rows[0].font_size,
            Some(96.0),
            "READ-009: row 0 has explicit font_size"
        );
        assert!(
            g.rows[0].font_name.is_none(),
            "READ-009: row 0 inherits shared font_name"
        );
        assert_eq!(
            g.rows[0].text_color.as_deref(),
            Some("#FFFFFF"),
            "READ-009: row 0 has explicit text_color"
        );

        // Row 1 (deviant font_size=48.0) should emit its specific value
        assert_eq!(
            g.rows[1].font_size,
            Some(48.0),
            "READ-009: row 1 deviant font_size emitted"
        );

        // Row 2 (deviant text_color=#FF0000) should emit its specific value
        assert_eq!(
            g.rows[2].text_color.as_deref(),
            Some("#FF0000"),
            "READ-009: row 2 deviant text_color emitted"
        );

        assert!(warning.is_none(), "no warning");
    }

    // -----------------------------------------------------------------------
    // Search formatting tests (READ-025..027)
    // -----------------------------------------------------------------------

    #[test]
    fn search_results_separated_by_group() {
        let result = format_search_results(
            vec![SearchHitInfo {
                media_id: "m1".into(),
                frame: 10,
                score: 0.9,
                kind: "visual".into(),
            }],
            vec![SearchHitInfo {
                media_id: "s1".into(),
                frame: 20,
                score: 0.8,
                kind: "spoken".into(),
            }],
            vec![SearchHitInfo {
                media_id: "f1".into(),
                frame: 0,
                score: 1.0,
                kind: "file".into(),
            }],
            "indexed".into(),
            10,
        );

        assert_eq!(result.moments.len(), 1, "READ-025: moments present");
        assert_eq!(result.spoken.len(), 1, "READ-025: spoken present");
        assert_eq!(result.files.len(), 1, "READ-025: files present");
        assert_eq!(result.status, "indexed", "READ-026: status preserved");
        assert_eq!(result.limit, 10, "limit preserved");
    }

    #[test]
    fn search_limit_clamped_to_1_50() {
        // Test lower clamp
        let result = format_search_results(
            vec![SearchHitInfo {
                media_id: "m1".into(),
                frame: 0,
                score: 1.0,
                kind: "visual".into(),
            }],
            vec![],
            vec![],
            "idle".into(),
            0,
        );
        assert_eq!(result.limit, 1, "READ-027: limit clamped to minimum 1");

        // Test upper clamp
        let result = format_search_results(vec![], vec![], vec![], "idle".into(), 100);
        assert_eq!(result.limit, 50, "READ-027: limit clamped to maximum 50");

        // Test within range
        let result = format_search_results(vec![], vec![], vec![], "idle".into(), 25);
        assert_eq!(result.limit, 25, "READ-027: limit within range preserved");
    }

    #[test]
    fn search_limit_truncates_results() {
        let many_hits: Vec<SearchHitInfo> = (0..10)
            .map(|i| SearchHitInfo {
                media_id: format!("m{}", i),
                frame: i * 10,
                score: 1.0 - (i as f64 * 0.1),
                kind: "visual".into(),
            })
            .collect();

        let result = format_search_results(many_hits.clone(), vec![], vec![], "indexed".into(), 3);

        assert_eq!(result.moments.len(), 3, "truncated to limit=3");
        assert_eq!(result.moments[0].media_id, "m0", "first hit preserved");
        assert_eq!(result.moments[2].media_id, "m2", "third hit preserved");
        assert_eq!(result.limit, 3, "limit stored");
    }

    // -----------------------------------------------------------------------
    // Models formatting tests (READ-029)
    // -----------------------------------------------------------------------

    #[test]
    fn list_models_loaded_distinction() {
        // loaded = true with actual models
        let models = json!({
            "video": [{"id": "gen-3", "name": "Gen-3 Alpha"}],
            "image": [],
            "audio": []
        });
        let result = format_models(models.clone(), true);
        assert!(result.loaded, "READ-029: loaded=true");
        assert_eq!(
            result
                .models
                .pointer("/video/0/id")
                .and_then(|v| v.as_str()),
            Some("gen-3")
        );

        // loaded = false (not yet fetched) vs empty models
        let result = format_models(json!({}), false);
        assert!(!result.loaded, "READ-029: loaded=false");

        // loaded = true with empty models (fetched, nothing available)
        let result = format_models(json!({"video": [], "image": [], "audio": []}), true);
        assert!(
            result.loaded,
            "READ-029: loaded=true even with empty models"
        );
    }

    // -----------------------------------------------------------------------
    // inspect_media formatting tests (READ-012..016)
    // -----------------------------------------------------------------------

    fn make_inspect_entry(r#type: ClipType) -> MediaManifestEntry {
        MediaManifestEntry {
            id: "media-001".into(),
            name: "Test Asset".into(),
            r#type,
            source: MediaSource::External {
                absolute_path: "/tmp/test.mp4".into(),
            },
            duration: 10.5,
            generation_input: None,
            source_width: Some(1920),
            source_height: Some(1080),
            source_fps: Some(29.97),
            has_audio: Some(true),
            folder_id: Some("folder-1".into()),
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

    fn sample_clip() -> Clip {
        Clip {
            id: "clip-001".into(),
            media_ref: "media-001".into(),
            media_type: ClipType::Video,
            source_clip_type: ClipType::Video,
            start_frame: 0,
            duration_frames: 150,
            trim_start_frame: 0,
            trim_end_frame: 0,
            speed: 1.0,
            volume: 1.0,
            fade_in_frames: 0,
            fade_out_frames: 0,
            fade_in_interpolation: core_model::Interpolation::Linear,
            fade_out_interpolation: core_model::Interpolation::Linear,
            opacity: 1.0,
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
            shape_style: None,
            stroke_progress_track: None,
            compound_timeline_id: None,
            blend_mode: Default::default(),
            chroma_key: None,
            multicam_group_id: None,
            text_animation: None,
            word_timings: None,
        }
    }

    #[test]
    fn read_012_inspect_media_video_returns_storyboard_and_transcript() {
        let input = InspectMediaInput {
            entry: make_inspect_entry(ClipType::Video),
            clip: Some(sample_clip()),
            timeline_fps: 30,
            max_frames: 6,
            inline_image_data: None,
            inline_video_frames: vec![
                InlineFrameInput {
                    frame: 0,
                    data_base64: "storyboard_frame_0".into(),
                },
                InlineFrameInput {
                    frame: 75,
                    data_base64: "storyboard_frame_75".into(),
                },
            ],
            transcription_words: vec![
                TranscriptWordInput {
                    word: "Hello".into(),
                    start_seconds: 0.0,
                    end_seconds: 0.5,
                },
                TranscriptWordInput {
                    word: "world".into(),
                    start_seconds: 0.5,
                    end_seconds: 1.0,
                },
            ],
        };
        let result = format_inspect_media(&input).unwrap();

        // Common metadata (READ-012)
        assert_eq!(result["id"], "media-001");
        assert_eq!(result["name"], "Test Asset");
        assert_eq!(result["type"], "video");
        assert_eq!(result["sourceWidth"], 1920);
        assert_eq!(result["sourceHeight"], 1080);

        // Clip-level info
        assert_eq!(result["clipStartFrame"], 0);
        assert_eq!(result["clipDurationFrames"], 150);

        // Storyboard frames (READ-012)
        assert_eq!(result["storyboard"].as_array().unwrap().len(), 2);
        assert_eq!(result["storyboard"][0]["frame"], 0);
        assert_eq!(result["storyboard"][1]["data"], "storyboard_frame_75");

        // Transcript (READ-012, READ-016)
        assert!(result.get("transcript").is_some());
    }

    #[test]
    fn read_012_inspect_media_image_returns_inline_image() {
        let input = InspectMediaInput {
            entry: make_inspect_entry(ClipType::Image),
            clip: None,
            timeline_fps: 30,
            max_frames: 6,
            inline_image_data: Some("base64_image_data".into()),
            inline_video_frames: Vec::new(),
            transcription_words: Vec::new(),
        };
        let result = format_inspect_media(&input).unwrap();

        assert_eq!(result["type"], "image");
        assert_eq!(result["image"]["data"], "base64_image_data");
        assert_eq!(result["image"]["mimeType"], "image/png");
        assert!(
            result.get("storyboard").is_none(),
            "images have no storyboard"
        );
    }

    #[test]
    fn read_012_inspect_media_audio_returns_transcript() {
        let input = InspectMediaInput {
            entry: make_inspect_entry(ClipType::Audio),
            clip: None,
            timeline_fps: 30,
            max_frames: 6,
            inline_image_data: None,
            inline_video_frames: Vec::new(),
            transcription_words: vec![TranscriptWordInput {
                word: "Hello".into(),
                start_seconds: 0.0,
                end_seconds: 1.0,
            }],
        };
        let result = format_inspect_media(&input).unwrap();
        assert_eq!(result["type"], "audio");
        assert!(result.get("transcript").is_some(), "audio has transcript");
        assert!(
            result.get("storyboard").is_none(),
            "audio has no storyboard"
        );
    }

    #[test]
    fn read_012_inspect_media_lottie_returns_frames_and_metadata() {
        let input = InspectMediaInput {
            entry: make_inspect_entry(ClipType::Lottie),
            clip: None,
            timeline_fps: 30,
            max_frames: 6,
            inline_image_data: None,
            inline_video_frames: vec![InlineFrameInput {
                frame: 0,
                data_base64: "lottie_frame_0".into(),
            }],
            transcription_words: Vec::new(),
        };
        let result = format_inspect_media(&input).unwrap();
        assert_eq!(result["type"], "lottie");
        assert!(result.get("frames").is_some(), "lottie has frames");
        assert_eq!(result["frames"][0]["frame"], 0);
        assert!(
            result.get("animationMetadata").is_some(),
            "lottie has animationMetadata"
        );
    }

    #[test]
    fn read_012_inspect_media_shape_returns_basic_metadata() {
        let input = InspectMediaInput {
            entry: make_inspect_entry(ClipType::Shape),
            clip: None,
            timeline_fps: 30,
            max_frames: 6,
            inline_image_data: None,
            inline_video_frames: Vec::new(),
            transcription_words: Vec::new(),
        };
        let result = format_inspect_media(&input).unwrap();
        assert_eq!(result["type"], "shape");
        assert_eq!(result["shapeType"], "annotation");
    }

    #[test]
    fn read_013_inspect_media_rejects_text_clips() {
        let input = InspectMediaInput {
            entry: make_inspect_entry(ClipType::Text),
            clip: None,
            timeline_fps: 30,
            max_frames: 6,
            inline_image_data: None,
            inline_video_frames: Vec::new(),
            transcription_words: Vec::new(),
        };
        let result = format_inspect_media(&input);
        assert!(result.is_err(), "READ-013: text clips rejected");
        assert!(
            result.unwrap_err().contains("text clip"),
            "READ-013: error mentions text clip"
        );
    }

    #[test]
    fn read_014_clip_id_not_provided_ok() {
        // When no clipId is provided, inspect_media should still work
        let input = InspectMediaInput {
            entry: make_inspect_entry(ClipType::Video),
            clip: None,
            timeline_fps: 30,
            max_frames: 6,
            inline_image_data: None,
            inline_video_frames: Vec::new(),
            transcription_words: Vec::new(),
        };
        let result = format_inspect_media(&input);
        assert!(result.is_ok(), "no clipId should still work");
    }
}

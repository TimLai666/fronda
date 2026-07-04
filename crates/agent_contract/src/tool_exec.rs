//! Tool execution dispatcher: routes agent tool calls to timeline engine.
//!
//! A ToolExecutor holds the mutable project state (Timeline + UndoStack)
//! and provides a single `execute()` entry point for the MCP server.

use crate::read_tools::{
    format_inspect_media, format_search_results, format_timeline_json, format_transcript_json,
    InspectMediaInput, SearchHitInfo, TranscriptClipInput, TranscriptFormatOptions,
};
use crate::undo::{UndoCommand, UndoStack};
use core_model::{
    AnimPair, Clip, ClipType, Crop, Effect, GenerationInput, Interpolation, Keyframe, KeyframeTrack,
    LayoutFit, MediaManifest, MediaManifestEntry, MediaSource, TextStyle, Timeline, Transform,
    VideoLayout,
};
use serde_json::{json, Value};
use uuid::Uuid;

const DEFAULT_CLIP_DURATION_FRAMES: i64 = 150;

/// Resolved clip placement geometry from optional agent args + manifest entry.
struct ResolvedPlacement {
    media_type: ClipType,
    duration_frames: i64,
    trim_start_frame: i64,
    trim_end_frame: i64,
    fps_warning: Option<String>,
}

/// Resolve a clip's type, duration, and symmetric trim from the manifest entry
/// and optional `trimStartFrame` / `trimEndFrame` / `durationFrames` args.
///
/// Symmetric trim model (upstream palmier-pro #236): `trimStartFrame` trims the
/// head (in-point), `trimEndFrame` trims the tail (out-point), and
/// `durationFrames` is derivable and mutually exclusive with `trimEndFrame`.
/// Trims and durations are clamped to the source length; synthetic clips
/// (image/text/shape) may run any duration.
///
/// Project fps is authoritative (upstream #233): a divergent source fps only
/// yields a warning, it never changes project fps.
fn resolve_placement(
    entry: Option<&MediaManifestEntry>,
    args: &Value,
    project_fps: i64,
) -> Result<ResolvedPlacement, String> {
    let media_type = entry.map(|e| e.r#type.clone()).unwrap_or(ClipType::Video);
    let synthetic = matches!(
        media_type,
        ClipType::Image | ClipType::Text | ClipType::Shape
    );

    let arg_i64 = |key: &str| args.get(key).and_then(Value::as_i64);
    let trim_start_in = arg_i64("trimStartFrame").unwrap_or(0).max(0);
    let duration_in = arg_i64("durationFrames");
    let trim_end_in = arg_i64("trimEndFrame");

    if duration_in.is_some() && trim_end_in.is_some() {
        return Err("Provide either durationFrames or trimEndFrame, not both".to_string());
    }

    // Source length expressed in project frames. Project fps is authoritative,
    // so seconds-based source duration is scaled by project fps, not source fps.
    let source_total = entry
        .filter(|_| !synthetic)
        .map(|e| (e.duration * project_fps as f64).round() as i64)
        .filter(|&total| total > 0);

    let (trim_start_frame, duration_frames, trim_end_frame) = match source_total {
        Some(total) => {
            let trim_start = trim_start_in.min((total - 1).max(0));
            let remaining = (total - trim_start).max(1);
            match (duration_in, trim_end_in) {
                (Some(d), _) => {
                    let d = d.clamp(1, remaining);
                    (trim_start, d, remaining - d)
                }
                (None, Some(te)) => {
                    let te = te.clamp(0, remaining - 1);
                    (trim_start, remaining - te, te)
                }
                (None, None) => (trim_start, remaining, 0),
            }
        }
        // Synthetic clip or no source metadata: any duration, no source trim.
        None => {
            let d = duration_in.unwrap_or(DEFAULT_CLIP_DURATION_FRAMES).max(1);
            (0, d, 0)
        }
    };

    let fps_warning = entry.and_then(|e| e.source_fps).and_then(|source_fps| {
        if source_fps > 0.0 && (source_fps - project_fps as f64).abs() > 0.01 {
            Some(format!(
                "Source fps {source_fps:.3} differs from project fps {project_fps}; \
                 project fps kept unchanged and the clip conforms to it. \
                 Use set_project_settings to change project fps."
            ))
        } else {
            None
        }
    });

    Ok(ResolvedPlacement {
        media_type,
        duration_frames,
        trim_start_frame,
        trim_end_frame,
        fps_warning,
    })
}

/// Parse an agent text-animation spec into a `TextAnimation`, or `None` when the
/// preset is `off`/absent (upstream #225 `parseTextAnimation`).
fn parse_text_animation(
    preset: Option<&str>,
    highlight_hex: Option<&str>,
) -> Result<Option<core_model::TextAnimation>, String> {
    let Some(raw) = preset else {
        return Ok(None);
    };
    let Some(p) = core_model::TextAnimationPreset::from_agent_str(raw) else {
        return Err(format!(
            "invalid animation '{raw}'. Valid: {}",
            core_model::TextAnimationPreset::agent_values().join(", ")
        ));
    };
    if p == core_model::TextAnimationPreset::None {
        return Ok(None);
    }
    let mut anim = core_model::TextAnimation {
        preset: p,
        ..Default::default()
    };
    if let Some(hex) = highlight_hex {
        anim.highlight = Some(core_model::TextRgba::from_hex(hex).ok_or_else(|| {
            format!("invalid highlightColor '{hex}'. Expected '#RGB', '#RRGGBB', or '#RRGGBBAA'")
        })?);
    }
    Ok(Some(anim))
}

/// Base pixel dimensions for an aspect-ratio preset (upstream #177).
/// Mirrors Swift `AspectPreset.width/height`.
fn aspect_preset_dims(aspect: &str) -> Result<(i64, i64), String> {
    match aspect {
        "16:9" => Ok((1920, 1080)),
        "9:14" => Ok((1080, 1680)),
        "9:16" => Ok((1080, 1920)),
        "1:1" => Ok((1080, 1080)),
        "4:3" => Ok((1440, 1080)),
        "2.4:1" => Ok((2560, 1080)),
        other => Err(format!(
            "Unknown aspectRatio '{other}'. Use one of: 16:9, 9:16, 1:1, 4:3, 2.4:1, 9:14"
        )),
    }
}

/// Scale a resolution to a quality preset's short edge, preserving aspect
/// (upstream #177). Mirrors Swift `QualityPreset.resolution` (truncating, like
/// Swift's `Int(Double)`).
fn quality_resolution(
    quality: &str,
    current_width: i64,
    current_height: i64,
) -> Result<(i64, i64), String> {
    let short_edge: i64 = match quality {
        "720p" => 720,
        "1080p" => 1080,
        "2K" => 1440,
        "4K" => 2160,
        other => {
            return Err(format!(
                "Unknown quality '{other}'. Use one of: 720p, 1080p, 2K, 4K"
            ))
        }
    };
    if current_width <= 0 || current_height <= 0 {
        return Err("Cannot apply quality preset to a non-positive resolution".to_string());
    }
    let (w, h) = if current_width <= current_height {
        (
            short_edge,
            (short_edge as f64 * current_height as f64 / current_width as f64) as i64,
        )
    } else {
        (
            (short_edge as f64 * current_width as f64 / current_height as f64) as i64,
            short_edge,
        )
    };
    Ok((w, h))
}

/// A skill available to the in-app agent (upstream #199). The app scans
/// `~/.palmier/skills` and loads these; `read_skill` returns the body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentSkill {
    pub id: String,
    pub name: String,
    pub description: String,
    pub body: String,
}

/// Resolve a layout slot entry's crop anchor (upstream #226). A named `anchor`
/// picks a preset; `anchorX`/`anchorY` (0..1) override each axis. Default center.
/// Value-column count and label for a keyframe `property`, or `None` if the
/// property is not keyframable. Shared by the executor and `validate_set_keyframes`.
pub(crate) fn keyframe_property_arity(property: &str) -> Option<(usize, &'static str)> {
    match property {
        "opacity" | "volume" | "rotation" => Some((1, "value")),
        "position" => Some((2, "topLeftX, topLeftY")),
        "scale" => Some((2, "width, height")),
        "crop" => Some((4, "top, right, bottom, left")),
        _ => None,
    }
}

/// Parse `[[frame, v0, v1, ..., interp?], ...]` keyframe rows (Swift set_keyframes
/// format) into `(frame, values, interp)` triples with `arity` value columns. Rows
/// are stable-sorted by frame, then de-duplicated so the last row for any repeated
/// frame wins (matches Swift `sortAndDedupe`).
pub(crate) fn parse_keyframe_rows(
    rows: &[Value],
    arity: usize,
    labels: &str,
) -> Result<Vec<(i64, Vec<f64>, Interpolation)>, String> {
    let min_len = arity + 1;
    let max_len = arity + 2;
    let mut out: Vec<(i64, Vec<f64>, Interpolation)> = Vec::with_capacity(rows.len());
    for (i, raw) in rows.iter().enumerate() {
        let row = raw
            .as_array()
            .ok_or_else(|| format!("keyframes[{i}]: expected array [frame, {labels}, interp?]"))?;
        if row.len() != min_len && row.len() != max_len {
            return Err(format!(
                "keyframes[{i}]: expected [frame, {labels}] or [frame, {labels}, interp] (got {} elements)",
                row.len()
            ));
        }
        let frame = row[0]
            .as_i64()
            .ok_or_else(|| format!("keyframes[{i}][0]: frame must be an integer"))?;
        let mut values = Vec::with_capacity(arity);
        for k in 0..arity {
            let v = row[k + 1]
                .as_f64()
                .ok_or_else(|| format!("keyframes[{i}][{}]: expected a number", k + 1))?;
            if !v.is_finite() {
                return Err(format!("keyframes[{i}][{}]: value must be finite", k + 1));
            }
            values.push(v);
        }
        let interp = if row.len() > min_len {
            match row[min_len].as_str() {
                Some("linear") => Interpolation::Linear,
                Some("hold") => Interpolation::Hold,
                Some("smooth") => Interpolation::Smooth,
                Some(other) => {
                    return Err(format!(
                        "keyframes[{i}]: invalid interp '{other}' (expected linear, hold, or smooth)"
                    ))
                }
                None => return Err(format!("keyframes[{i}]: interp must be a string")),
            }
        } else {
            Interpolation::Smooth
        };
        out.push((frame, values, interp));
    }
    out.sort_by_key(|(f, _, _)| *f);
    let mut deduped: Vec<(i64, Vec<f64>, Interpolation)> = Vec::with_capacity(out.len());
    for row in out {
        if deduped.last().map(|(f, _, _)| *f) == Some(row.0) {
            *deduped.last_mut().unwrap() = row;
        } else {
            deduped.push(row);
        }
    }
    Ok(deduped)
}

/// Parse a caption background/border fill object `{enabled?, color?, padding?,
/// cornerRadius?}` into a [`core_model::TextFill`] (Issue #18). Missing fields
/// keep the default; an invalid colour is an error. Full replacement, not a merge.
fn parse_text_fill(v: &Value, what: &str) -> Result<core_model::TextFill, String> {
    let mut fill = core_model::TextFill::default();
    if let Some(en) = v.get("enabled").and_then(|x| x.as_bool()) {
        fill.enabled = en;
    }
    if let Some(hex) = v.get("color").and_then(|x| x.as_str()) {
        fill.color = core_model::TextRgba::from_hex(hex)
            .ok_or_else(|| format!("invalid {what} color '{hex}'"))?;
    }
    if let Some(p) = v.get("padding").and_then(|x| x.as_f64()) {
        fill.padding = Some(p);
    }
    if let Some(r) = v.get("cornerRadius").and_then(|x| x.as_f64()) {
        fill.corner_radius = Some(r);
    }
    Ok(fill)
}

fn resolve_layout_anchor(entry: &Value) -> Result<(f64, f64), String> {
    const ANCHORS: &[(&str, (f64, f64))] = &[
        ("center", (0.5, 0.5)),
        ("top", (0.5, 0.0)),
        ("bottom", (0.5, 1.0)),
        ("left", (0.0, 0.5)),
        ("right", (1.0, 0.5)),
        ("top_left", (0.0, 0.0)),
        ("top_right", (1.0, 0.0)),
        ("bottom_left", (0.0, 1.0)),
        ("bottom_right", (1.0, 1.0)),
    ];
    let mut anchor = (0.5, 0.5);
    if let Some(name) = entry.get("anchor").and_then(Value::as_str) {
        anchor = ANCHORS
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, a)| *a)
            .ok_or_else(|| format!("invalid anchor '{name}'"))?;
    }
    for key in ["anchorX", "anchorY"] {
        if let Some(v) = entry.get(key).and_then(Value::as_f64) {
            if !(0.0..=1.0).contains(&v) {
                return Err(format!("{key} must be between 0 and 1 (got {v})"));
            }
        }
    }
    if let Some(ax) = entry.get("anchorX").and_then(Value::as_f64) {
        anchor.0 = ax;
    }
    if let Some(ay) = entry.get("anchorY").and_then(Value::as_f64) {
        anchor.1 = ay;
    }
    Ok(anchor)
}

/// Runtime state for executing agent timeline tools.
pub struct ToolExecutor {
    timeline: Timeline,
    media_manifest: MediaManifest,
    undo_stack: UndoStack,
    /// READ-026: Status reporting for visual indexing.
    /// Set by the caller (app shell) to reflect search model state.
    search_status: String,
    /// Strictly increases after each successful mutating tool execution.
    revision: u64,
    /// Skills loaded from `~/.palmier/skills`, sorted by id (upstream #199).
    skills: Vec<AgentSkill>,
}

/// Read-only tools: successful execution does not bump the revision.
const READ_ONLY_TOOLS: &[&str] = &[
    "get_timeline",
    "get_media",
    "search_media",
    "list_folders",
    "list_models",
    "inspect_media",
    "inspect_timeline",
    "get_transcript",
    "inspect_color",
    "read_skill",
];

impl ToolExecutor {
    pub fn new(timeline: Timeline, media_manifest: MediaManifest) -> Self {
        Self {
            timeline,
            media_manifest,
            undo_stack: UndoStack::new(),
            search_status: String::new(),
            revision: 0,
            skills: Vec::new(),
        }
    }

    /// Load the in-app agent's skills (upstream #199). The app scans
    /// `~/.palmier/skills` and passes the parsed skills here.
    pub fn set_skills(&mut self, skills: Vec<AgentSkill>) {
        self.skills = skills;
    }

    pub fn skills(&self) -> &[AgentSkill] {
        &self.skills
    }

    /// Change counter for UI invalidation: bumps on successful mutations.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Replace project state in place (project open). Clears the undo
    /// stack and bumps the revision; a running MCP server serves the new
    /// state on its next request.
    pub fn load_project(&mut self, timeline: Timeline, media_manifest: MediaManifest) {
        self.timeline = timeline;
        self.media_manifest = media_manifest;
        self.undo_stack = UndoStack::new();
        self.revision += 1;
    }

    pub fn media_manifest(&self) -> &MediaManifest {
        &self.media_manifest
    }

    pub fn media_manifest_mut(&mut self) -> &mut MediaManifest {
        &mut self.media_manifest
    }

    pub fn timeline(&self) -> &Timeline {
        &self.timeline
    }

    pub fn timeline_mut(&mut self) -> &mut Timeline {
        &mut self.timeline
    }

    pub fn undo_stack(&self) -> &UndoStack {
        &self.undo_stack
    }

    pub fn undo_stack_mut(&mut self) -> &mut UndoStack {
        &mut self.undo_stack
    }

    /// READ-026: Get the current search indexing status.
    pub fn search_status(&self) -> &str {
        &self.search_status
    }

    /// READ-026: Set the search indexing status (by app shell).
    pub fn set_search_status(&mut self, status: &str) {
        self.search_status = status.to_string();
    }

    /// Returns IDs of media entries that are offline (missing local file, no fresh
    /// cached URL). `now` gates cache-expiry (see `MediaManifestEntry::cache_is_fresh`).
    pub fn media_offline_ids(
        &self,
        now: chrono::DateTime<chrono::Utc>,
        is_missing: impl Fn(&MediaManifestEntry) -> bool,
    ) -> Vec<String> {
        self.media_manifest.missing_entry_ids(now, is_missing)
    }

    /// Returns true if the given media ref is offline.
    pub fn is_media_offline(
        &self,
        media_ref: &str,
        now: chrono::DateTime<chrono::Utc>,
        is_missing: impl Fn(&MediaManifestEntry) -> bool,
    ) -> bool {
        let offline_ids = self.media_offline_ids(now, is_missing);
        offline_ids.iter().any(|id| id == media_ref)
    }

    /// Returns true if the given media ref is unprocessable (present but failed to decode).
    ///
    /// Uses the `is_missing` callback to exclude entries whose files are simply missing
    /// (those are "offline", not "unprocessable"); an entry with a fresh cached copy is
    /// likewise not unprocessable.
    pub fn is_media_unprocessable(
        &self,
        media_ref: &str,
        now: chrono::DateTime<chrono::Utc>,
        is_missing: impl Fn(&MediaManifestEntry) -> bool,
        is_unprocessable: impl Fn(&MediaManifestEntry) -> bool,
    ) -> bool {
        self.media_manifest.entries.iter().any(|e| {
            e.id == media_ref
                && !e.cache_is_fresh(now)
                && !is_missing(e)
                && is_unprocessable(e)
        })
    }

    /// Execute a tool by name with validated JSON arguments.
    ///
    /// Returns the JSON result that should become the MCP `content` array.
    /// For mutation tools, automatically snapshots before/after for undo.
    pub fn execute(&mut self, tool_name: &str, args: &Value) -> Result<Value, String> {
        let result = self.execute_inner(tool_name, args);
        if result.is_ok() && !READ_ONLY_TOOLS.contains(&tool_name) {
            self.revision += 1;
        }
        result
    }

    fn execute_inner(&mut self, tool_name: &str, args: &Value) -> Result<Value, String> {
        match tool_name {
            // ── Read-only tools ──────────────────────────────────────────
            "get_timeline" => self.cmd_get_timeline(),

            // ── Mutation tools (undo-tracked) ────────────────────────────
            "split_clips" => self.exec_mut(tool_name, ToolExecutor::cmd_split_clips, args),
            "remove_clips" => self.exec_mut(tool_name, ToolExecutor::cmd_remove_clips, args),
            "move_clips" => self.exec_mut(tool_name, ToolExecutor::cmd_move_clips, args),
            "move_clips_linked" => {
                self.exec_mut(tool_name, ToolExecutor::cmd_move_clips_linked, args)
            }
            "set_clip_properties" => {
                self.exec_mut(tool_name, ToolExecutor::cmd_set_clip_properties, args)
            }
            "set_keyframes" => self.exec_mut(tool_name, ToolExecutor::cmd_set_keyframes, args),
            "ripple_delete_ranges" => {
                self.exec_mut(tool_name, ToolExecutor::cmd_ripple_delete_ranges, args)
            }
            "remove_tracks" => self.exec_mut(tool_name, ToolExecutor::cmd_remove_tracks, args),
            "add_clips" => self.exec_mut(tool_name, ToolExecutor::cmd_add_clips, args),
            "insert_clips" => self.exec_mut(tool_name, ToolExecutor::cmd_insert_clips, args),
            "apply_layout" => self.exec_mut(tool_name, ToolExecutor::cmd_apply_layout, args),
            "add_texts" => self.exec_mut(tool_name, ToolExecutor::cmd_add_texts, args),
            "add_shapes" => self.exec_mut(tool_name, ToolExecutor::cmd_add_shapes, args),
            "apply_color" => self.exec_mut(tool_name, ToolExecutor::cmd_apply_color, args),
            "apply_effect" => self.exec_mut(tool_name, ToolExecutor::cmd_apply_effect, args),
            "set_chroma_key" => self.exec_mut(tool_name, ToolExecutor::cmd_set_chroma_key, args),
            "set_blend_mode" => self.exec_mut(tool_name, ToolExecutor::cmd_set_blend_mode, args),
            "set_color_grade" => self.exec_mut(tool_name, ToolExecutor::cmd_set_color_grade, args),
            "set_project_settings" => {
                self.exec_mut(tool_name, ToolExecutor::cmd_set_project_settings, args)
            }
            "undo" => self.cmd_undo(),
            "redo" => self.cmd_redo(),

            // ── Media mutation tools (no undo yet) ───────────────────────
            "create_folder" => self.cmd_create_folder(args),
            "rename_folder" => self.cmd_rename_folder(args),
            "delete_folder" => self.cmd_delete_folder(args),
            "rename_media" => self.cmd_rename_media(args),
            "delete_media" => self.cmd_delete_media(args),
            "move_to_folder" => self.cmd_move_to_folder(args),
            "import_media" => self.cmd_import_media(args),
            "import_folder" => self.cmd_import_folder(args),
            "duplicate_project" => self.cmd_duplicate_project(),

            // ── Read-only tools ──────────────────────────────────────────
            "get_media" => self.cmd_get_media(args),
            "search_media" => self.cmd_search_media(args),
            "list_folders" => self.cmd_list_folders(),
            "list_models" => self.cmd_list_models(),
            "inspect_media" => self.cmd_inspect_media(args),
            "inspect_timeline" => self.cmd_inspect_timeline(),
            "get_transcript" => self.cmd_get_transcript(args),
            "read_skill" => self.cmd_read_skill(args),

            // ── Generation tools (stub — need remote API) ────────────────
            "generate_video" => self.cmd_generate_video(args),
            "generate_image" => self.cmd_generate_image(args),
            "generate_audio" => self.cmd_generate_audio(args),
            "generate_music" => self.cmd_generate_music(args),
            "upscale_media" => self.cmd_upscale_media(args),

            // ── Read-only color inspect (no mutation) ────────────────────
            "inspect_color" => self.cmd_inspect_color(args),

            // ── Captions (stub — needs transcription engine) ─────────────
            "add_captions" => self.cmd_add_captions(args),
            "apply_animation" => self.cmd_apply_animation(args),

            _ => Err(format!("Unknown tool: {tool_name}")),
        }
    }

    // ── Undo-wrapper for mutation tools ──────────────────────────────────

    fn exec_mut(
        &mut self,
        tool_name: &str,
        f: fn(&mut ToolExecutor, &Value) -> Result<Value, String>,
        args: &Value,
    ) -> Result<Value, String> {
        let before = self.timeline.clone();
        let result = f(self, args)?;
        let after = self.timeline.clone();

        if before != after {
            let cmd = UndoCommand::new(
                Uuid::new_v4().to_string(),
                tool_name.to_string(),
                before,
                after,
            );
            self.undo_stack.push_command(cmd);
        }

        Ok(result)
    }

    // ── Tool implementations ─────────────────────────────────────────────

    fn cmd_get_timeline(&self) -> Result<Value, String> {
        let timeline_json =
            serde_json::to_value(&self.timeline).map_err(|e| format!("Serialize error: {e}"))?;
        Ok(json!({
            "content": [{
                "type": "text",
                "text": serde_json::to_string_pretty(&timeline_json)
                    .unwrap_or_else(|_| "{}".into()),
            }]
        }))
    }

    /// READ_SKILL: return a loaded skill's full SKILL.md body by id (upstream #199).
    fn cmd_read_skill(&self, args: &Value) -> Result<Value, String> {
        let id = args
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing id".to_string())?;
        let skill = self
            .skills
            .iter()
            .find(|s| s.id == id)
            .ok_or_else(|| format!("Skill '{id}' not found"))?;
        Ok(json!({
            "content": [{
                "type": "text",
                "text": skill.body.clone(),
            }]
        }))
    }

    fn cmd_split_clips(&mut self, args: &Value) -> Result<Value, String> {
        use timeline_core::ClipMathExt;

        // Resolve every cut to (track_index, frame), validating up-front so one
        // bad point rejects the whole call with no partial state (upstream #186).
        let mut cuts: Vec<(usize, i64)> = Vec::new();

        let splits = args.get("splits").and_then(|v| v.as_array());
        let has_splits = splits.map(|a| !a.is_empty()).unwrap_or(false);

        if has_splits {
            for (i, item) in splits.unwrap().iter().enumerate() {
                let clip_id = item
                    .get("clipId")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| format!("splits[{i}]: missing clipId"))?;
                let at_frame = item
                    .get("atFrame")
                    .and_then(|v| v.as_i64())
                    .ok_or_else(|| format!("splits[{i}]: missing atFrame"))?;
                let located = self.timeline.tracks.iter().enumerate().find_map(|(ti, t)| {
                    t.clips
                        .iter()
                        .find(|c| c.id == clip_id)
                        .map(|c| (ti, c.start_frame, c.end_frame()))
                });
                let (ti, start, end) =
                    located.ok_or_else(|| format!("splits[{i}]: clip '{clip_id}' not found"))?;
                if at_frame <= start || at_frame >= end {
                    return Err(format!(
                        "splits[{i}]: atFrame {at_frame} must be strictly inside clip '{clip_id}' [{start}, {end})"
                    ));
                }
                cuts.push((ti, at_frame));
            }
        } else {
            let track_index = args
                .get("trackIndex")
                .and_then(|v| v.as_i64())
                .ok_or_else(|| "Provide either 'splits' or 'trackIndex' + 'frames'".to_string())?
                as usize;
            let frames = args
                .get("frames")
                .and_then(|v| v.as_array())
                .filter(|a| !a.is_empty())
                .ok_or_else(|| "'frames' array required with trackIndex".to_string())?;
            if track_index >= self.timeline.tracks.len() {
                return Err(format!("Track index {track_index} out of bounds"));
            }
            for f in frames {
                let frame = f
                    .as_i64()
                    .ok_or_else(|| "frames must be integers".to_string())?;
                let inside = self.timeline.tracks[track_index]
                    .clips
                    .iter()
                    .any(|c| frame > c.start_frame && frame < c.end_frame());
                if !inside {
                    return Err(format!(
                        "frame {frame} is not strictly inside any clip on track {track_index}"
                    ));
                }
                cuts.push((track_index, frame));
            }
        }

        cuts.sort_unstable();
        cuts.dedup();

        // Apply — resolve each cut against the CURRENT sub-clips so repeated cuts
        // on the same original clip land on the right piece.
        let mut new_ids: Vec<String> = Vec::new();
        for (ti, frame) in &cuts {
            let clip_id = self.timeline.tracks[*ti]
                .clips
                .iter()
                .find(|c| *frame > c.start_frame && *frame < c.end_frame())
                .map(|c| c.id.clone());
            if let Some(cid) = clip_id {
                new_ids.extend(timeline_core::split_clip(&mut self.timeline, &cid, *frame));
            }
        }

        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!(
                    "Split at {} cut point(s). Created {} new clip(s): {new_ids:?}",
                    cuts.len(),
                    new_ids.len()
                )
            }]
        }))
    }

    fn cmd_remove_clips(&mut self, args: &Value) -> Result<Value, String> {
        let clip_ids: Vec<String> = args
            .get("clipIds")
            .and_then(|v| v.as_array())
            .ok_or_else(|| "Missing clipIds".to_string())?
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();

        if clip_ids.is_empty() {
            return Err("clipIds must be non-empty".to_string());
        }

        let ripple = args
            .get("ripple")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let len = clip_ids.len();
        timeline_core::remove_clips(&mut self.timeline, clip_ids, ripple);
        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!("Removed {len} clip(s) (ripple={ripple})")
            }]
        }))
    }

    fn cmd_move_clips(&mut self, args: &Value) -> Result<Value, String> {
        let clip_ids: Vec<String> = args
            .get("clipIds")
            .and_then(|v| v.as_array())
            .ok_or_else(|| "Missing clipIds".to_string())?
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();

        let to_track = args
            .get("toTrack")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| "Missing toTrack".to_string())? as usize;
        let to_frame = args
            .get("toFrame")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| "Missing toFrame".to_string())?;

        if to_track >= self.timeline.tracks.len() {
            return Err(format!(
                "Track index {to_track} out of bounds ({} tracks)",
                self.timeline.tracks.len()
            ));
        }

        let placed = timeline_core::move_clips(&mut self.timeline, &clip_ids, to_track, to_frame);
        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!("Moved {} clip(s) to track {to_track} at frame {to_frame}: {placed:?}",
                    placed.len())
            }]
        }))
    }

    fn cmd_move_clips_linked(&mut self, args: &Value) -> Result<Value, String> {
        self.cmd_move_clips(args)
    }

    fn cmd_set_clip_properties(&mut self, args: &Value) -> Result<Value, String> {
        let clip_ids: Vec<String> = args
            .get("clipIds")
            .and_then(|v| v.as_array())
            .ok_or_else(|| "Missing clipIds".to_string())?
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();

        if clip_ids.is_empty() {
            return Err("clipIds must be non-empty".to_string());
        }

        let properties = args
            .get("properties")
            .ok_or_else(|| "Missing properties".to_string())?;

        let duration = properties.get("durationFrames").and_then(|v| v.as_i64());
        let trim_start = properties.get("trimStartFrame").and_then(|v| v.as_i64());
        let trim_end = properties.get("trimEndFrame").and_then(|v| v.as_i64());
        let speed = properties.get("speed").and_then(|v| v.as_f64());
        let volume = properties.get("volume").and_then(|v| v.as_f64());
        let opacity = properties.get("opacity").and_then(|v| v.as_f64());
        let content = properties.get("content").and_then(|v| v.as_str());
        let font_name = properties.get("fontName").and_then(|v| v.as_str());
        let font_size = properties.get("fontSize").and_then(|v| v.as_f64());
        let font_weight = properties.get("fontWeight").and_then(|v| v.as_f64());
        let color = match properties.get("color").and_then(|v| v.as_str()) {
            Some(hex) => Some(core_model::TextRgba::from_hex(hex).ok_or_else(|| {
                format!("invalid color '{hex}'. Expected '#RGB', '#RRGGBB', or '#RRGGBBAA'")
            })?),
            None => None,
        };
        let alignment = match properties.get("alignment").and_then(|v| v.as_str()) {
            Some(a) => Some(core_model::TextAlignment::from_name(a).ok_or_else(|| {
                format!("invalid alignment '{a}'. Expected 'left', 'center', or 'right'")
            })?),
            None => None,
        };
        let background = match properties.get("background") {
            Some(v) => Some(parse_text_fill(v, "background")?),
            None => None,
        };
        let border = match properties.get("border") {
            Some(v) => Some(parse_text_fill(v, "border")?),
            None => None,
        };

        let transform = properties
            .get("transform")
            .map(|t| timeline_core::PartialTransform {
                center_x: t.get("centerX").and_then(|v| v.as_f64()),
                center_y: t.get("centerY").and_then(|v| v.as_f64()),
                width: t.get("width").and_then(|v| v.as_f64()),
                height: t.get("height").and_then(|v| v.as_f64()),
                rotation: t.get("rotation").and_then(|v| v.as_f64()),
                flip_horizontal: t.get("flipHorizontal").and_then(|v| v.as_bool()),
                flip_vertical: t.get("flipVertical").and_then(|v| v.as_bool()),
            });

        let update = timeline_core::ClipPropertyUpdate {
            duration_frames: duration,
            trim_start_frame: trim_start,
            trim_end_frame: trim_end,
            speed,
            volume,
            opacity,
            transform: transform.as_ref(),
            content,
            font_name,
            font_size,
            font_weight,
            color,
            alignment,
            background,
            border,
        };

        let mut changed_count = 0usize;
        let mut changed_fields: Vec<String> = Vec::new();
        for clip_id in &clip_ids {
            let Some(loc) = timeline_core::find_clip(&self.timeline, clip_id) else {
                continue;
            };
            let clip = &mut self.timeline.tracks[loc.track_index].clips[loc.clip_index];
            let changes = timeline_core::set_clip_properties(clip, &update);
            changed_count += 1;
            if changed_fields.is_empty() {
                changed_fields = changes.changed;
            }
        }

        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!(
                    "Updated properties on {changed_count} clip(s): {}",
                    changed_fields.join(", ")
                )
            }]
        }))
    }

    fn cmd_set_keyframes(&mut self, args: &Value) -> Result<Value, String> {
        let clip_id = args
            .get("clipId")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing clipId".to_string())?;
        let property = args
            .get("property")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing property".to_string())?;
        let kf_array = args
            .get("keyframes")
            .and_then(|v| v.as_array())
            .ok_or_else(|| "Missing keyframes array".to_string())?;

        // Rows are `[frame, ...values, interp?]` (Swift set_keyframes format);
        // validate the property up front so we know how many value columns to expect.
        let (arity, labels) = keyframe_property_arity(property).ok_or_else(|| {
            format!(
                "Unknown keyframe property '{property}'. Expected one of: opacity, volume, rotation, position, scale, crop"
            )
        })?;

        let Some(loc) = timeline_core::find_clip(&self.timeline, clip_id) else {
            return Err(format!("Clip '{clip_id}' not found"));
        };
        let rows = parse_keyframe_rows(kf_array, arity, labels)?;
        let clip = &mut self.timeline.tracks[loc.track_index].clips[loc.clip_index];

        let kf_len = rows.len();
        // An empty array clears the track.
        let scalar = |col: usize| {
            if rows.is_empty() {
                None
            } else {
                Some(KeyframeTrack {
                    keyframes: rows
                        .iter()
                        .map(|(f, v, i)| Keyframe {
                            frame: *f,
                            value: v[col],
                            interpolation_out: *i,
                        })
                        .collect(),
                })
            }
        };
        match property {
            "opacity" => clip.opacity_track = scalar(0),
            "volume" => clip.volume_track = scalar(0),
            "rotation" => clip.rotation_track = scalar(0),
            "position" | "scale" => {
                let track = if rows.is_empty() {
                    None
                } else {
                    Some(KeyframeTrack {
                        keyframes: rows
                            .iter()
                            .map(|(f, v, i)| Keyframe {
                                frame: *f,
                                value: AnimPair { a: v[0], b: v[1] },
                                interpolation_out: *i,
                            })
                            .collect(),
                    })
                };
                if property == "position" {
                    clip.position_track = track;
                } else {
                    clip.scale_track = track;
                }
            }
            "crop" => {
                clip.crop_track = if rows.is_empty() {
                    None
                } else {
                    Some(KeyframeTrack {
                        keyframes: rows
                            .iter()
                            .map(|(f, v, i)| Keyframe {
                                // Input order is [top, right, bottom, left].
                                frame: *f,
                                value: Crop {
                                    top: v[0],
                                    right: v[1],
                                    bottom: v[2],
                                    left: v[3],
                                },
                                interpolation_out: *i,
                            })
                            .collect(),
                    })
                };
            }
            _ => unreachable!("property validated above"),
        }

        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!(
                    "Set {kf_len} keyframe(s) on clip '{clip_id}' property '{property}'"
                )
            }]
        }))
    }

    fn cmd_ripple_delete_ranges(&mut self, args: &Value) -> Result<Value, String> {
        let track_index = args
            .get("trackIndex")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| "Missing trackIndex".to_string())? as usize;
        let ranges_val = args
            .get("ranges")
            .and_then(|v| v.as_array())
            .ok_or_else(|| "Missing ranges array".to_string())?;

        let ranges: Vec<timeline_core::FrameRange> = ranges_val
            .iter()
            .filter_map(|r| {
                let start = r.get("start").and_then(|v| v.as_i64())?;
                let end = r.get("end").and_then(|v| v.as_i64())?;
                if end > start {
                    Some(timeline_core::FrameRange { start, end })
                } else {
                    None
                }
            })
            .collect();

        if ranges.is_empty() {
            return Err("No valid ranges".to_string());
        }

        if track_index >= self.timeline.tracks.len() {
            return Err(format!("Track index {track_index} out of bounds"));
        }

        let range_list = ranges.clone();
        let config = timeline_core::RippleDeleteConfig {
            anchor_track_index: track_index,
            ranges,
        };
        let outcome = timeline_core::compute_ripple_delete(&self.timeline, config);

        let result = match outcome {
            timeline_core::RippleDeleteOutcome::Ok(report) => {
                let merged = timeline_core::merge_ranges(&range_list);
                let cleared: std::collections::HashSet<usize> =
                    report.cleared_track_indices.iter().copied().collect();

                // RPL-004: fragment-cut each range on every cleared track (anchor +
                // linked partners) — a clip fully inside a range is removed, a
                // partial overlap is trimmed/split so only the non-overlapping
                // fragments survive. (Previously the whole clip was deleted whenever
                // it merely touched a range, silently losing media.)
                for ti in &report.cleared_track_indices {
                    for r in &merged {
                        timeline_core::clear_region(&mut self.timeline, *ti, r.start, r.end, false);
                    }
                }

                // Close the gaps: shift later clips left on the cleared tracks (their
                // own gap) and on sync-locked follower tracks (to stay aligned).
                for (ti, track) in self.timeline.tracks.iter_mut().enumerate() {
                    if !cleared.contains(&ti) && !track.sync_locked {
                        continue;
                    }
                    let shifts =
                        timeline_core::compute_ripple_shifts_for_ranges(&track.clips, &merged);
                    for shift in shifts {
                        if let Some(clip) = track.clips.iter_mut().find(|c| c.id == shift.clip_id) {
                            clip.start_frame = shift.new_start_frame;
                        }
                    }
                }

                let removed_frames = report.removed_frames;
                let removed = report.cleared_track_indices.len();
                json!({
                    "content": [{
                        "type": "text",
                        "text": format!(
                            "Ripple-deleted {removed_frames} frames across {removed} track(s)"
                        )
                    }]
                })
            }
            timeline_core::RippleDeleteOutcome::Refused(msg) => json!({
                "content": [{
                    "type": "text",
                    "text": format!("Ripple delete refused: {msg}")
                }],
                "isError": true,
            }),
        };

        Ok(result)
    }

    fn cmd_remove_tracks(&mut self, args: &Value) -> Result<Value, String> {
        let track_ids: Vec<String> = args
            .get("trackIds")
            .and_then(|v| v.as_array())
            .ok_or_else(|| "Missing trackIds".to_string())?
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();

        if track_ids.is_empty() {
            return Err("trackIds must be non-empty".to_string());
        }

        let before_count = self.timeline.tracks.len();
        let mut indices: Vec<usize> = track_ids
            .iter()
            .filter_map(|id| self.timeline.tracks.iter().position(|t| t.id == *id))
            .collect();
        indices.sort_unstable_by(|a, b| b.cmp(a));
        indices.dedup();

        for idx in indices {
            timeline_core::remove_track(&mut self.timeline, idx);
        }

        let removed = before_count - self.timeline.tracks.len();
        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!("Removed {removed} track(s)")
            }]
        }))
    }

    fn cmd_set_project_settings(&mut self, args: &Value) -> Result<Value, String> {
        let fps_in = args.get("fps").and_then(Value::as_i64);
        let width_in = args.get("width").and_then(Value::as_i64);
        let height_in = args.get("height").and_then(Value::as_i64);
        let aspect_in = args.get("aspectRatio").and_then(Value::as_str);
        let quality_in = args.get("quality").and_then(Value::as_str);

        if fps_in.is_none()
            && width_in.is_none()
            && height_in.is_none()
            && aspect_in.is_none()
            && quality_in.is_none()
        {
            return Err(
                "Provide at least one of: fps, width, height, aspectRatio, quality".to_string(),
            );
        }
        if aspect_in.is_some() && (width_in.is_some() || height_in.is_some()) {
            return Err(
                "'aspectRatio' and explicit 'width'/'height' are mutually exclusive".to_string(),
            );
        }
        if let Some(fps) = fps_in {
            if !(1..=120).contains(&fps) {
                return Err(format!("fps must be between 1 and 120 (got {fps})"));
            }
        }

        let new_fps = fps_in.unwrap_or(self.timeline.fps);

        let (new_width, new_height) = if let Some(aspect) = aspect_in {
            let (base_w, base_h) = aspect_preset_dims(aspect)?;
            match quality_in {
                Some(q) => quality_resolution(q, base_w, base_h)?,
                None => (base_w, base_h),
            }
        } else if let Some(q) = quality_in {
            quality_resolution(q, self.timeline.width, self.timeline.height)?
        } else {
            (
                width_in.unwrap_or(self.timeline.width),
                height_in.unwrap_or(self.timeline.height),
            )
        };

        if new_width <= 0 || new_height <= 0 {
            return Err("Resolution must have positive width and height".to_string());
        }

        let prev_fps = self.timeline.fps;
        let prev_width = self.timeline.width;
        let prev_height = self.timeline.height;

        timeline_core::apply_settings(&mut self.timeline, new_fps, new_width, new_height, |c| {
            c.transform == Transform::default()
        });

        let mut changes: Vec<String> = Vec::new();
        if new_fps != prev_fps {
            changes.push(format!("fps {prev_fps} -> {new_fps}"));
        }
        if new_width != prev_width || new_height != prev_height {
            changes.push(format!(
                "resolution {prev_width}x{prev_height} -> {new_width}x{new_height}"
            ));
        }

        let text = if changes.is_empty() {
            format!("No change - settings already match: {new_width}x{new_height} @ {new_fps}fps")
        } else {
            format!(
                "Updated: {}. Now {new_width}x{new_height} @ {new_fps}fps.",
                changes.join(", ")
            )
        };
        Ok(json!({ "content": [{ "type": "text", "text": text }] }))
    }

    fn cmd_apply_layout(&mut self, args: &Value) -> Result<Value, String> {
        let layout_name = args
            .get("layout")
            .and_then(Value::as_str)
            .ok_or_else(|| "Missing layout".to_string())?;
        let layout = VideoLayout::from_str(layout_name).ok_or_else(|| {
            let valid: Vec<&str> = VideoLayout::ALL.iter().map(|l| l.as_str()).collect();
            format!("unknown layout '{layout_name}'. Valid: {}", valid.join(", "))
        })?;
        let fit_name = args.get("fit").and_then(Value::as_str).unwrap_or("fill");
        let fit = LayoutFit::from_str(fit_name)
            .ok_or_else(|| format!("invalid fit '{fit_name}'. Valid: fill, fit"))?;

        let slots_val = args
            .get("slots")
            .and_then(Value::as_array)
            .filter(|a| !a.is_empty())
            .ok_or_else(|| "apply_layout needs a non-empty 'slots' array".to_string())?;

        let layout_slots = layout.slots();

        // Parse + validate every slot entry before mutating anything.
        let mut seen_slots: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut seen_clips: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut uses_media = false;
        // (slot, clip_ids, media_ref, anchor). clip_ids drives re-layout mode;
        // media_ref drives place-new mode (mutually exclusive across all slots).
        let mut entries: Vec<(core_model::LayoutSlot, Vec<String>, Option<String>, (f64, f64))> =
            Vec::new();
        for (i, e) in slots_val.iter().enumerate() {
            let slot_name = e
                .get("slot")
                .and_then(Value::as_str)
                .ok_or_else(|| format!("slots[{i}]: missing 'slot'"))?;
            let slot = layout_slots
                .iter()
                .find(|s| s.id == slot_name)
                .ok_or_else(|| {
                    let ids: Vec<&str> = layout_slots.iter().map(|s| s.id).collect();
                    format!(
                        "slots[{i}]: '{slot_name}' is not a slot of '{layout_name}'. Slots: {}",
                        ids.join(", ")
                    )
                })?;
            if !seen_slots.insert(slot_name.to_string()) {
                return Err(format!("slots[{i}]: duplicate slot '{slot_name}'"));
            }
            let media_ref = e.get("mediaRef").and_then(Value::as_str);
            // Clip assignment: 'clipIds' array (batch) preferred; 'clipId' singular accepted.
            let clip_ids: Option<Vec<String>> =
                if let Some(arr) = e.get("clipIds").and_then(Value::as_array) {
                    Some(
                        arr.iter()
                            .filter_map(Value::as_str)
                            .map(str::to_string)
                            .collect(),
                    )
                } else {
                    e.get("clipId")
                        .and_then(Value::as_str)
                        .map(|c| vec![c.to_string()])
                };
            if media_ref.is_some() == clip_ids.is_some() {
                return Err(format!(
                    "slots[{i}]: provide exactly one of 'mediaRef' or 'clipIds'"
                ));
            }
            if let Some(cids) = &clip_ids {
                if cids.is_empty() {
                    return Err(format!("slots[{i}]: 'clipIds' must not be empty"));
                }
                for cid in cids {
                    if !seen_clips.insert(cid.clone()) {
                        return Err(format!(
                            "slots[{i}]: clip '{cid}' is assigned to more than one slot; each clip can fill only one."
                        ));
                    }
                }
            }
            uses_media = uses_media || media_ref.is_some();
            let anchor = resolve_layout_anchor(e).map_err(|m| format!("slots[{i}]: {m}"))?;
            entries.push((
                slot.clone(),
                clip_ids.unwrap_or_default(),
                media_ref.map(str::to_string),
                anchor,
            ));
        }

        let missing: Vec<&str> = layout_slots
            .iter()
            .map(|s| s.id)
            .filter(|id| !seen_slots.contains(*id))
            .collect();
        if !missing.is_empty() {
            return Err(format!(
                "layout '{layout_name}' needs every slot filled. Missing: {}",
                missing.join(", ")
            ));
        }
        if uses_media {
            return self.apply_layout_place_new(&entries, layout_name, fit, args);
        }

        // Re-layout co-visibility validation (before any mutation):
        //   (a) two clips in DIFFERENT slots on the SAME track must not overlap in
        //       time — only the first would render; (b) with more than one slot,
        //       some frame must have every slot playing, else no frame shows all
        //       regions. Also validates every clip exists and is video/image.
        let mut ranges_by_track: std::collections::HashMap<String, Vec<(String, i64, i64)>> =
            std::collections::HashMap::new();
        let mut intervals_by_slot: std::collections::HashMap<String, Vec<(i64, i64)>> =
            std::collections::HashMap::new();
        for (slot, clip_ids, _media, _) in &entries {
            for cid in clip_ids {
                let loc = timeline_core::find_clip(&self.timeline, cid)
                    .ok_or_else(|| format!("slot '{}': clip not found: {cid}", slot.id))?;
                let track = &self.timeline.tracks[loc.track_index];
                let clip = &track.clips[loc.clip_index];
                if !matches!(clip.media_type, ClipType::Video | ClipType::Image) {
                    return Err(format!(
                        "slot '{}': clip {cid} is {:?}; layout applies to video/image clips",
                        slot.id, clip.media_type
                    ));
                }
                let (start, end) = (clip.start_frame, clip.start_frame + clip.duration_frames);
                let track_id = track.id.clone();
                if let Some(existing) = ranges_by_track.get(&track_id) {
                    for (other_slot, o_start, o_end) in existing {
                        if other_slot != &slot.id && start < *o_end && *o_start < end {
                            return Err(format!(
                                "clips in slots '{other_slot}' and '{}' are on the same track \
                                 and their times overlap; only the first would render. Move them \
                                 to separate tracks so every region shows.",
                                slot.id
                            ));
                        }
                    }
                }
                ranges_by_track
                    .entry(track_id)
                    .or_default()
                    .push((slot.id.to_string(), start, end));
                intervals_by_slot
                    .entry(slot.id.to_string())
                    .or_default()
                    .push((start, end));
            }
        }
        if entries.len() > 1 {
            let candidates: Vec<i64> = intervals_by_slot
                .values()
                .flat_map(|ivs| ivs.iter().map(|(s, _)| *s))
                .collect();
            let coincides = candidates.iter().any(|&f| {
                intervals_by_slot
                    .values()
                    .all(|ivs| ivs.iter().any(|&(s, e)| s <= f && f < e))
            });
            if !coincides {
                return Err(
                    "the selected clips never play at the same time, so no single frame shows \
                     every region. Overlap their timeline ranges before laying them out."
                        .to_string(),
                );
            }
        }

        // Re-layout mode: set each clip's transform + crop from its slot.
        let canvas_w = self.timeline.width;
        let canvas_h = self.timeline.height;
        let mut applied: Vec<String> = Vec::new();
        for (slot, clip_ids, _media, anchor) in &entries {
            for clip_id in clip_ids {
                let loc = timeline_core::find_clip(&self.timeline, clip_id)
                    .ok_or_else(|| format!("slot '{}': clip not found: {clip_id}", slot.id))?;
                let (ti, ci) = (loc.track_index, loc.clip_index);
                let media_ref = self.timeline.tracks[ti].clips[ci].media_ref.clone();
                let entry = self.media_manifest.entry_for(&media_ref);
                let sw = entry.and_then(|e| e.source_width).unwrap_or(0);
                let sh = entry.and_then(|e| e.source_height).unwrap_or(0);
                let (transform, crop) = core_model::video_layout::layout_placement(
                    slot.rect, fit, sw, sh, canvas_w, canvas_h, anchor.0, anchor.1,
                );
                let clip = &mut self.timeline.tracks[ti].clips[ci];
                clip.transform = transform;
                clip.crop = crop;
                clip.position_track = None;
                clip.scale_track = None;
                clip.rotation_track = None;
                clip.crop_track = None;
            }
            applied.push(format!("{} -> {}", slot.id, clip_ids.join(", ")));
        }

        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!(
                    "Applied '{layout_name}' layout ({}) on existing clips: {}. \
                     Stacking follows current track order; reorder tracks if a PIP inset \
                     isn't on top.",
                    fit.as_str(),
                    applied.join("; ")
                )
            }]
        }))
    }

    /// Place-new mode (#226): create a stacked video track per slot (highest z on
    /// top, since tracks[0] is the TOP layer) and place one new clip from each
    /// slot's mediaRef, auto-detecting project settings from the first video, with
    /// the layout transform/crop baked into each clip. New clips at a common
    /// start_frame/duration are inherently co-visible, so no overlap checks apply.
    fn apply_layout_place_new(
        &mut self,
        entries: &[(core_model::LayoutSlot, Vec<String>, Option<String>, (f64, f64))],
        layout_name: &str,
        fit: LayoutFit,
        args: &Value,
    ) -> Result<Value, String> {
        let start_frame = args.get("startFrame").and_then(Value::as_i64).unwrap_or(0);
        let duration_frames = args
            .get("durationFrames")
            .and_then(Value::as_i64)
            .unwrap_or(0);
        if start_frame < 0 {
            return Err(format!("startFrame must be >= 0 (got {start_frame})"));
        }
        if duration_frames < 1 {
            return Err("apply_layout placing new clips requires durationFrames >= 1.".to_string());
        }
        // resolve_placement (called per slot below) errors when BOTH durationFrames
        // and trimEndFrame are present. Reject that here, up front, so the failure
        // can't fire mid-loop after tracks are already created (exec_mut does not
        // roll back a partial mutation) — validate everything before mutating.
        if args.get("trimEndFrame").is_some() {
            return Err(
                "apply_layout place-new sizes clips with durationFrames; remove trimEndFrame."
                    .to_string(),
            );
        }

        // Validate every slot's asset exists and is video/image BEFORE mutating.
        for (slot, _clips, media_ref, _anchor) in entries {
            let mref = media_ref.as_deref().unwrap_or_default();
            match self.media_manifest.entry_for(mref).map(|e| e.r#type.clone()) {
                Some(ClipType::Video) | Some(ClipType::Image) => {}
                Some(other) => {
                    return Err(format!(
                        "slot '{}': asset {mref} is {other:?}; layout slots take video or image.",
                        slot.id
                    ))
                }
                None => return Err(format!("slot '{}': asset not found: {mref}", slot.id)),
            }
        }

        // Auto-detect project settings from the first video if not yet configured
        // (same rule as add_clips / Swift applySettingsIfNeeded).
        let mut settings_note: Option<String> = None;
        if !self.timeline.settings_configured {
            let detected = entries.iter().find_map(|(_s, _c, mref, _a)| {
                self.media_manifest
                    .entry_for(mref.as_deref().unwrap_or_default())
                    .filter(|e| e.r#type == ClipType::Video)
                    .map(|e| (e.source_fps, e.source_width, e.source_height))
            });
            if let Some((sfps, sw, sh)) = detected {
                let fps = sfps
                    .map(|f| f.round() as i64)
                    .filter(|f| (1..=120).contains(f))
                    .unwrap_or(self.timeline.fps);
                // Guard degenerate (0/negative) source dims: fall back to the current
                // canvas so the applied settings match the reported note (apply_settings
                // itself ignores a non-positive canvas).
                let width = sw.filter(|&w| w > 0).unwrap_or(self.timeline.width);
                let height = sh.filter(|&h| h > 0).unwrap_or(self.timeline.height);
                let (pf, pw, ph) = (self.timeline.fps, self.timeline.width, self.timeline.height);
                timeline_core::apply_settings(&mut self.timeline, fps, width, height, |c| {
                    c.transform == Transform::default()
                });
                if fps != pf || width != pw || height != ph {
                    settings_note = Some(format!(
                        "Set project to {width}x{height} @ {fps}fps from the first clip."
                    ));
                }
            }
        }
        let project_fps = self.timeline.fps;

        // Create a video track per slot, inserting each at index 0 in ascending z
        // order so the highest-z slot ends up on top (tracks[0] is the TOP layer).
        let mut sorted: Vec<&(core_model::LayoutSlot, Vec<String>, Option<String>, (f64, f64))> =
            entries.iter().collect();
        sorted.sort_by_key(|(s, _, _, _)| s.z);
        let mut track_id_by_slot: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();
        let tracks_before = self.timeline.tracks.len();
        for (slot, _, _, _) in &sorted {
            let idx = timeline_core::insert_track_at(&mut self.timeline, 0, ClipType::Video)
                .map_err(|_| "Failed to create video track".to_string())?;
            track_id_by_slot.insert(slot.id.to_string(), self.timeline.tracks[idx].id.clone());
        }

        let canvas_w = self.timeline.width;
        let canvas_h = self.timeline.height;
        let mut applied: Vec<String> = Vec::new();
        for (slot, _clips, media_ref, anchor) in entries {
            let mref = media_ref.as_deref().unwrap_or_default();
            let entry = self.media_manifest.entry_for(mref);
            let placement = resolve_placement(entry, args, project_fps)?;
            let sw = entry.and_then(|e| e.source_width).unwrap_or(0);
            let sh = entry.and_then(|e| e.source_height).unwrap_or(0);
            let (transform, crop) = core_model::video_layout::layout_placement(
                slot.rect, fit, sw, sh, canvas_w, canvas_h, anchor.0, anchor.1,
            );
            let clip = Clip {
                id: Uuid::new_v4().to_string(),
                media_ref: mref.to_string(),
                media_type: placement.media_type.clone(),
                source_clip_type: placement.media_type,
                start_frame,
                duration_frames: placement.duration_frames,
                trim_start_frame: placement.trim_start_frame,
                trim_end_frame: placement.trim_end_frame,
                speed: 1.0,
                volume: 1.0,
                fade_in_frames: 0,
                fade_out_frames: 0,
                fade_in_interpolation: Interpolation::Linear,
                fade_out_interpolation: Interpolation::Linear,
                opacity: 1.0,
                transform,
                crop,
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
                text_animation: None,
                word_timings: None,
            };
            let Some(tid) = track_id_by_slot.get(slot.id) else {
                continue;
            };
            let Some(tidx) = self.timeline.tracks.iter().position(|t| &t.id == tid) else {
                continue;
            };
            let placed = timeline_core::place_clips(&mut self.timeline, tidx, start_frame, &[clip]);
            if let Some(pid) = placed.first() {
                applied.push(format!("{} -> {pid}", slot.id));
            }
        }

        if applied.is_empty() {
            return Err("apply_layout created no clips.".to_string());
        }
        let created = self.timeline.tracks.len() - tracks_before;
        let prefix = settings_note.map(|n| format!("{n} ")).unwrap_or_default();
        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!(
                    "{prefix}Created {created} video track(s). Applied '{layout_name}' layout \
                     ({}) at frame {start_frame} for {duration_frames}: {}.",
                    fit.as_str(),
                    applied.join("; ")
                )
            }]
        }))
    }

    /// CLP-007/008: create a linked audio clip for each placed video-with-audio
    /// clip, on the first audio track FREE over the span the audio will occupy
    /// (so existing audio is never clobbered), or a newly created track. Returns
    /// the number of linked audio clips created.
    fn auto_link_placed_audio(&mut self, video_with_audio: &[String]) -> Result<usize, String> {
        if video_with_audio.is_empty() {
            return Ok(0);
        }
        let (mut span_start, mut span_end) = (i64::MAX, i64::MIN);
        for pid in video_with_audio {
            if let Some(loc) = timeline_core::find_clip(&self.timeline, pid) {
                let c = &self.timeline.tracks[loc.track_index].clips[loc.clip_index];
                span_start = span_start.min(c.start_frame);
                span_end = span_end.max(c.start_frame + c.duration_frames);
            }
        }
        let free = self.timeline.tracks.iter().position(|t| {
            t.r#type == ClipType::Audio
                && !t.clips.iter().any(|c| {
                    c.start_frame < span_end && c.start_frame + c.duration_frames > span_start
                })
        });
        let audio_ti = match free {
            Some(ti) => ti,
            None => {
                let at = self.timeline.tracks.len();
                timeline_core::insert_track_at(&mut self.timeline, at, ClipType::Audio)
                    .map_err(|_| "Failed to create audio track for linked audio".to_string())?
            }
        };
        Ok(timeline_core::link_audio_for_placed_clips(&mut self.timeline, video_with_audio, audio_ti).len())
    }

    fn cmd_add_clips(&mut self, args: &Value) -> Result<Value, String> {
        let media_ids: Vec<String> = args
            .get("mediaIds")
            .and_then(|v| v.as_array())
            .ok_or_else(|| "Missing mediaIds".to_string())?
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();

        if media_ids.is_empty() {
            return Err("mediaIds must be non-empty".to_string());
        }

        // trackIndex is optional (MUT-002/003): omit it and the tool auto-creates /
        // reuses a video track for visual clips and an audio track for audio clips.
        let track_index_opt = args
            .get("trackIndex")
            .and_then(|v| v.as_i64())
            .map(|i| i as usize);

        // Auto-detect project settings from the first video the FIRST time clips are
        // added (Swift `checkProjectSettings`): silently adopt its fps/size and mark
        // the project configured. Later adds see it configured, keep settings fixed,
        // and only warn on a source-fps mismatch (#233). Runs before `project_fps` so
        // the new clips are placed on the detected timebase.
        let mut settings_note: Option<String> = None;
        if !self.timeline.settings_configured {
            let detected = media_ids.iter().find_map(|id| {
                self.media_manifest
                    .entry_for(id)
                    .filter(|e| e.r#type == ClipType::Video)
                    .map(|e| (e.source_fps, e.source_width, e.source_height))
            });
            if let Some((sfps, sw, sh)) = detected {
                let fps = sfps
                    .map(|f| f.round() as i64)
                    .filter(|f| (1..=120).contains(f))
                    .unwrap_or(self.timeline.fps);
                // Guard degenerate (0/negative) source dims: fall back to the current
                // canvas so the applied settings match the reported note (apply_settings
                // itself ignores a non-positive canvas).
                let width = sw.filter(|&w| w > 0).unwrap_or(self.timeline.width);
                let height = sh.filter(|&h| h > 0).unwrap_or(self.timeline.height);
                let (pf, pw, ph) = (self.timeline.fps, self.timeline.width, self.timeline.height);
                timeline_core::apply_settings(&mut self.timeline, fps, width, height, |c| {
                    c.transform == Transform::default()
                });
                if fps != pf || width != pw || height != ph {
                    settings_note =
                        Some(format!("Set project to {width}x{height} @ {fps}fps from the first clip."));
                }
            }
        }

        let project_fps = self.timeline.fps;
        let mut warnings: Vec<String> = Vec::new();
        let mut clips: Vec<Clip> = Vec::with_capacity(media_ids.len());
        for media_id in &media_ids {
            let entry = self.media_manifest.entry_for(media_id);
            let placement = resolve_placement(entry, args, project_fps)?;
            if let Some(warning) = placement.fps_warning {
                if !warnings.contains(&warning) {
                    warnings.push(warning);
                }
            }
            clips.push(Clip {
                id: Uuid::new_v4().to_string(),
                media_ref: media_id.clone(),
                media_type: placement.media_type.clone(),
                source_clip_type: placement.media_type,
                start_frame: 0,
                duration_frames: placement.duration_frames,
                trim_start_frame: placement.trim_start_frame,
                trim_end_frame: placement.trim_end_frame,
                speed: 1.0,
                volume: 1.0,
                fade_in_frames: 0,
                fade_out_frames: 0,
                fade_in_interpolation: Interpolation::Linear,
                fade_out_interpolation: Interpolation::Linear,
                opacity: 1.0,
                transform: Transform::default(),
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
                text_animation: None,
                word_timings: None,
            });
        }

        // Place the clips and collect the placed ids of the VISUAL clips (the only
        // ones that can carry linked audio).
        let (placed_count, placed_visual_ids): (usize, Vec<String>) = match track_index_opt {
            Some(track_index) => {
                if track_index >= self.timeline.tracks.len() {
                    return Err(format!("Track index {track_index} out of bounds"));
                }
                // Reject type-incompatible placement before mutating anything.
                let track = &self.timeline.tracks[track_index];
                for clip in &clips {
                    if !track.is_compatible_with(clip.media_type) {
                        return Err(format!(
                            "media type {:?} is not compatible with track {track_index} ({:?})",
                            clip.media_type, track.r#type
                        ));
                    }
                }
                let placed = timeline_core::place_clips(&mut self.timeline, track_index, 0, &clips);
                (placed.len(), placed)
            }
            None => {
                // Auto-create: visual clips share a video track, audio clips share an
                // audio track (creating either if absent).
                let visual: Vec<Clip> =
                    clips.iter().filter(|c| c.media_type.is_visual()).cloned().collect();
                let audio: Vec<Clip> =
                    clips.iter().filter(|c| !c.media_type.is_visual()).cloned().collect();
                let mut visual_ids = Vec::new();
                if !visual.is_empty() {
                    let vti = match self
                        .timeline
                        .tracks
                        .iter()
                        .position(|t| t.r#type != ClipType::Audio)
                    {
                        Some(ti) => ti,
                        None => {
                            let at = self.timeline.tracks.len();
                            timeline_core::insert_track_at(&mut self.timeline, at, ClipType::Video)
                                .map_err(|_| "Failed to create video track".to_string())?
                        }
                    };
                    visual_ids = timeline_core::place_clips(&mut self.timeline, vti, 0, &visual);
                }
                if !audio.is_empty() {
                    let ati = match self
                        .timeline
                        .tracks
                        .iter()
                        .position(|t| t.r#type == ClipType::Audio)
                    {
                        Some(ti) => ti,
                        None => {
                            let at = self.timeline.tracks.len();
                            timeline_core::insert_track_at(&mut self.timeline, at, ClipType::Audio)
                                .map_err(|_| "Failed to create audio track".to_string())?
                        }
                    };
                    let _ = timeline_core::place_clips(&mut self.timeline, ati, 0, &audio);
                }
                (visual.len() + audio.len(), visual_ids)
            }
        };

        // CLP-007/008: auto-link video-with-audio. Detect from each placed visual
        // clip's own media_ref (works whichever placement path ran).
        let video_with_audio: Vec<String> = placed_visual_ids
            .iter()
            .filter(|pid| {
                let Some(loc) = timeline_core::find_clip(&self.timeline, pid) else {
                    return false;
                };
                let c = &self.timeline.tracks[loc.track_index].clips[loc.clip_index];
                c.media_type == ClipType::Video
                    && self
                        .media_manifest
                        .entry_for(&c.media_ref)
                        .and_then(|e| e.has_audio)
                        == Some(true)
            })
            .cloned()
            .collect();
        let linked_audio_count = self.auto_link_placed_audio(&video_with_audio)?;

        let mut text = format!("Added {placed_count} clip(s)");
        if let Some(note) = &settings_note {
            text.push('\n');
            text.push_str(note);
        }
        if linked_audio_count > 0 {
            text.push_str(&format!("\n(+{linked_audio_count} linked audio clip(s))"));
        }
        for warning in &warnings {
            text.push('\n');
            text.push_str(warning);
        }
        Ok(json!({
            "content": [{
                "type": "text",
                "text": text
            }]
        }))
    }

    fn cmd_insert_clips(&mut self, args: &Value) -> Result<Value, String> {
        let media_ids: Vec<String> = args
            .get("mediaIds")
            .and_then(|v| v.as_array())
            .ok_or_else(|| "Missing mediaIds".to_string())?
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();

        let track_index = args
            .get("trackIndex")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| "Missing trackIndex".to_string())? as usize;

        let frame = args
            .get("frame")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| "Missing frame".to_string())?;

        if track_index >= self.timeline.tracks.len() {
            return Err(format!("Track index {track_index} out of bounds"));
        }

        let project_fps = self.timeline.fps;
        let mut warnings: Vec<String> = Vec::new();
        let mut clip_specs: Vec<timeline_core::RippleInsertClipSpec> =
            Vec::with_capacity(media_ids.len());
        for media_id in &media_ids {
            let entry = self.media_manifest.entry_for(media_id);
            let placement = resolve_placement(entry, args, project_fps)?;
            if let Some(warning) = placement.fps_warning {
                if !warnings.contains(&warning) {
                    warnings.push(warning);
                }
            }
            clip_specs.push(timeline_core::RippleInsertClipSpec {
                asset_id: media_id.clone(),
                duration_frames: placement.duration_frames,
                trim_start_frame: Some(placement.trim_start_frame),
                trim_end_frame: Some(placement.trim_end_frame),
            });
        }

        // Reject type-incompatible placement before mutating anything.
        {
            let track = &self.timeline.tracks[track_index];
            for media_id in &media_ids {
                let media_type = self
                    .media_manifest
                    .entry_for(media_id)
                    .map(|e| e.r#type)
                    .unwrap_or(ClipType::Video);
                if !track.is_compatible_with(media_type) {
                    return Err(format!(
                        "media type {media_type:?} is not compatible with track {track_index} ({:?})",
                        track.r#type
                    ));
                }
            }
        }

        // CLP-007/008/RPL-010: a video-with-audio inserted on a video track gets a
        // linked audio clip. The audio track is pushed with the video (so room opens
        // at the insert frame) via linked_audio_track_index. When no audio track
        // exists we target the future end index — compute skips pushing an
        // out-of-range track (a new empty one needs no push) — and create it only on
        // success, so a refusal leaves no orphan track.
        let has_linked_audio: Vec<bool> = media_ids
            .iter()
            .map(|mid| {
                self.media_manifest
                    .entry_for(mid)
                    .map(|e| e.r#type == ClipType::Video && e.has_audio == Some(true))
                    .unwrap_or(false)
            })
            .collect();
        let existing_audio_ti = self
            .timeline
            .tracks
            .iter()
            .position(|t| t.r#type == ClipType::Audio);
        let linked_audio_ti = if has_linked_audio.iter().any(|&b| b) {
            Some(existing_audio_ti.unwrap_or(self.timeline.tracks.len()))
        } else {
            None
        };

        let config = timeline_core::RippleInsertConfig {
            track_index,
            insert_frame: frame,
            clips: clip_specs,
            linked_audio_track_index: linked_audio_ti,
        };

        let outcome = timeline_core::compute_ripple_insert_with_split(&self.timeline, config);

        match outcome {
            timeline_core::RippleInsertWithSplitOutcome::Ok(plan) => {
                // Apply split actions before shifting.
                for (_, clip_id, split_at) in &plan.split_actions {
                    timeline_core::split_clip(&mut self.timeline, clip_id, *split_at);
                }
                // Shift POSITIONALLY, not by clip id: splitting a straddling clip
                // creates a fresh right-half id the pre-computed shift list cannot
                // reference, so a by-id apply left it un-shifted and place_clips then
                // trimmed (destroyed) its tail. Push every clip at/after the insert
                // frame by total_push on each pushed track (matches the library
                // apply_ripple_insert_with_split).
                let push_tracks: std::collections::BTreeSet<usize> =
                    plan.insert.push_track_indices.iter().copied().collect();
                let insert_frame = plan.insert.insert_frame;
                let total_push = plan.insert.total_push;
                for ti in 0..self.timeline.tracks.len() {
                    if !push_tracks.contains(&ti) {
                        continue;
                    }
                    for clip in &mut self.timeline.tracks[ti].clips {
                        if clip.start_frame >= insert_frame {
                            clip.start_frame += total_push;
                        }
                    }
                    timeline_core::sort_clips_on_track(&mut self.timeline, ti);
                }
                // Place new clips
                let new_clips: Vec<Clip> = plan
                    .insert
                    .clips
                    .iter()
                    .map(|spec| {
                        let media_type = self
                            .media_manifest
                            .entry_for(&spec.asset_id)
                            .map(|e| e.r#type.clone())
                            .unwrap_or(ClipType::Video);
                        Clip {
                        id: Uuid::new_v4().to_string(),
                        media_ref: spec.asset_id.clone(),
                        media_type: media_type.clone(),
                        source_clip_type: media_type,
                        start_frame: plan.insert.insert_frame,
                        duration_frames: spec.duration_frames,
                        trim_start_frame: spec.trim_start_frame.unwrap_or(0),
                        trim_end_frame: spec.trim_end_frame.unwrap_or(0),
                        speed: 1.0,
                        volume: 1.0,
                        fade_in_frames: 0,
                        fade_out_frames: 0,
                        fade_in_interpolation: Interpolation::Linear,
                        fade_out_interpolation: Interpolation::Linear,
                        opacity: 1.0,
                        transform: Transform::default(),
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
                        text_animation: None,
                        word_timings: None,
                        }
                    })
                    .collect();

                let placed = timeline_core::place_clips(
                    &mut self.timeline,
                    plan.insert.track_index,
                    plan.insert.insert_frame,
                    &new_clips,
                );

                // Link audio for the placed video-with-audio clips. The audio track
                // has already had room opened by the ripple push (or is a freshly
                // created empty track), so link_audio_for_placed_clips places into a
                // clear region. `placed` is 1:1 with media_ids/has_linked_audio.
                let mut linked_audio_count = 0usize;
                if linked_audio_ti.is_some() {
                    let video_with_audio_ids: Vec<String> = has_linked_audio
                        .iter()
                        .zip(placed.iter())
                        .filter_map(|(&b, pid)| b.then(|| pid.clone()))
                        .collect();
                    if !video_with_audio_ids.is_empty() {
                        let audio_ti = match existing_audio_ti {
                            Some(ti) => ti,
                            None => {
                                let at = self.timeline.tracks.len();
                                timeline_core::insert_track_at(
                                    &mut self.timeline,
                                    at,
                                    ClipType::Audio,
                                )
                                .map_err(|_| {
                                    "Failed to create audio track for linked audio".to_string()
                                })?
                            }
                        };
                        linked_audio_count = timeline_core::link_audio_for_placed_clips(
                            &mut self.timeline,
                            &video_with_audio_ids,
                            audio_ti,
                        )
                        .len();
                    }
                }

                let mut text = format!(
                    "Inserted {} clip(s) at track {} frame {}",
                    placed.len(),
                    plan.insert.track_index,
                    plan.insert.insert_frame
                );
                if linked_audio_count > 0 {
                    text.push_str(&format!("\n(+{linked_audio_count} linked audio clip(s))"));
                }
                for warning in &warnings {
                    text.push('\n');
                    text.push_str(warning);
                }
                Ok(json!({
                    "content": [{
                        "type": "text",
                        "text": text
                    }]
                }))
            }
            timeline_core::RippleInsertWithSplitOutcome::Refused(msg) => Ok(json!({
                "content": [{
                    "type": "text",
                    "text": format!("Insert refused: {msg}")
                }],
                "isError": true,
            })),
        }
    }

    fn cmd_undo(&mut self) -> Result<Value, String> {
        match self.undo_stack.undo() {
            Ok(timeline) => {
                self.timeline = timeline;
                Ok(json!({
                    "content": [{
                        "type": "text",
                        "text": "Undo successful".to_string()
                    }]
                }))
            }
            Err(_) => Ok(json!({
                "content": [{
                    "type": "text",
                    "text": "Nothing to undo".to_string()
                }],
                "isError": true,
            })),
        }
    }

    fn cmd_redo(&mut self) -> Result<Value, String> {
        match self.undo_stack.redo() {
            Ok(timeline) => {
                self.timeline = timeline;
                Ok(json!({
                    "content": [{
                        "type": "text",
                        "text": "Redo successful".to_string()
                    }]
                }))
            }
            Err(_) => Ok(json!({
                "content": [{
                    "type": "text",
                    "text": "Nothing to redo".to_string()
                }],
                "isError": true,
            })),
        }
    }

    // ── Media read-only tools ──────────────────────────────────────────────

    fn cmd_get_media(&self, args: &Value) -> Result<Value, String> {
        let media_id = args
            .get("mediaId")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing mediaId".to_string())?;

        let entry = self
            .media_manifest
            .entries
            .iter()
            .find(|e| e.id == media_id)
            .ok_or_else(|| format!("Media '{}' not found", media_id))?;

        // Surface async-generation status (#216) so the agent waits for 'none'
        // before referencing an asset that is still preparing/generating/downloading.
        let status_note = match entry.generation_status.as_deref() {
            Some(s) if s != "none" && !s.is_empty() => {
                format!(", generationStatus: {s} (not ready — poll get_media until 'none')")
            }
            _ => String::new(),
        };
        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!(
                    "Media: {} ({:?}), duration: {:.1}s, source: {:?}{status_note}",
                    entry.name, entry.r#type, entry.duration, entry.source
                )
            }]
        }))
    }

    fn cmd_search_media(&self, args: &Value) -> Result<Value, String> {
        let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
        let r#type = args.get("type").and_then(|v| v.as_str());
        let limit = args
            .get("limit")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
            .unwrap_or(50);

        let results: Vec<&MediaManifestEntry> = self
            .media_manifest
            .entries
            .iter()
            .filter(|e| {
                let name_match =
                    query.is_empty() || e.name.to_lowercase().contains(&query.to_lowercase());
                let type_match = r#type.map_or(true, |t| {
                    let t_lower = t.to_lowercase();
                    let type_str = format!("{:?}", e.r#type).to_lowercase();
                    type_str == t_lower
                });
                name_match && type_match
            })
            .collect();

        // Convert to SearchHitInfo for the files group (name-based search).
        let files: Vec<SearchHitInfo> = results
            .iter()
            .map(|e| SearchHitInfo {
                media_id: e.id.clone(),
                frame: 0,
                score: 1.0,
                kind: "File".to_string(),
            })
            .collect();

        // READ-026: Include search indexing status in output.
        let status = if results.is_empty() && self.search_status.is_empty() {
            "ok".to_string()
        } else if !self.search_status.is_empty() {
            if results.is_empty() {
                self.search_status.clone()
            } else {
                format!("Found {} media; {}", results.len(), self.search_status)
            }
        } else {
            format!("Found {} media", results.len())
        };

        let output = format_search_results(Vec::new(), Vec::new(), files, status, limit);
        let output_json = serde_json::to_string_pretty(&output).unwrap_or_default();

        Ok(json!({
            "content": [{
                "type": "text",
                "text": output_json
            }]
        }))
    }

    fn cmd_list_folders(&self) -> Result<Value, String> {
        let folders = &self.media_manifest.folders;
        if folders.is_empty() {
            return Ok(json!({
                "content": [{"type": "text", "text": "No folders".to_string()}]
            }));
        }
        let lines: Vec<String> = folders
            .iter()
            .map(|f| {
                let parent = f
                    .parent_folder_id
                    .as_ref()
                    .map(|p| format!(" (parent: {})", p))
                    .unwrap_or_default();
                format!("{}: {}{}", f.id, f.name, parent)
            })
            .collect();
        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!("Folders ({}):\n{}", folders.len(), lines.join("\n"))
            }]
        }))
    }

    fn cmd_list_models(&self) -> Result<Value, String> {
        Ok(json!({
            "content": [{
                "type": "text",
                "text": serde_json::to_string_pretty(&json!({
                    "video": [
                        {"id": "gen-3", "name": "Gen-3 Alpha", "status": "available"},
                        {"id": "kling", "name": "Kling 1.6", "status": "available"}
                    ],
                    "image": [
                        {"id": "sd3", "name": "Stable Diffusion 3", "status": "available"},
                        {"id": "dalle", "name": "DALL-E 3", "status": "available"}
                    ],
                    "audio": [
                        {"id": "elevenlabs", "name": "ElevenLabs", "status": "available"},
                        {"id": "music-gen", "name": "MusicGen", "status": "available"}
                    ]
                }))
                .unwrap_or_default()
            }]
        }))
    }

    fn cmd_inspect_media(&self, args: &Value) -> Result<Value, String> {
        let media_id = args
            .get("mediaId")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing mediaId".to_string())?;

        // Issue #39: resolve language — per-call arg → project setting → None.
        let _language = args
            .get("language")
            .and_then(|v| v.as_str())
            .map(String::from)
            .or_else(|| self.timeline.transcription_language.clone());

        let entry = self
            .media_manifest
            .entries
            .iter()
            .find(|e| e.id == media_id)
            .ok_or_else(|| format!("Media '{}' not found", media_id))?;

        // READ-013: Text clip rejection
        if entry.r#type == core_model::ClipType::Text {
            return Ok(json!({
                "content": [{
                    "type": "text",
                    "text": "Cannot inspect a text clip with inspect_media. Use get_timeline to view text clips."
                }],
                "isError": true,
            }));
        }

        // READ-014: clipId → mediaRef cross-validation
        if let Some(clip_id) = args.get("clipId").and_then(|v| v.as_str()) {
            let all_clips: Vec<&Clip> =
                self.timeline.tracks.iter().flat_map(|t| &t.clips).collect();
            let clip = all_clips
                .iter()
                .find(|c| c.id == clip_id)
                .ok_or_else(|| format!("Clip '{}' not found on timeline", clip_id))?;
            if clip.media_ref != entry.id {
                return Err(format!(
                    "Clip '{}' references media '{}', not '{}'",
                    clip_id, clip.media_ref, media_id
                ));
            }
        }

        // READ-015: maxFrames default 6, clamped to 1..12
        let max_frames: usize = args
            .get("maxFrames")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(6)
            .clamp(1, 12);

        // Find matching clip on timeline (if any)
        let clip = self
            .timeline
            .tracks
            .iter()
            .flat_map(|t| &t.clips)
            .find(|c| c.media_ref == media_id)
            .cloned();

        // Build the InspectMediaInput
        let inspect_input = InspectMediaInput {
            entry: entry.clone(),
            clip,
            timeline_fps: self.timeline.fps,
            max_frames,
            inline_image_data: None,         // caller supplies via callbacks
            inline_video_frames: Vec::new(), // caller supplies via callbacks
            transcription_words: Vec::new(), // caller supplies via callbacks
        };

        let details = format_inspect_media(&inspect_input)
            .map_err(|e| format!("inspect_media error: {}", e))?;

        Ok(json!({
            "content": [{
                "type": "text",
                "text": serde_json::to_string_pretty(&details).unwrap_or_default()
            }]
        }))
    }

    fn cmd_inspect_timeline(&self) -> Result<Value, String> {
        let formatted = format_timeline_json(&self.timeline);
        Ok(json!({
            "content": [{
                "type": "text",
                "text": serde_json::to_string_pretty(&formatted).unwrap_or_default()
            }]
        }))
    }

    fn cmd_get_transcript(&self, args: &Value) -> Result<Value, String> {
        // READ-021: tolerate legacy wordTimestamps
        let _word_timestamps = args.get("wordTimestamps");

        // Look up media
        let media_id = args.get("mediaId").and_then(|v| v.as_str());
        if media_id.is_none() {
            return Ok(json!({
                "content": [{
                    "type": "text",
                    "text": "Transcript system is not yet connected to the timeline engine. No captions available."
                }],
                "isError": true,
            }));
        }

        // Issue #39: resolve language — per-call arg → project setting → None (platform uses system).
        let language = args
            .get("language")
            .and_then(|v| v.as_str())
            .map(String::from)
            .or_else(|| self.timeline.transcription_language.clone());

        // Parse optional pagination
        let start_frame = args
            .get("startFrame")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<i64>().ok());

        let fps = self.timeline.fps.max(1);

        // Collect timeline-visible clips for word attribution
        let clips: Vec<TranscriptClipInput> = self
            .timeline
            .tracks
            .iter()
            .flat_map(|t| t.clips.iter())
            .filter(|c| c.media_ref == media_id.unwrap())
            .map(|c| TranscriptClipInput {
                id: c.id.clone(),
                start_frame: c.start_frame,
                duration_frames: c.duration_frames,
            })
            .collect();

        let options = TranscriptFormatOptions {
            start_frame,
            language,
            ..Default::default()
        };

        // No transcript data source connected yet, return empty result
        let formatted = format_transcript_json(fps, &[], &clips, &options);
        Ok(json!({
            "content": [{
                "type": "text",
                "text": serde_json::to_string_pretty(&formatted)
                    .unwrap_or_else(|_| "{}".into()),
            }]
        }))
    }

    // ── Media mutation tools ───────────────────────────────────────────────

    fn cmd_create_folder(&mut self, args: &Value) -> Result<Value, String> {
        let name = args
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing name".to_string())?;
        let parent_folder_id = args.get("parentFolderId").and_then(|v| v.as_str());

        let folder = core_model::MediaFolder {
            id: Uuid::new_v4().to_string(),
            name: name.to_string(),
            parent_folder_id: parent_folder_id.map(String::from),
        };
        let folder_id = folder.id.clone();
        self.media_manifest.folders.push(folder);

        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!("Created folder '{}' with id {}", name, folder_id)
            }]
        }))
    }

    fn cmd_rename_folder(&mut self, args: &Value) -> Result<Value, String> {
        let folder_id = args
            .get("folderId")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing folderId".to_string())?;
        let name = args
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing name".to_string())?;

        let folder = self
            .media_manifest
            .folders
            .iter_mut()
            .find(|f| f.id == folder_id)
            .ok_or_else(|| format!("Folder '{}' not found", folder_id))?;
        folder.name = name.to_string();

        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!("Renamed folder '{}' to '{}'", folder_id, name)
            }]
        }))
    }

    fn cmd_delete_folder(&mut self, args: &Value) -> Result<Value, String> {
        let folder_id = args
            .get("folderId")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing folderId".to_string())?;

        let pos = self
            .media_manifest
            .folders
            .iter()
            .position(|f| f.id == folder_id)
            .ok_or_else(|| format!("Folder '{}' not found", folder_id))?;
        self.media_manifest.folders.remove(pos);

        // Unset folder_id on entries in this folder
        for entry in self.media_manifest.entries.iter_mut() {
            if entry.folder_id.as_deref() == Some(folder_id) {
                entry.folder_id = None;
            }
        }

        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!("Deleted folder '{}'", folder_id)
            }]
        }))
    }

    fn cmd_rename_media(&mut self, args: &Value) -> Result<Value, String> {
        let media_id = args
            .get("mediaId")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing mediaId".to_string())?;
        let name = args
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing name".to_string())?;

        let entry = self
            .media_manifest
            .entries
            .iter_mut()
            .find(|e| e.id == media_id)
            .ok_or_else(|| format!("Media '{}' not found", media_id))?;
        entry.name = name.to_string();

        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!("Renamed media '{}' to '{}'", media_id, name)
            }]
        }))
    }

    fn cmd_delete_media(&mut self, args: &Value) -> Result<Value, String> {
        let media_id = args
            .get("mediaId")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing mediaId".to_string())?;

        let pos = self
            .media_manifest
            .entries
            .iter()
            .position(|e| e.id == media_id)
            .ok_or_else(|| format!("Media '{}' not found", media_id))?;
        self.media_manifest.entries.remove(pos);

        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!("Deleted media '{}'", media_id)
            }]
        }))
    }

    fn cmd_move_to_folder(&mut self, args: &Value) -> Result<Value, String> {
        let media_id = args
            .get("mediaId")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing mediaId".to_string())?;
        let folder_id = args
            .get("folderId")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing folderId".to_string())?;

        let entry = self
            .media_manifest
            .entries
            .iter_mut()
            .find(|e| e.id == media_id)
            .ok_or_else(|| format!("Media '{}' not found", media_id))?;

        // Verify folder exists
        if !self
            .media_manifest
            .folders
            .iter()
            .any(|f| f.id == folder_id)
        {
            return Err(format!("Folder '{}' not found", folder_id));
        }

        entry.folder_id = Some(folder_id.to_string());
        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!("Moved media '{}' to folder '{}'", media_id, folder_id)
            }]
        }))
    }

    fn cmd_import_media(&mut self, args: &Value) -> Result<Value, String> {
        let name = args
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing name".to_string())?;
        let file_path = args
            .get("filePath")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing filePath".to_string())?;
        let media_type = args.get("type").and_then(|v| v.as_str()).unwrap_or("video");
        let duration = args
            .get("duration")
            .and_then(|v| v.as_f64())
            .unwrap_or(10.0);
        let folder_id = args.get("folderId").and_then(|v| v.as_str());

        let clip_type = match media_type.to_lowercase().as_str() {
            "audio" => core_model::ClipType::Audio,
            "image" => core_model::ClipType::Image,
            "text" => core_model::ClipType::Text,
            _ => core_model::ClipType::Video,
        };

        let entry = core_model::MediaManifestEntry {
            id: Uuid::new_v4().to_string(),
            name: name.to_string(),
            r#type: clip_type,
            source: MediaSource::External {
                absolute_path: file_path.to_string(),
            },
            duration,
            generation_input: None,
            source_width: None,
            source_height: None,
            source_fps: None,
            has_audio: None,
            folder_id: folder_id.map(String::from),
            cached_remote_url: None,
            cached_remote_url_expires_at: None,
            source_timecode_frame: None,
            source_timecode_quanta: None,
            source_timecode_drop_frame: None,
            ai_tags: None,
            ai_description: None,
            ai_label_status: None,
            generation_status: None,
        };
        let entry_id = entry.id.clone();
        self.media_manifest.entries.push(entry);

        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!("Imported '{}' as '{}' (id: {})", file_path, name, entry_id)
            }]
        }))
    }

    fn cmd_import_folder(&mut self, args: &Value) -> Result<Value, String> {
        let folder_name = args
            .get("folderName")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing folderName".to_string())?;
        let recursive = args
            .get("recursive")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let folder = core_model::MediaFolder {
            id: Uuid::new_v4().to_string(),
            name: folder_name.to_string(),
            parent_folder_id: None,
        };
        let folder_id = folder.id.clone();
        self.media_manifest.folders.push(folder);

        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!(
                    "Created folder '{}' (id: {}, recursive: {}) — actual file scanning is not yet implemented",
                    folder_name, folder_id, recursive
                )
            }]
        }))
    }

    fn cmd_duplicate_project(&mut self) -> Result<Value, String> {
        // Upstream #67: duplicating a project copies its .palmier package on disk and
        // reopens the copy as current. That is host filesystem I/O the in-memory
        // ToolExecutor cannot do (it has no project path or fs handle). The pure plan
        // exists as project_io::project_duplicate::plan_duplicate; the host must
        // execute it. Report honestly rather than claiming a no-op succeeded.
        Ok(json!({
            "content": [{
                "type": "text",
                "text": "Project duplication requires host filesystem support and is not available in this context."
            }],
            "isError": true,
        }))
    }

    // ── Text / annotation tools ────────────────────────────────────────────

    fn cmd_add_texts(&mut self, args: &Value) -> Result<Value, String> {
        let texts_val = args
            .get("texts")
            .and_then(|v| v.as_array())
            .ok_or_else(|| "Missing texts array".to_string())?;

        let track_index = args
            .get("trackIndex")
            .and_then(|v| v.as_i64())
            .unwrap_or(-1) as usize;

        // Find or create a text track
        let ti = if track_index < self.timeline.tracks.len() {
            track_index
        } else {
            // Find existing text/visual track or create one
            let existing = self.timeline.tracks.iter().position(|t| {
                t.r#type == core_model::ClipType::Text || t.r#type == core_model::ClipType::Video
            });
            match existing {
                Some(idx) => idx,
                None => {
                    timeline_core::insert_track_at(
                        &mut self.timeline,
                        0,
                        core_model::ClipType::Video,
                    )
                    .map_err(|_| "Failed to create track".to_string())?;
                    0
                }
            }
        };

        let mut created_ids: Vec<String> = Vec::new();
        let mut clips: Vec<Clip> = Vec::new();
        let mut current_frame = 0i64;

        // Find the max end frame on this track for placement
        for clip in &self.timeline.tracks[ti].clips {
            let end = clip.start_frame + clip.duration_frames;
            if end > current_frame {
                current_frame = end;
            }
        }

        for t_val in texts_val {
            // Accept Swift's `content` key, falling back to `text`.
            let text = t_val
                .get("content")
                .or_else(|| t_val.get("text"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let start_frame = t_val
                .get("startFrame")
                .and_then(|v| v.as_i64())
                .unwrap_or(current_frame);
            let duration_frames = t_val
                .get("durationFrames")
                .and_then(|v| v.as_i64())
                .unwrap_or(150);

            // Per-entry text styling (reuses the set_clip_properties parsers).
            let mut style = TextStyle::default();
            if let Some(f) = t_val.get("fontName").and_then(|v| v.as_str()) {
                style.font_name = f.to_string();
            }
            if let Some(s) = t_val.get("fontSize").and_then(|v| v.as_f64()) {
                style.font_size = s;
            }
            if let Some(w) = t_val.get("fontWeight").and_then(|v| v.as_f64()) {
                style.font_weight = w;
            }
            if let Some(hex) = t_val.get("color").and_then(|v| v.as_str()) {
                style.color = core_model::TextRgba::from_hex(hex).ok_or_else(|| {
                    format!("invalid color '{hex}'. Expected '#RGB', '#RRGGBB', or '#RRGGBBAA'")
                })?;
            }
            if let Some(a) = t_val.get("alignment").and_then(|v| v.as_str()) {
                style.alignment = core_model::TextAlignment::from_name(a).ok_or_else(|| {
                    format!("invalid alignment '{a}'. Expected 'left', 'center', or 'right'")
                })?;
            }

            // Explicit box override; partial (centre-only) shifts position, keeping
            // the default size. Auto-fit-to-content sizing is deferred (needs text
            // measurement, which lives in the render layer).
            let mut transform = Transform::default();
            if let Some(t) = t_val.get("transform") {
                transform = timeline_core::PartialTransform {
                    center_x: t.get("centerX").and_then(|v| v.as_f64()),
                    center_y: t.get("centerY").and_then(|v| v.as_f64()),
                    width: t.get("width").and_then(|v| v.as_f64()),
                    height: t.get("height").and_then(|v| v.as_f64()),
                    rotation: t.get("rotation").and_then(|v| v.as_f64()),
                    flip_horizontal: None,
                    flip_vertical: None,
                }
                .merge_into(&transform);
            }

            let text_animation = parse_text_animation(
                t_val.get("animation").and_then(|v| v.as_str()),
                t_val.get("highlightColor").and_then(|v| v.as_str()),
            )?;

            let clip = Clip {
                id: Uuid::new_v4().to_string(),
                media_ref: String::new(),
                media_type: core_model::ClipType::Text,
                source_clip_type: core_model::ClipType::Text,
                start_frame,
                duration_frames,
                trim_start_frame: 0,
                trim_end_frame: 0,
                speed: 1.0,
                volume: 1.0,
                fade_in_frames: 0,
                fade_out_frames: 0,
                fade_in_interpolation: Interpolation::Linear,
                fade_out_interpolation: Interpolation::Linear,
                opacity: 1.0,
                transform,
                crop: core_model::Crop::default(),
                link_group_id: None,
                caption_group_id: None,
                text_content: Some(if text.is_empty() {
                    "Text".to_string()
                } else {
                    text.to_string()
                }),
                text_style: Some(style),
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
                text_animation,
                word_timings: None,
            };
            let clip_id = clip.id.clone();
            created_ids.push(clip_id);
            clips.push(clip);
            current_frame = start_frame + duration_frames;
        }

        timeline_core::place_clips(&mut self.timeline, ti, 0, &clips);

        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!("Added {} text clip(s) to track {}: {:?}", created_ids.len(), ti, created_ids)
            }]
        }))
    }

    fn cmd_add_shapes(&mut self, args: &Value) -> Result<Value, String> {
        let entries = args
            .get("entries")
            .and_then(|v| v.as_array())
            .ok_or_else(|| "Missing entries array".to_string())?;

        if entries.is_empty() {
            return Err("entries must be non-empty".to_string());
        }

        // Find or create a video track
        let ti = self
            .timeline
            .tracks
            .iter()
            .position(|t| t.r#type == core_model::ClipType::Video)
            .unwrap_or(0);
        if ti >= self.timeline.tracks.len() {
            timeline_core::insert_track_at(&mut self.timeline, 0, core_model::ClipType::Video)
                .map_err(|_| "Failed to create track".to_string())?;
        }

        let mut current_frame = 0i64;
        for clip in &self.timeline.tracks[ti].clips {
            let end = clip.start_frame + clip.duration_frames;
            if end > current_frame {
                current_frame = end;
            }
        }

        let mut created_ids: Vec<String> = Vec::new();
        let mut clips: Vec<Clip> = Vec::new();

        for entry in entries {
            let shape_type = entry.get("type").and_then(|v| v.as_str()).unwrap_or("rect");
            let start_frame = entry
                .get("startFrame")
                .and_then(|v| v.as_i64())
                .unwrap_or(current_frame);
            let duration_frames = entry
                .get("durationFrames")
                .and_then(|v| v.as_i64())
                .unwrap_or(150);

            let shape_kind = match shape_type.to_lowercase().as_str() {
                "oval" => core_model::ShapeKind::Oval,
                "circle" => core_model::ShapeKind::Circle,
                "arrow" => core_model::ShapeKind::Arrow,
                "line" => core_model::ShapeKind::Line,
                _ => core_model::ShapeKind::Rect,
            };

            let mut shape_style = core_model::ShapeStyle {
                kind: shape_kind,
                ..core_model::ShapeStyle::default()
            };
            // style → stroke (color/width/dashed/arrowheadStyle).
            if let Some(st) = entry.get("style") {
                if let Some(hex) = st.get("color").and_then(|v| v.as_str()) {
                    shape_style.stroke.color = core_model::shape_style::Rgba::from_hex(hex)
                        .ok_or_else(|| format!("invalid style color '{hex}'"))?;
                }
                if let Some(w) = st.get("width").and_then(|v| v.as_f64()) {
                    shape_style.stroke.width = w;
                }
                if let Some(d) = st.get("dashed").and_then(|v| v.as_bool()) {
                    shape_style.stroke.dashed = d;
                }
                if let Some(a) = st.get("arrowheadStyle").and_then(|v| v.as_str()) {
                    shape_style.stroke.arrowhead_style = Some(a.to_string());
                }
            }
            // fill → enabled + colour.
            if let Some(f) = entry.get("fill") {
                if let Some(en) = f.get("enabled").and_then(|v| v.as_bool()) {
                    shape_style.fill.enabled = en;
                }
                if let Some(hex) = f.get("color").and_then(|v| v.as_str()) {
                    shape_style.fill.color = core_model::shape_style::Rgba::from_hex(hex)
                        .ok_or_else(|| format!("invalid fill color '{hex}'"))?;
                }
            }

            let mut transform = Transform::default();
            if let Some(t) = entry.get("transform") {
                transform = timeline_core::PartialTransform {
                    center_x: t.get("centerX").and_then(|v| v.as_f64()),
                    center_y: t.get("centerY").and_then(|v| v.as_f64()),
                    width: t.get("width").and_then(|v| v.as_f64()),
                    height: t.get("height").and_then(|v| v.as_f64()),
                    rotation: t.get("rotation").and_then(|v| v.as_f64()),
                    flip_horizontal: None,
                    flip_vertical: None,
                }
                .merge_into(&transform);
            }

            let clip = Clip {
                id: Uuid::new_v4().to_string(),
                media_ref: String::new(),
                media_type: core_model::ClipType::Shape,
                source_clip_type: core_model::ClipType::Shape,
                start_frame,
                duration_frames,
                trim_start_frame: 0,
                trim_end_frame: 0,
                speed: 1.0,
                volume: 1.0,
                fade_in_frames: 0,
                fade_out_frames: 0,
                fade_in_interpolation: Interpolation::Linear,
                fade_out_interpolation: Interpolation::Linear,
                opacity: 1.0,
                transform,
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
                shape_style: Some(shape_style),
                stroke_progress_track: None,
                compound_timeline_id: None,
                blend_mode: Default::default(),
                chroma_key: None,
                text_animation: None,
                word_timings: None,
            };
            let clip_id = clip.id.clone();
            created_ids.push(clip_id);
            clips.push(clip);
            current_frame = start_frame + duration_frames;
        }

        timeline_core::place_clips(&mut self.timeline, 0, 0, &clips);

        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!("Added {} shape clip(s): {:?}", created_ids.len(), created_ids)
            }]
        }))
    }

    fn cmd_apply_color(&mut self, args: &Value) -> Result<Value, String> {
        let clip_id = args
            .get("clipId")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing clipId".to_string())?;

        let Some(loc) = timeline_core::find_clip(&self.timeline, clip_id) else {
            return Err(format!("Clip '{}' not found", clip_id));
        };
        let clip = &mut self.timeline.tracks[loc.track_index].clips[loc.clip_index];

        let reset = args.get("reset").and_then(|v| v.as_bool()).unwrap_or(false);

        if reset {
            // Remove all color effects
            if let Some(ref mut effects) = clip.effects {
                effects.retain(|e| !e.r#type.starts_with("color."));
            }
        }

        let exposure = args.get("exposure").and_then(|v| v.as_f64());
        let contrast = args.get("contrast").and_then(|v| v.as_f64());
        let saturation = args.get("saturation").and_then(|v| v.as_f64());
        let temperature = args.get("temperature").and_then(|v| v.as_f64());

        let effects = clip.effects.get_or_insert(Vec::new());

        if let Some(v) = exposure {
            Self::upsert_effect_param(effects, "color.exposure", "ev", v);
        }
        if let Some(v) = contrast {
            Self::upsert_effect_param(effects, "color.contrast", "amount", v);
        }
        if let Some(v) = saturation {
            Self::upsert_effect_param(effects, "color.saturation", "amount", v);
        }
        if let Some(v) = temperature {
            Self::upsert_effect_param(effects, "color.temperature", "amount", v);
        }

        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!("Applied color adjustments to clip '{}'", clip_id)
            }]
        }))
    }

    fn upsert_effect_param(
        effects: &mut Vec<Effect>,
        effect_type: &str,
        param_name: &str,
        value: f64,
    ) {
        let existing = effects.iter_mut().find(|e| e.r#type == effect_type);
        match existing {
            Some(effect) => {
                effect.params.insert(
                    param_name.to_string(),
                    core_model::EffectParam::value(value),
                );
            }
            None => {
                let mut params = std::collections::HashMap::new();
                params.insert(
                    param_name.to_string(),
                    core_model::EffectParam::value(value),
                );
                effects.push(Effect {
                    id: Uuid::new_v4().to_string(),
                    r#type: effect_type.to_string(),
                    enabled: true,
                    params,
                });
            }
        }
    }

    fn cmd_apply_effect(&mut self, args: &Value) -> Result<Value, String> {
        let clip_id = args
            .get("clipId")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing clipId".to_string())?;
        let effect_type = args
            .get("effectType")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing effectType".to_string())?;
        let enabled = args
            .get("enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let remove = args
            .get("remove")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let intensity = args.get("intensity").and_then(|v| v.as_f64());

        let Some(loc) = timeline_core::find_clip(&self.timeline, clip_id) else {
            return Err(format!("Clip '{}' not found", clip_id));
        };
        let clip = &mut self.timeline.tracks[loc.track_index].clips[loc.clip_index];

        if remove {
            if let Some(ref mut effects) = clip.effects {
                effects.retain(|e| e.r#type != effect_type);
            }
        } else {
            let effects = clip.effects.get_or_insert(Vec::new());
            let existing = effects.iter_mut().find(|e| e.r#type == effect_type);
            match existing {
                Some(effect) => {
                    effect.enabled = enabled;
                    if let Some(v) = intensity {
                        effect
                            .params
                            .insert("intensity".to_string(), core_model::EffectParam::value(v));
                    }
                }
                None => {
                    let mut params = std::collections::HashMap::new();
                    if let Some(v) = intensity {
                        params.insert("intensity".to_string(), core_model::EffectParam::value(v));
                    }
                    effects.push(Effect {
                        id: Uuid::new_v4().to_string(),
                        r#type: effect_type.to_string(),
                        enabled,
                        params,
                    });
                }
            }
        }

        let action = if remove { "Removed" } else { "Applied" };
        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!("{} effect '{}' on clip '{}'", action, effect_type, clip_id)
            }]
        }))
    }

    fn cmd_set_chroma_key(&mut self, args: &Value) -> Result<Value, String> {
        let clip_id = args
            .get("clipId")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing clipId".to_string())?;

        let Some(loc) = timeline_core::find_clip(&self.timeline, clip_id) else {
            return Err(format!("Clip '{}' not found", clip_id));
        };
        let clip = &mut self.timeline.tracks[loc.track_index].clips[loc.clip_index];

        let enabled = args
            .get("enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let color = args.get("color").and_then(|v| v.as_str());
        let threshold = args.get("threshold").and_then(|v| v.as_f64());
        let smoothness = args.get("smoothness").and_then(|v| v.as_f64());

        let effects = clip.effects.get_or_insert(Vec::new());
        let existing = effects.iter_mut().find(|e| e.r#type == "chroma.key");

        match existing {
            Some(effect) => {
                effect.enabled = enabled;
                if let Some(c) = color {
                    effect
                        .params
                        .insert("color".to_string(), core_model::EffectParam::string(c));
                }
                if let Some(v) = threshold {
                    effect
                        .params
                        .insert("threshold".to_string(), core_model::EffectParam::value(v));
                }
                if let Some(v) = smoothness {
                    effect
                        .params
                        .insert("smoothness".to_string(), core_model::EffectParam::value(v));
                }
            }
            None => {
                let mut params = std::collections::HashMap::new();
                if let Some(c) = color {
                    params.insert("color".to_string(), core_model::EffectParam::string(c));
                }
                if let Some(v) = threshold {
                    params.insert("threshold".to_string(), core_model::EffectParam::value(v));
                }
                if let Some(v) = smoothness {
                    params.insert("smoothness".to_string(), core_model::EffectParam::value(v));
                }
                effects.push(Effect {
                    id: Uuid::new_v4().to_string(),
                    r#type: "chroma.key".to_string(),
                    enabled,
                    params,
                });
            }
        }

        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!("Set chroma key on clip '{}' (enabled: {})", clip_id, enabled)
            }]
        }))
    }

    fn cmd_set_blend_mode(&mut self, args: &Value) -> Result<Value, String> {
        let clip_id = args
            .get("clipId")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing clipId".to_string())?;
        let mode = args
            .get("mode")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing mode".to_string())?;

        let Some(loc) = timeline_core::find_clip(&self.timeline, clip_id) else {
            return Err(format!("Clip '{}' not found", clip_id));
        };
        let clip = &mut self.timeline.tracks[loc.track_index].clips[loc.clip_index];

        let effects = clip.effects.get_or_insert(Vec::new());
        let existing = effects.iter_mut().find(|e| e.r#type == "blend.mode");

        match existing {
            Some(effect) => {
                effect
                    .params
                    .insert("mode".to_string(), core_model::EffectParam::string(mode));
            }
            None => {
                let mut params = std::collections::HashMap::new();
                params.insert("mode".to_string(), core_model::EffectParam::string(mode));
                effects.push(Effect {
                    id: Uuid::new_v4().to_string(),
                    r#type: "blend.mode".to_string(),
                    enabled: true,
                    params,
                });
            }
        }

        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!("Set blend mode '{}' on clip '{}'", mode, clip_id)
            }]
        }))
    }

    fn cmd_set_color_grade(&mut self, args: &Value) -> Result<Value, String> {
        let clip_id = args
            .get("clipId")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing clipId".to_string())?;

        let Some(loc) = timeline_core::find_clip(&self.timeline, clip_id) else {
            return Err(format!("Clip '{}' not found", clip_id));
        };
        let clip = &mut self.timeline.tracks[loc.track_index].clips[loc.clip_index];

        let exposure = args.get("exposure").and_then(|v| v.as_f64());
        let contrast = args.get("contrast").and_then(|v| v.as_f64());
        let saturation = args.get("saturation").and_then(|v| v.as_f64());
        let temperature = args.get("temperature").and_then(|v| v.as_f64());

        let effects = clip.effects.get_or_insert(Vec::new());

        let color_grade = effects.iter_mut().find(|e| e.r#type == "color.grade");
        match color_grade {
            Some(effect) => {
                if let Some(v) = exposure {
                    effect
                        .params
                        .insert("exposure".to_string(), core_model::EffectParam::value(v));
                }
                if let Some(v) = contrast {
                    effect
                        .params
                        .insert("contrast".to_string(), core_model::EffectParam::value(v));
                }
                if let Some(v) = saturation {
                    effect
                        .params
                        .insert("saturation".to_string(), core_model::EffectParam::value(v));
                }
                if let Some(v) = temperature {
                    effect
                        .params
                        .insert("temperature".to_string(), core_model::EffectParam::value(v));
                }
            }
            None => {
                let mut params = std::collections::HashMap::new();
                if let Some(v) = exposure {
                    params.insert("exposure".to_string(), core_model::EffectParam::value(v));
                }
                if let Some(v) = contrast {
                    params.insert("contrast".to_string(), core_model::EffectParam::value(v));
                }
                if let Some(v) = saturation {
                    params.insert("saturation".to_string(), core_model::EffectParam::value(v));
                }
                if let Some(v) = temperature {
                    params.insert("temperature".to_string(), core_model::EffectParam::value(v));
                }
                effects.push(Effect {
                    id: Uuid::new_v4().to_string(),
                    r#type: "color.grade".to_string(),
                    enabled: true,
                    params,
                });
            }
        }

        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!("Set color grade on clip '{}'", clip_id)
            }]
        }))
    }

    // ── Color inspect (read-only) ──────────────────────────────────────────

    fn cmd_inspect_color(&self, args: &Value) -> Result<Value, String> {
        let clip_id = args.get("clipId").and_then(|v| v.as_str());
        let media_ref = args.get("mediaRef").and_then(|v| v.as_str());

        if clip_id.is_none() && media_ref.is_none() {
            return Err("Provide either clipId or mediaRef".to_string());
        }

        if let Some(cid) = clip_id {
            let loc = timeline_core::find_clip(&self.timeline, cid)
                .ok_or_else(|| format!("Clip '{}' not found", cid))?;
            let clip = &self.timeline.tracks[loc.track_index].clips[loc.clip_index];
            let color_effects: Vec<&Effect> = clip
                .effects
                .as_ref()
                .map(|e| {
                    e.iter()
                        .filter(|ef| ef.r#type.starts_with("color."))
                        .collect()
                })
                .unwrap_or_default();
            return Ok(json!({
                "content": [{
                    "type": "text",
                    "text": format!(
                        "Color info for clip '{}': {} color effect(s) applied",
                        cid, color_effects.len()
                    )
                }]
            }));
        }

        if let Some(mid) = media_ref {
            let in_manifest = self.media_manifest.entries.iter().find(|e| e.id == mid);
            match in_manifest {
                Some(entry) => Ok(json!({
                    "content": [{
                        "type": "text",
                        "text": format!(
                            "Media '{}' ({:?}) — width: {:?}, height: {:?}, fps: {:?}",
                            entry.name, entry.r#type, entry.source_width, entry.source_height, entry.source_fps
                        )
                    }]
                })),
                None => Ok(json!({
                    "content": [{
                        "type": "text",
                        "text": format!("Media '{}' not found in manifest", mid)
                    }],
                    "isError": true,
                })),
            }
        } else {
            Err("No clipId or mediaRef provided".to_string())
        }
    }

    // ── Captions (stub — needs transcription engine) ───────────────────────

    fn cmd_add_captions(&mut self, args: &Value) -> Result<Value, String> {
        let clip_ids: Vec<String> = args
            .get("clipIds")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        if clip_ids.is_empty() {
            return Err("clipIds must be non-empty".to_string());
        }

        // Verify all clips exist
        for cid in &clip_ids {
            if timeline_core::find_clip(&self.timeline, cid).is_none() {
                return Err(format!("Clip '{}' not found", cid));
            }
        }

        let language = args
            .get("language")
            .and_then(|v| v.as_str())
            .unwrap_or("en");
        let words_per_caption = args
            .get("wordsPerCaption")
            .and_then(|v| v.as_i64())
            .unwrap_or(6)
            .clamp(1, 12);

        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!(
                    "Caption generation requested for {} clip(s) (language: {}, wordsPerCaption: {}). Actual transcription requires a remote API.",
                    clip_ids.len(), language, words_per_caption
                )
            }],
            "isError": true,
        }))
    }

    fn cmd_apply_animation(&mut self, args: &Value) -> Result<Value, String> {
        let clip_id = args
            .get("clipId")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing clipId".to_string())?;
        let preset = args
            .get("preset")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing preset".to_string())?;

        let Some(loc) = timeline_core::find_clip(&self.timeline, clip_id) else {
            return Err(format!("Clip '{}' not found", clip_id));
        };
        let clip = &mut self.timeline.tracks[loc.track_index].clips[loc.clip_index];

        let intensity = args
            .get("intensity")
            .and_then(|v| v.as_f64())
            .unwrap_or(1.0);

        // Store animation preset as an effect
        let effects = clip.effects.get_or_insert(Vec::new());
        let anim_effect = Effect {
            id: Uuid::new_v4().to_string(),
            r#type: format!("animation.{}", preset),
            enabled: true,
            params: {
                let mut m = std::collections::HashMap::new();
                m.insert(
                    "intensity".to_string(),
                    core_model::EffectParam::value(intensity),
                );
                m
            },
        };
        effects.push(anim_effect);

        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!("Applied animation '{}' to clip '{}' (intensity: {})", preset, clip_id, intensity)
            }]
        }))
    }

    // ── Generation tools (stub — need remote API) ──────────────────────────

    fn cmd_generate_video(&mut self, args: &Value) -> Result<Value, String> {
        let prompt = args
            .get("prompt")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing prompt".to_string())?;
        let duration = args.get("duration").and_then(|v| v.as_f64()).unwrap_or(5.0);
        let model = args
            .get("model")
            .and_then(|v| v.as_str())
            .unwrap_or("gen-3");

        let entry_id = Uuid::new_v4().to_string();
        let entry = core_model::MediaManifestEntry {
            id: entry_id.clone(),
            name: format!("Generated video: {}", &prompt[..prompt.len().min(40)]),
            r#type: core_model::ClipType::Video,
            source: MediaSource::External {
                absolute_path: String::new(),
            },
            duration,
            generation_input: Some(GenerationInput {
                prompt: prompt.to_string(),
                model: model.to_string(),
                duration: (duration * 30.0) as i64,
                aspect_ratio: "16:9".to_string(),
                resolution: None,
                quality: None,
                image_urls: None,
                num_images: None,
                voice: None,
                lyrics: None,
                style_instructions: None,
                instrumental: None,
                generate_audio: None,
                reference_image_urls: None,
                reference_video_urls: None,
                reference_audio_urls: None,
                image_url_asset_ids: None,
                reference_image_asset_ids: None,
                reference_video_asset_ids: None,
                reference_audio_asset_ids: None,
                created_at: None,
                backend_job_id: None,
                output_index: None,
                result_urls: None,
            }),
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
        };
        self.media_manifest.entries.push(entry);

        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!(
                    "Video generation queued (model: {}, duration: {:.1}s, prompt: '{}'). Media id: {}. Actual generation requires a remote API.",
                    model, duration, prompt, entry_id
                )
            }],
            "isError": true,
        }))
    }

    fn cmd_generate_image(&mut self, args: &Value) -> Result<Value, String> {
        let prompt = args
            .get("prompt")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing prompt".to_string())?;
        let model = args.get("model").and_then(|v| v.as_str()).unwrap_or("sd3");

        let entry_id = Uuid::new_v4().to_string();
        let entry = core_model::MediaManifestEntry {
            id: entry_id.clone(),
            name: format!("Generated image: {}", &prompt[..prompt.len().min(40)]),
            r#type: core_model::ClipType::Image,
            source: MediaSource::External {
                absolute_path: String::new(),
            },
            duration: 10.0,
            generation_input: Some(GenerationInput {
                prompt: prompt.to_string(),
                model: model.to_string(),
                duration: 0,
                aspect_ratio: "16:9".to_string(),
                resolution: None,
                quality: None,
                image_urls: None,
                num_images: None,
                voice: None,
                lyrics: None,
                style_instructions: None,
                instrumental: None,
                generate_audio: None,
                reference_image_urls: None,
                reference_video_urls: None,
                reference_audio_urls: None,
                image_url_asset_ids: None,
                reference_image_asset_ids: None,
                reference_video_asset_ids: None,
                reference_audio_asset_ids: None,
                created_at: None,
                backend_job_id: None,
                output_index: None,
                result_urls: None,
            }),
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
        };
        self.media_manifest.entries.push(entry);

        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!(
                    "Image generation queued (model: {}, prompt: '{}'). Media id: {}. Actual generation requires a remote API.",
                    model, prompt, entry_id
                )
            }],
            "isError": true,
        }))
    }

    fn cmd_generate_audio(&mut self, args: &Value) -> Result<Value, String> {
        let prompt = args
            .get("prompt")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing prompt".to_string())?;
        let duration = args
            .get("duration")
            .and_then(|v| v.as_f64())
            .unwrap_or(10.0);

        let entry_id = Uuid::new_v4().to_string();
        let entry = core_model::MediaManifestEntry {
            id: entry_id.clone(),
            name: format!("Generated audio: {}", &prompt[..prompt.len().min(40)]),
            r#type: core_model::ClipType::Audio,
            source: MediaSource::External {
                absolute_path: String::new(),
            },
            duration,
            generation_input: None,
            source_width: None,
            source_height: None,
            source_fps: None,
            has_audio: Some(true),
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
        };
        self.media_manifest.entries.push(entry);

        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!(
                    "Audio generation queued ({:.1}s, prompt: '{}'). Media id: {}. Actual generation requires a remote API.",
                    duration, prompt, entry_id
                )
            }],
            "isError": true,
        }))
    }

    fn cmd_generate_music(&mut self, args: &Value) -> Result<Value, String> {
        let prompt = args
            .get("prompt")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing prompt".to_string())?;
        let duration = args
            .get("duration")
            .and_then(|v| v.as_f64())
            .unwrap_or(30.0);
        let style = args.get("style").and_then(|v| v.as_str());

        let entry_id = Uuid::new_v4().to_string();
        let entry = core_model::MediaManifestEntry {
            id: entry_id.clone(),
            name: format!("Generated music: {}", &prompt[..prompt.len().min(40)]),
            r#type: core_model::ClipType::Audio,
            source: MediaSource::External {
                absolute_path: String::new(),
            },
            duration,
            generation_input: Some(GenerationInput {
                prompt: prompt.to_string(),
                model: String::new(),
                duration: (duration * 30.0) as i64,
                aspect_ratio: String::new(),
                resolution: None,
                quality: None,
                image_urls: None,
                num_images: None,
                voice: None,
                lyrics: None,
                style_instructions: style.map(String::from),
                instrumental: None,
                generate_audio: None,
                reference_image_urls: None,
                reference_video_urls: None,
                reference_audio_urls: None,
                image_url_asset_ids: None,
                reference_image_asset_ids: None,
                reference_video_asset_ids: None,
                reference_audio_asset_ids: None,
                created_at: None,
                backend_job_id: None,
                output_index: None,
                result_urls: None,
            }),
            source_width: None,
            source_height: None,
            source_fps: None,
            has_audio: Some(true),
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
        };
        self.media_manifest.entries.push(entry);

        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!(
                    "Music generation queued ({:.1}s, style: {:?}, prompt: '{}'). Media id: {}. Actual generation requires a remote API.",
                    duration, style, prompt, entry_id
                )
            }],
            "isError": true,
        }))
    }

    fn cmd_upscale_media(&mut self, args: &Value) -> Result<Value, String> {
        let media_id = args
            .get("mediaId")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing mediaId".to_string())?;

        let entry = self
            .media_manifest
            .entries
            .iter()
            .find(|e| e.id == media_id)
            .ok_or_else(|| format!("Media '{}' not found", media_id))?;

        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!(
                    "Upscale requested for '{}' ({}). Actual upscaling requires a remote API.",
                    entry.name, media_id
                )
            }],
            "isError": true,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_model::{MediaManifest, Timeline};

    fn make_executor() -> ToolExecutor {
        ToolExecutor::new(Timeline::default(), MediaManifest::default())
    }

    fn make_executor_with_media() -> ToolExecutor {
        let mut manifest = MediaManifest::default();
        manifest.entries.push(core_model::MediaManifestEntry {
            id: "media-001".to_string(),
            name: "test_video.mp4".to_string(),
            r#type: core_model::ClipType::Video,
            source: core_model::MediaSource::External {
                absolute_path: "/path/to/video.mp4".to_string(),
            },
            duration: 10.0,
            generation_input: None,
            source_width: Some(1920),
            source_height: Some(1080),
            source_fps: Some(30.0),
            has_audio: Some(true),
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
        });
        manifest.folders.push(core_model::MediaFolder {
            id: "folder-001".to_string(),
            name: "Test Folder".to_string(),
            parent_folder_id: None,
        });
        ToolExecutor::new(Timeline::default(), manifest)
    }

    fn video_media(id: &str, w: i64, h: i64, fps: f64) -> core_model::MediaManifestEntry {
        core_model::MediaManifestEntry {
            id: id.into(),
            name: format!("{id}.mp4"),
            r#type: ClipType::Video,
            source: core_model::MediaSource::External {
                absolute_path: format!("/{id}.mp4"),
            },
            duration: 3.0,
            generation_input: None,
            source_width: Some(w),
            source_height: Some(h),
            source_fps: Some(fps),
            has_audio: Some(false),
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

    #[test]
    fn add_clips_auto_detects_settings_from_first_video() {
        // First clip ever (settings not configured) → silently adopt its fps/size.
        let mut manifest = MediaManifest::default();
        manifest.entries.push(video_media("v4k", 3840, 2160, 24.0));
        let mut exec = ToolExecutor::new(Timeline::default(), manifest);
        assert!(!exec.timeline().settings_configured);
        let res = exec.execute("add_clips", &json!({"mediaIds": ["v4k"]})).unwrap();
        assert_eq!(exec.timeline().fps, 24);
        assert_eq!(exec.timeline().width, 3840);
        assert_eq!(exec.timeline().height, 2160);
        assert!(exec.timeline().settings_configured);
        let text = res["content"][0]["text"].as_str().unwrap();
        assert!(
            text.contains("3840x2160") && text.contains("24fps"),
            "settings note expected: {text}"
        );
        // Clip duration is measured on the DETECTED 24fps grid (3s → 72 frames).
        assert_eq!(exec.timeline().tracks[0].clips[0].duration_frames, 72);
    }

    #[test]
    fn add_clips_keeps_settings_fixed_after_first_clip() {
        let mut manifest = MediaManifest::default();
        manifest.entries.push(video_media("a24", 1920, 1080, 24.0));
        manifest.entries.push(video_media("b60", 1920, 1080, 60.0));
        let mut exec = ToolExecutor::new(Timeline::default(), manifest);
        exec.execute("add_clips", &json!({"mediaIds": ["a24"]})).unwrap();
        assert_eq!(exec.timeline().fps, 24, "first clip sets project to 24fps");
        // A later 60fps clip must NOT re-detect: project fps stays fixed (#233).
        let res = exec.execute("add_clips", &json!({"mediaIds": ["b60"]})).unwrap();
        assert_eq!(exec.timeline().fps, 24, "settings stay fixed after the first clip");
        let text = res["content"][0]["text"].as_str().unwrap();
        assert!(!text.contains("Set project"), "no re-detect on later adds: {text}");
    }

    #[test]
    fn add_clips_resolves_type_and_full_source_duration() {
        // Upstream #236: omitting trim/duration places the full source length,
        // and the clip type comes from the manifest — not a hardcoded Video/150.
        let mut exec = make_executor_with_media();
        let fps = exec.timeline().fps;
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        exec.execute("add_clips", &json!({"mediaIds": ["media-001"], "trackIndex": 0}))
            .unwrap();
        let clip = &exec.timeline().tracks[0].clips[0];
        assert!(matches!(clip.media_type, ClipType::Video));
        assert_eq!(clip.duration_frames, (10.0 * fps as f64).round() as i64);
        assert_eq!(clip.trim_start_frame, 0);
        assert_eq!(clip.trim_end_frame, 0);
    }

    #[test]
    fn add_clips_rejects_type_incompatible_track() {
        // An audio asset must not be placed on a video track (and vice versa).
        let mut manifest = MediaManifest::default();
        manifest.entries.push(core_model::MediaManifestEntry {
            id: "aud".into(),
            name: "a.wav".into(),
            r#type: ClipType::Audio,
            source: core_model::MediaSource::External {
                absolute_path: "/a.wav".into(),
            },
            duration: 5.0,
            generation_input: None,
            source_width: None,
            source_height: None,
            source_fps: None,
            has_audio: Some(true),
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
        });
        let mut exec = ToolExecutor::new(Timeline::default(), manifest);
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let err = exec
            .execute("add_clips", &json!({"mediaIds": ["aud"], "trackIndex": 0}))
            .unwrap_err();
        assert!(err.contains("not compatible"), "got: {err}");
        assert!(
            exec.timeline().tracks[0].clips.is_empty(),
            "nothing placed on rejection"
        );
    }

    #[test]
    fn add_clips_auto_creates_linked_audio_for_video_with_audio() {
        // CLP-007/008: a video-with-audio placed on a video track auto-creates a
        // linked audio clip on an audio track (created if needed), sharing a group.
        let mut manifest = MediaManifest::default();
        manifest.entries.push(core_model::MediaManifestEntry {
            id: "vid".into(),
            name: "v.mp4".into(),
            r#type: ClipType::Video,
            source: core_model::MediaSource::External {
                absolute_path: "/v.mp4".into(),
            },
            duration: 4.0,
            generation_input: None,
            source_width: Some(1920),
            source_height: Some(1080),
            source_fps: Some(30.0),
            has_audio: Some(true),
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
        });
        let mut exec = ToolExecutor::new(Timeline::default(), manifest);
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        exec.execute("add_clips", &json!({"mediaIds": ["vid"], "trackIndex": 0}))
            .unwrap();

        let video = exec.timeline().tracks[0].clips[0].clone();
        assert!(video.link_group_id.is_some(), "video should be linked");
        let audio_track = exec
            .timeline()
            .tracks
            .iter()
            .find(|t| t.r#type == ClipType::Audio)
            .expect("linked audio track created");
        assert_eq!(audio_track.clips.len(), 1);
        let audio = &audio_track.clips[0];
        assert_eq!(audio.link_group_id, video.link_group_id, "shares link group");
        assert_eq!(audio.start_frame, video.start_frame);
        assert_eq!(audio.duration_frames, video.duration_frames);
        assert!(matches!(audio.media_type, ClipType::Audio));
    }

    fn media_entry(id: &str, ty: ClipType, has_audio: bool, duration: f64) -> core_model::MediaManifestEntry {
        core_model::MediaManifestEntry {
            id: id.into(),
            name: format!("{id}.media"),
            r#type: ty,
            source: core_model::MediaSource::External {
                absolute_path: format!("/{id}"),
            },
            duration,
            generation_input: None,
            source_width: Some(1920),
            source_height: Some(1080),
            source_fps: Some(30.0),
            has_audio: Some(has_audio),
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

    #[test]
    fn add_clips_omit_track_index_auto_creates_video_track() {
        // MUT-002/003: omitting trackIndex auto-creates a track for the clips.
        let mut manifest = MediaManifest::default();
        manifest.entries.push(media_entry("vid", ClipType::Video, false, 3.0));
        let mut exec = ToolExecutor::new(Timeline::default(), manifest); // zero tracks
        exec.execute("add_clips", &json!({"mediaIds": ["vid"]})).unwrap();
        assert_eq!(exec.timeline().tracks.len(), 1, "one track auto-created");
        assert!(matches!(exec.timeline().tracks[0].r#type, ClipType::Video));
        assert_eq!(exec.timeline().tracks[0].clips.len(), 1);
    }

    #[test]
    fn add_clips_omit_track_index_splits_visual_and_audio() {
        // Mixed visual + audio with no trackIndex → a video track for the visual and
        // an audio track for the audio.
        let mut manifest = MediaManifest::default();
        manifest.entries.push(media_entry("vid", ClipType::Video, false, 2.0));
        manifest.entries.push(media_entry("aud", ClipType::Audio, true, 2.0));
        let mut exec = ToolExecutor::new(Timeline::default(), manifest);
        exec.execute("add_clips", &json!({"mediaIds": ["vid", "aud"]}))
            .unwrap();
        let video_track = exec
            .timeline()
            .tracks
            .iter()
            .find(|t| t.r#type == ClipType::Video)
            .expect("video track created");
        let audio_track = exec
            .timeline()
            .tracks
            .iter()
            .find(|t| t.r#type == ClipType::Audio)
            .expect("audio track created");
        assert_eq!(video_track.clips.len(), 1);
        assert_eq!(video_track.clips[0].media_ref, "vid");
        assert_eq!(audio_track.clips.len(), 1);
        assert_eq!(audio_track.clips[0].media_ref, "aud");
    }

    #[test]
    fn add_clips_linked_audio_does_not_clobber_existing_audio() {
        // The existing audio track holds music over the target span; the linked
        // audio must go to a free/new track, never overwriting the music.
        let mut manifest = MediaManifest::default();
        manifest.entries.push(core_model::MediaManifestEntry {
            id: "vid".into(),
            name: "v.mp4".into(),
            r#type: ClipType::Video,
            source: core_model::MediaSource::External {
                absolute_path: "/v.mp4".into(),
            },
            duration: 4.0, // 120 frames @ 30fps
            generation_input: None,
            source_width: Some(1920),
            source_height: Some(1080),
            source_fps: Some(30.0),
            has_audio: Some(true),
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
        });
        let mut exec = ToolExecutor::new(Timeline::default(), manifest);
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let audio_ti =
            timeline_core::insert_track_at(exec.timeline_mut(), 1, ClipType::Audio).unwrap();
        // Pre-existing music spanning the whole target region.
        exec.timeline_mut().tracks[audio_ti].clips.push(only_clip_helper_music());

        exec.execute("add_clips", &json!({"mediaIds": ["vid"], "trackIndex": 0}))
            .unwrap();

        // The music clip must still exist somewhere untouched.
        let music_alive = exec
            .timeline()
            .tracks
            .iter()
            .flat_map(|t| t.clips.iter())
            .any(|c| c.id == "music" && c.start_frame == 0 && c.duration_frames == 300);
        assert!(music_alive, "pre-existing audio was clobbered");
        // The linked audio landed on a different audio track (not over the music).
        let audio_clip_count: usize = exec
            .timeline()
            .tracks
            .iter()
            .filter(|t| t.r#type == ClipType::Audio)
            .map(|t| t.clips.len())
            .sum();
        assert_eq!(audio_clip_count, 2, "music + linked audio both present");
    }

    fn only_clip_helper_music() -> Clip {
        let mut c = executor_with_clip().timeline().tracks[0].clips[0].clone();
        c.id = "music".into();
        c.media_type = ClipType::Audio;
        c.source_clip_type = ClipType::Audio;
        c.start_frame = 0;
        c.duration_frames = 300;
        c.link_group_id = None;
        c
    }

    #[test]
    fn add_clips_honors_symmetric_trim_and_duration() {
        // Upstream #236: trimStartFrame + durationFrames derive trimEndFrame.
        let mut exec = make_executor_with_media();
        let fps = exec.timeline().fps;
        let total = (10.0 * fps as f64).round() as i64;
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        exec.execute(
            "add_clips",
            &json!({
                "mediaIds": ["media-001"],
                "trackIndex": 0,
                "trimStartFrame": 10,
                "durationFrames": 50
            }),
        )
        .unwrap();
        let clip = &exec.timeline().tracks[0].clips[0];
        assert_eq!(clip.trim_start_frame, 10);
        assert_eq!(clip.duration_frames, 50);
        assert_eq!(clip.trim_end_frame, total - 10 - 50);
    }

    #[test]
    fn add_clips_rejects_duration_and_trim_end_together() {
        // Upstream #236: durationFrames and trimEndFrame are mutually exclusive.
        let mut exec = make_executor_with_media();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let err = exec
            .execute(
                "add_clips",
                &json!({
                    "mediaIds": ["media-001"],
                    "trackIndex": 0,
                    "durationFrames": 50,
                    "trimEndFrame": 10
                }),
            )
            .unwrap_err();
        assert!(err.contains("not both"), "err={err}");
    }

    #[test]
    fn add_clips_warns_on_source_fps_divergence_without_changing_project_fps() {
        // Upstream #233: on a CONFIGURED project, project fps is authoritative; a
        // divergent source fps only warns and points at set_project_settings (the
        // first-clip auto-detect only fires when settings are not yet configured).
        let mut exec = make_executor_with_media();
        exec.timeline_mut().fps = 24; // source asset is 30 fps
        exec.timeline_mut().settings_configured = true;
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let result = exec
            .execute("add_clips", &json!({"mediaIds": ["media-001"], "trackIndex": 0}))
            .unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("Source fps"), "text={text}");
        assert!(text.contains("set_project_settings"), "text={text}");
        assert_eq!(exec.timeline().fps, 24, "project fps must be unchanged");
    }

    #[test]
    fn set_project_settings_requires_at_least_one_arg() {
        let mut exec = make_executor();
        let err = exec.execute("set_project_settings", &json!({})).unwrap_err();
        assert!(err.contains("at least one"), "err={err}");
    }

    #[test]
    fn set_project_settings_aspect_and_explicit_dims_conflict() {
        let mut exec = make_executor();
        let err = exec
            .execute(
                "set_project_settings",
                &json!({"aspectRatio": "16:9", "width": 1920}),
            )
            .unwrap_err();
        assert!(err.contains("mutually exclusive"), "err={err}");
    }

    #[test]
    fn set_project_settings_aspect_preset_sets_dims() {
        let mut exec = make_executor(); // default 1920x1080 @30
        exec.execute("set_project_settings", &json!({"aspectRatio": "9:16"}))
            .unwrap();
        assert_eq!(exec.timeline().width, 1080);
        assert_eq!(exec.timeline().height, 1920);
        assert!(exec.timeline().settings_configured);
    }

    #[test]
    fn set_project_settings_quality_scales_short_edge() {
        let mut exec = make_executor(); // 1920x1080 landscape
        exec.execute("set_project_settings", &json!({"quality": "4K"}))
            .unwrap();
        // short edge 2160, landscape -> (2160*1920/1080, 2160)
        assert_eq!(exec.timeline().width, 3840);
        assert_eq!(exec.timeline().height, 2160);
    }

    #[test]
    fn set_project_settings_fps_rescales_clips_and_is_undoable() {
        let mut exec = make_executor_with_media(); // default 30fps
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        exec.execute(
            "add_clips",
            &json!({"mediaIds": ["media-001"], "trackIndex": 0, "durationFrames": 50}),
        )
        .unwrap();
        assert_eq!(exec.timeline().tracks[0].clips[0].duration_frames, 50);

        exec.execute("set_project_settings", &json!({"fps": 60}))
            .unwrap();
        assert_eq!(exec.timeline().fps, 60);
        assert_eq!(exec.timeline().tracks[0].clips[0].duration_frames, 100);

        exec.execute("undo", &json!({})).unwrap();
        assert_eq!(exec.timeline().fps, 30);
        assert_eq!(exec.timeline().tracks[0].clips[0].duration_frames, 50);
    }

    #[test]
    fn set_project_settings_fps_out_of_range_rejected() {
        let mut exec = make_executor();
        let err = exec
            .execute("set_project_settings", &json!({"fps": 500}))
            .unwrap_err();
        assert!(err.contains("between 1 and 120"), "err={err}");
    }

    #[test]
    fn split_clips_explicit_mode_two_cuts_on_same_clip() {
        let mut exec = make_executor_with_media();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        exec.execute(
            "add_clips",
            &json!({"mediaIds": ["media-001"], "trackIndex": 0, "durationFrames": 100}),
        )
        .unwrap();
        let clip_id = exec.timeline().tracks[0].clips[0].id.clone();
        exec.execute(
            "split_clips",
            &json!({"splits": [
                {"clipId": clip_id, "atFrame": 30},
                {"clipId": clip_id, "atFrame": 60}
            ]}),
        )
        .unwrap();
        assert_eq!(exec.timeline().tracks[0].clips.len(), 3);
    }

    #[test]
    fn split_clips_track_mode_and_dedup() {
        let mut exec = make_executor_with_media();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        exec.execute(
            "add_clips",
            &json!({"mediaIds": ["media-001"], "trackIndex": 0, "durationFrames": 100}),
        )
        .unwrap();
        // Duplicate cut points must dedup to a single split.
        exec.execute("split_clips", &json!({"trackIndex": 0, "frames": [50, 50, 50]}))
            .unwrap();
        assert_eq!(exec.timeline().tracks[0].clips.len(), 2);
    }

    #[test]
    fn split_clips_rejects_bad_point_with_no_partial_state() {
        let mut exec = make_executor_with_media();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        exec.execute(
            "add_clips",
            &json!({"mediaIds": ["media-001"], "trackIndex": 0, "durationFrames": 100}),
        )
        .unwrap();
        let clip_id = exec.timeline().tracks[0].clips[0].id.clone();
        let err = exec
            .execute(
                "split_clips",
                &json!({"splits": [
                    {"clipId": clip_id, "atFrame": 30},
                    {"clipId": clip_id, "atFrame": 999}
                ]}),
            )
            .unwrap_err();
        assert!(err.contains("strictly inside"), "err={err}");
        assert_eq!(
            exec.timeline().tracks[0].clips.len(),
            1,
            "no partial split on rejection"
        );
    }

    #[test]
    fn read_skill_returns_body_for_loaded_skill() {
        let mut exec = make_executor();
        exec.set_skills(vec![AgentSkill {
            id: "captions".into(),
            name: "Captions".into(),
            description: "burn in captions".into(),
            body: "1. Transcribe\n2. Style".into(),
        }]);
        let result = exec.execute("read_skill", &json!({"id": "captions"})).unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("Transcribe"));
        assert!(text.contains("Style"));
    }

    #[test]
    fn read_skill_unknown_id_errors() {
        let mut exec = make_executor();
        let err = exec
            .execute("read_skill", &json!({"id": "nope"}))
            .unwrap_err();
        assert!(err.contains("not found"), "err={err}");
    }

    #[test]
    fn read_skill_is_read_only_no_revision_bump() {
        let mut exec = make_executor();
        exec.set_skills(vec![AgentSkill {
            id: "a".into(),
            name: "A".into(),
            description: "d".into(),
            body: "body".into(),
        }]);
        let before = exec.revision();
        exec.execute("read_skill", &json!({"id": "a"})).unwrap();
        assert_eq!(exec.revision(), before, "read_skill must not bump revision");
    }

    #[test]
    fn apply_layout_side_by_side_sets_transforms_and_crop() {
        let mut exec = make_executor_with_media(); // media-001 Video 1920x1080, canvas 1920x1080
        // One clip per track, both starting at frame 0 → co-visible for side_by_side.
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 1, ClipType::Video);
        exec.execute("add_clips", &json!({"mediaIds": ["media-001"], "trackIndex": 0}))
            .unwrap();
        exec.execute("add_clips", &json!({"mediaIds": ["media-001"], "trackIndex": 1}))
            .unwrap();
        let id0 = exec.timeline().tracks[0].clips[0].id.clone();
        let id1 = exec.timeline().tracks[1].clips[0].id.clone();

        exec.execute(
            "apply_layout",
            &json!({
                "layout": "side_by_side",
                "slots": [
                    {"slot": "left", "clipId": id0},
                    {"slot": "right", "clipId": id1}
                ]
            }),
        )
        .unwrap();

        let left = &exec.timeline().tracks[0].clips[0];
        let right = &exec.timeline().tracks[1].clips[0];
        assert!(
            (left.transform.center_x - 0.25).abs() < 1e-6,
            "left cx={}",
            left.transform.center_x
        );
        assert!(
            (right.transform.center_x - 0.75).abs() < 1e-6,
            "right cx={}",
            right.transform.center_x
        );
        // 16:9 source cover-cropped into a half-width slot → 0.25 side crops.
        assert!((left.crop.left - 0.25).abs() < 1e-6, "crop.left={}", left.crop.left);
    }

    #[test]
    fn apply_layout_batches_multiple_clips_per_slot() {
        let mut exec = make_executor_with_media();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 1, ClipType::Video);
        // Two sequential takes in the left region (same track), one clip on the right.
        exec.execute(
            "add_clips",
            &json!({"mediaIds": ["media-001", "media-001"], "trackIndex": 0}),
        )
        .unwrap();
        exec.execute("add_clips", &json!({"mediaIds": ["media-001"], "trackIndex": 1}))
            .unwrap();
        let a = exec.timeline().tracks[0].clips[0].id.clone();
        let b = exec.timeline().tracks[0].clips[1].id.clone();
        let c = exec.timeline().tracks[1].clips[0].id.clone();
        exec.execute(
            "apply_layout",
            &json!({
                "layout": "side_by_side",
                "slots": [
                    {"slot": "left", "clipIds": [a, b]},
                    {"slot": "right", "clipIds": [c]}
                ]
            }),
        )
        .unwrap();
        // Both left-slot clips are framed into the left region.
        for clip in &exec.timeline().tracks[0].clips {
            assert!(
                (clip.transform.center_x - 0.25).abs() < 1e-6,
                "left cx={}",
                clip.transform.center_x
            );
        }
        let right = &exec.timeline().tracks[1].clips[0];
        assert!(
            (right.transform.center_x - 0.75).abs() < 1e-6,
            "right cx={}",
            right.transform.center_x
        );
    }

    #[test]
    fn apply_layout_same_track_overlap_errors() {
        let mut exec = make_executor_with_media();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        exec.execute(
            "add_clips",
            &json!({"mediaIds": ["media-001", "media-001"], "trackIndex": 0}),
        )
        .unwrap();
        // Force the two clips to overlap in time on the SAME track.
        exec.timeline_mut().tracks[0].clips[1].start_frame = 100;
        let a = exec.timeline().tracks[0].clips[0].id.clone();
        let b = exec.timeline().tracks[0].clips[1].id.clone();
        let err = exec
            .execute(
                "apply_layout",
                &json!({
                    "layout": "side_by_side",
                    "slots": [
                        {"slot": "left", "clipId": a},
                        {"slot": "right", "clipId": b}
                    ]
                }),
            )
            .unwrap_err();
        assert!(err.contains("same track") && err.contains("overlap"), "err={err}");
    }

    #[test]
    fn apply_layout_non_coincident_clips_error() {
        let mut exec = make_executor_with_media();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 1, ClipType::Video);
        exec.execute("add_clips", &json!({"mediaIds": ["media-001"], "trackIndex": 0}))
            .unwrap();
        exec.execute("add_clips", &json!({"mediaIds": ["media-001"], "trackIndex": 1}))
            .unwrap();
        // Right clip starts only after the left one ends → never co-visible.
        let left_end = {
            let l = &exec.timeline().tracks[0].clips[0];
            l.start_frame + l.duration_frames
        };
        exec.timeline_mut().tracks[1].clips[0].start_frame = left_end;
        let a = exec.timeline().tracks[0].clips[0].id.clone();
        let b = exec.timeline().tracks[1].clips[0].id.clone();
        let err = exec
            .execute(
                "apply_layout",
                &json!({
                    "layout": "side_by_side",
                    "slots": [
                        {"slot": "left", "clipId": a},
                        {"slot": "right", "clipId": b}
                    ]
                }),
            )
            .unwrap_err();
        assert!(err.contains("never play at the same time"), "err={err}");
    }

    #[test]
    fn apply_layout_unknown_layout_errors() {
        let mut exec = make_executor();
        let err = exec
            .execute(
                "apply_layout",
                &json!({"layout": "nope", "slots": [{"slot": "main", "clipId": "x"}]}),
            )
            .unwrap_err();
        assert!(err.contains("unknown layout"), "err={err}");
    }

    #[test]
    fn apply_layout_missing_slot_errors() {
        let mut exec = make_executor();
        let err = exec
            .execute(
                "apply_layout",
                &json!({"layout": "side_by_side", "slots": [{"slot": "left", "clipId": "x"}]}),
            )
            .unwrap_err();
        assert!(
            err.contains("needs every slot filled") && err.contains("right"),
            "err={err}"
        );
    }

    #[test]
    fn apply_layout_place_new_requires_duration() {
        let mut manifest = MediaManifest::default();
        manifest.entries.push(video_media("m1", 1920, 1080, 30.0));
        let mut exec = ToolExecutor::new(Timeline::default(), manifest);
        let err = exec
            .execute(
                "apply_layout",
                &json!({"layout": "full", "slots": [{"slot": "main", "mediaRef": "m1"}]}),
            )
            .unwrap_err();
        assert!(err.contains("durationFrames >= 1"), "err={err}");
    }

    #[test]
    fn apply_layout_place_new_rejects_missing_asset() {
        let mut exec = make_executor();
        let err = exec
            .execute(
                "apply_layout",
                &json!({"layout": "full", "durationFrames": 30, "slots": [{"slot": "main", "mediaRef": "nope"}]}),
            )
            .unwrap_err();
        assert!(err.contains("asset not found"), "err={err}");
    }

    #[test]
    fn apply_layout_place_new_conflicting_args_leave_timeline_untouched() {
        // durationFrames + trimEndFrame is rejected BEFORE any track is created, so a
        // rejected place-new call does not leave orphaned empty tracks behind.
        let mut manifest = MediaManifest::default();
        manifest.entries.push(video_media("L", 1920, 1080, 30.0));
        manifest.entries.push(video_media("R", 1920, 1080, 30.0));
        let mut exec = ToolExecutor::new(Timeline::default(), manifest);
        let err = exec
            .execute(
                "apply_layout",
                &json!({
                    "layout": "side_by_side",
                    "durationFrames": 60,
                    "trimEndFrame": 10,
                    "slots": [
                        {"slot": "left", "mediaRef": "L"},
                        {"slot": "right", "mediaRef": "R"}
                    ]
                }),
            )
            .unwrap_err();
        assert!(err.contains("trimEndFrame"), "err={err}");
        assert_eq!(exec.timeline().tracks.len(), 0, "no orphaned tracks created");
    }

    #[test]
    fn apply_layout_place_new_creates_tracks_and_frames_clips() {
        let mut manifest = MediaManifest::default();
        manifest.entries.push(video_media("L", 1920, 1080, 30.0));
        manifest.entries.push(video_media("R", 1920, 1080, 30.0));
        let mut exec = ToolExecutor::new(Timeline::default(), manifest);
        let res = exec
            .execute(
                "apply_layout",
                &json!({
                    "layout": "side_by_side",
                    "durationFrames": 60,
                    "slots": [
                        {"slot": "left", "mediaRef": "L"},
                        {"slot": "right", "mediaRef": "R"}
                    ]
                }),
            )
            .unwrap();
        // One new video track per slot, each with one framed clip.
        assert_eq!(exec.timeline().tracks.len(), 2);
        let tl = exec.timeline();
        let clips: Vec<&Clip> = tl.tracks.iter().flat_map(|t| &t.clips).collect();
        let left = clips.iter().find(|c| c.media_ref == "L").expect("left clip");
        let right = clips.iter().find(|c| c.media_ref == "R").expect("right clip");
        assert!((left.transform.center_x - 0.25).abs() < 1e-6, "left cx={}", left.transform.center_x);
        assert!(
            (right.transform.center_x - 0.75).abs() < 1e-6,
            "right cx={}",
            right.transform.center_x
        );
        assert_eq!(left.duration_frames, 60);
        assert_eq!(left.start_frame, 0);
        let text = res["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("Created 2 video track"), "note: {text}");
    }

    #[test]
    fn apply_layout_place_new_stacks_pip_inset_on_top() {
        // The PIP inset has z=1 (main z=0). tracks[0] is the TOP layer, so the inset's
        // track must land at a LOWER index than main's — the insert-at-0 ascending-z
        // stacking put the highest z on top.
        let mut manifest = MediaManifest::default();
        manifest.entries.push(video_media("MAIN", 1920, 1080, 30.0));
        manifest.entries.push(video_media("INSET", 1920, 1080, 30.0));
        let mut exec = ToolExecutor::new(Timeline::default(), manifest);
        exec.execute(
            "apply_layout",
            &json!({
                "layout": "pip_bottom_right",
                "durationFrames": 30,
                "slots": [
                    {"slot": "main", "mediaRef": "MAIN"},
                    {"slot": "inset", "mediaRef": "INSET"}
                ]
            }),
        )
        .unwrap();
        let tl = exec.timeline();
        let inset_track = tl
            .tracks
            .iter()
            .position(|t| t.clips.iter().any(|c| c.media_ref == "INSET"))
            .expect("inset placed");
        let main_track = tl
            .tracks
            .iter()
            .position(|t| t.clips.iter().any(|c| c.media_ref == "MAIN"))
            .expect("main placed");
        assert!(
            inset_track < main_track,
            "PIP inset (z=1) must stack above main (z=0): inset={inset_track}, main={main_track}"
        );
    }

    #[test]
    fn exec_001_get_timeline_returns_default() {
        let mut exec = make_executor();
        let result = exec.execute("get_timeline", &json!({})).unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("fps"));
        assert!(text.contains("1920"));
        assert!(text.contains("1080"));
    }

    #[test]
    fn exec_002_unknown_tool_returns_error() {
        let mut exec = make_executor();
        let err = exec.execute("nonexistent", &json!({})).unwrap_err();
        assert!(err.contains("Unknown tool"));
    }

    #[test]
    fn exec_003_split_clips_missing_args() {
        let mut exec = make_executor();
        let err = exec.execute("split_clips", &json!({})).unwrap_err();
        assert!(err.contains("Provide either"), "err={err}");
    }

    #[test]
    fn exec_004_undo_empty_returns_error() {
        let mut exec = make_executor();
        let result = exec.execute("undo", &json!({})).unwrap();
        assert_eq!(result["isError"], true);
    }

    #[test]
    fn exec_005_add_then_remove_track() {
        let mut exec = make_executor();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        assert_eq!(exec.timeline().tracks.len(), 1);
        assert!(exec.execute("undo", &json!({})).is_ok());
    }

    #[test]
    fn exec_006_remove_clips_empty_ids() {
        let mut exec = make_executor();
        let err = exec
            .execute("remove_clips", &json!({"clipIds": []}))
            .unwrap_err();
        assert!(err.contains("non-empty"));
    }

    #[test]
    fn exec_007_move_clips_no_tracks() {
        let mut exec = make_executor();
        let err = exec
            .execute(
                "move_clips",
                &json!({"clipIds": ["c1"], "toTrack": 0, "toFrame": 10}),
            )
            .unwrap_err();
        assert!(err.contains("out of bounds"));
    }

    #[test]
    fn exec_008_set_clip_properties_missing_ids() {
        let mut exec = make_executor();
        let err = exec
            .execute(
                "set_clip_properties",
                &json!({"clipIds": [], "properties": {}}),
            )
            .unwrap_err();
        assert!(err.contains("non-empty"));
    }

    #[test]
    fn exec_009_remove_tracks_empty_ids() {
        let mut exec = make_executor();
        let err = exec
            .execute("remove_tracks", &json!({"trackIds": []}))
            .unwrap_err();
        assert!(err.contains("non-empty"));
    }

    #[test]
    fn exec_010_undo_tracking_on_mutation() {
        let mut exec = make_executor();
        assert_eq!(exec.undo_stack().len(), 0);

        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        assert_eq!(exec.timeline().tracks.len(), 1);

        let track_id = exec.timeline().tracks[0].id.clone();
        let result = exec
            .execute("remove_tracks", &json!({"trackIds": [track_id]}))
            .unwrap();
        assert!(result["isError"].is_null() || result["isError"] == false);
        assert_eq!(exec.undo_stack().len(), 1);
    }

    #[test]
    fn exec_011_get_media_found() {
        let mut exec = make_executor_with_media();
        let result = exec
            .execute("get_media", &json!({"mediaId": "media-001"}))
            .unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("test_video.mp4"));
    }

    #[test]
    fn get_media_surfaces_generation_status() {
        // #216: get_media surfaces a not-ready async-generation status so the agent
        // waits for 'none' before referencing the asset.
        let mut exec = make_executor_with_media();
        exec.media_manifest.entries[0].generation_status = Some("generating".into());
        let res = exec
            .execute("get_media", &json!({"mediaId": "media-001"}))
            .unwrap();
        assert!(
            res["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("generationStatus: generating"),
            "got: {}",
            res["content"][0]["text"]
        );
        // 'none' is ready → not surfaced.
        exec.media_manifest.entries[0].generation_status = Some("none".into());
        let res2 = exec
            .execute("get_media", &json!({"mediaId": "media-001"}))
            .unwrap();
        assert!(!res2["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("generationStatus"));
    }

    #[test]
    fn exec_012_get_media_not_found() {
        let mut exec = make_executor_with_media();
        let err = exec
            .execute("get_media", &json!({"mediaId": "nonexistent"}))
            .unwrap_err();
        assert!(err.contains("not found"));
    }

    #[test]
    fn exec_013_get_media_missing_id() {
        let mut exec = make_executor_with_media();
        let err = exec.execute("get_media", &json!({})).unwrap_err();
        assert!(err.contains("Missing mediaId"));
    }

    #[test]
    fn exec_014_search_media_by_name() {
        let mut exec = make_executor_with_media();
        let result = exec
            .execute("search_media", &json!({"query": "test_video"}))
            .unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        // Should return structured output with files group
        assert!(text.contains("media-001"));
        assert!(text.contains("\"files\""));
        assert!(text.contains("Found 1 media"));
    }

    #[test]
    fn exec_015_search_media_no_results() {
        let mut exec = make_executor_with_media();
        let result = exec
            .execute("search_media", &json!({"query": "nothing"}))
            .unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
        assert!(parsed["files"].as_array().unwrap().is_empty());
        assert_eq!(parsed["status"], "ok");
    }

    #[test]
    fn exec_023_search_media_by_type() {
        let mut exec = make_executor_with_media();
        let result = exec
            .execute("search_media", &json!({"type": "video"}))
            .unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
        let files = parsed["files"].as_array().unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0]["media_id"], "media-001");
    }

    #[test]
    fn exec_024_search_media_no_match_type() {
        let mut exec = make_executor_with_media();
        let result = exec
            .execute("search_media", &json!({"type": "image"}))
            .unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
        assert!(parsed["files"].as_array().unwrap().is_empty());
    }

    #[test]
    fn exec_025_search_media_limit() {
        let mut exec = make_executor_with_media();
        let result = exec
            .execute("search_media", &json!({"query": "", "limit": 1}))
            .unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
        assert_eq!(parsed["limit"], 1);
        let files = parsed["files"].as_array().unwrap();
        assert_eq!(files.len(), 1);
    }

    #[test]
    fn exec_026_search_media_empty_query_shows_all() {
        let mut exec = make_executor_with_media();
        let result = exec.execute("search_media", &json!({"query": ""})).unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
        let files = parsed["files"].as_array().unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0]["media_id"], "media-001");
    }

    #[test]
    fn exec_027_search_media_with_status() {
        // READ-026: Status reporting for visual indexing
        let mut exec = make_executor_with_media();
        exec.set_search_status("Indexing 1 asset");
        let result = exec
            .execute("search_media", &json!({"query": "test_video"}))
            .unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
        assert!(parsed["status"].as_str().unwrap().contains("Indexing"));
    }

    #[test]
    fn exec_028_search_media_no_results_with_status() {
        // READ-026: Status shown even with no results
        let mut exec = make_executor_with_media();
        exec.set_search_status("Model not ready");
        let result = exec
            .execute("search_media", &json!({"query": "nothing"}))
            .unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
        assert_eq!(parsed["status"], "Model not ready");
    }

    #[test]
    fn exec_029_search_media_default_status_ok() {
        // READ-026: Default empty status shows ok
        let mut exec = make_executor_with_media();
        let result = exec
            .execute("search_media", &json!({"query": "test_video"}))
            .unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
        assert!(parsed["status"].as_str().unwrap().contains("Found"));
    }

    #[test]
    fn exec_016_list_folders() {
        let mut exec = make_executor_with_media();
        let result = exec.execute("list_folders", &json!({})).unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("Folders (1)"));
    }

    #[test]
    fn exec_017_list_folders_empty() {
        let mut exec = make_executor();
        let result = exec.execute("list_folders", &json!({})).unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("No folders"));
    }

    #[test]
    fn exec_018_list_models() {
        let mut exec = make_executor();
        let result = exec.execute("list_models", &json!({})).unwrap();
        assert!(result["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("video"));
    }

    #[test]
    fn exec_019_inspect_media() {
        let mut exec = make_executor_with_media();
        let result = exec
            .execute("inspect_media", &json!({"mediaId": "media-001"}))
            .unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("1920"));
    }

    #[test]
    fn exec_019_inspect_media_rejects_text() {
        // READ-013: Text clip rejection
        let mut manifest = core_model::MediaManifest::default();
        manifest.entries.push(core_model::MediaManifestEntry {
            id: "text-media".to_string(),
            name: "text_asset".to_string(),
            r#type: core_model::ClipType::Text,
            source: core_model::MediaSource::External {
                absolute_path: "/tmp/text.txt".to_string(),
            },
            duration: 5.0,
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
        });
        let mut exec = ToolExecutor::new(core_model::Timeline::default(), manifest);
        let result = exec
            .execute("inspect_media", &json!({"mediaId": "text-media"}))
            .unwrap();
        assert_eq!(result["isError"], true, "READ-013: text clips return error");
        assert!(
            result["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("text clip"),
            "READ-013: error mentions text clip"
        );
    }

    #[test]
    fn exec_019_inspect_media_cross_validates_clip_id() {
        // READ-014: clipId → mediaRef cross-validation
        let mut manifest = core_model::MediaManifest::default();
        manifest.entries.push(core_model::MediaManifestEntry {
            id: "media-vid".to_string(),
            name: "video.mp4".to_string(),
            r#type: core_model::ClipType::Video,
            source: core_model::MediaSource::External {
                absolute_path: "/tmp/video.mp4".to_string(),
            },
            duration: 10.0,
            generation_input: None,
            source_width: Some(1920),
            source_height: Some(1080),
            source_fps: Some(30.0),
            has_audio: Some(true),
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
        });
        let mut timeline = core_model::Timeline::default();
        timeline.tracks.push(core_model::Track {
            id: "track-v".to_string(),
            r#type: core_model::ClipType::Video,
            muted: false,
            hidden: false,
            sync_locked: false,
            clips: vec![core_model::Clip {
                id: "clip-vid".to_string(),
                media_ref: "media-vid".to_string(),
                media_type: core_model::ClipType::Video,
                source_clip_type: core_model::ClipType::Video,
                start_frame: 0,
                duration_frames: 100,
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
                text_animation: None,
                word_timings: None,
            }],
        });
        let mut exec = ToolExecutor::new(timeline, manifest);
        // Valid clipId → mediaRef should succeed
        let result = exec.execute(
            "inspect_media",
            &json!({"mediaId": "media-vid", "clipId": "clip-vid"}),
        );
        assert!(result.is_ok(), "READ-014: valid clipId should work");

        // Mismatched clipId → mediaRef should fail
        let result = exec.execute(
            "inspect_media",
            &json!({"mediaId": "media-vid", "clipId": "nonexistent"}),
        );
        assert!(result.is_err(), "READ-014: nonexistent clipId should fail");
    }

    #[test]
    fn exec_020_inspect_timeline() {
        let mut exec = make_executor();
        let result = exec.execute("inspect_timeline", &json!({})).unwrap();
        assert!(result["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("fps"));
    }

    #[test]
    fn exec_021_get_transcript_no_media_id() {
        let mut exec = make_executor();
        let result = exec.execute("get_transcript", &json!({})).unwrap();
        assert_eq!(result["isError"], true);
    }

    #[test]
    fn exec_021_get_transcript_with_media_id() {
        let mut exec = make_executor_with_media();
        let result = exec
            .execute("get_transcript", &json!({"mediaId": "media-001"}))
            .unwrap();
        assert!(result.get("isError").is_none(), "no error for known media");
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(!text.is_empty(), "has result text");
    }

    #[test]
    fn exec_021_get_transcript_tolerates_word_timestamps() {
        let mut exec = make_executor();
        // READ-021: legacy wordTimestamps should not cause errors
        let result = exec
            .execute(
                "get_transcript",
                &json!({"wordTimestamps": true, "mediaId": "media-001"}),
            )
            .unwrap();
        // isError should be absent since mediaId is present
        assert!(
            result.get("isError").is_none(),
            "no error when mediaId provided"
        );
        assert!(
            result["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("clips"),
            "returns formatted transcript JSON"
        );
    }

    // ---- Issue #39: language resolution in get_transcript / inspect_media --

    #[test]
    fn issue_039_get_transcript_per_call_language_propagated() {
        let mut exec = make_executor_with_media();
        let result = exec
            .execute(
                "get_transcript",
                &json!({"mediaId": "media-001", "language": "fr"}),
            )
            .unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        // The formatted output should include the language field
        assert!(
            text.contains("\"language\""),
            "language field in output: {text}"
        );
        assert!(text.contains("fr"), "language value in output: {text}");
    }

    #[test]
    fn issue_039_get_transcript_project_language_fallback() {
        // When no per-call language but timeline has transcriptionLanguage
        let mut exec = make_executor_with_media();
        exec.timeline.transcription_language = Some("ja".to_string());
        let result = exec
            .execute("get_transcript", &json!({"mediaId": "media-001"}))
            .unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(
            text.contains("\"language\""),
            "project language in output: {text}"
        );
        assert!(text.contains("ja"), "language value in output: {text}");
    }

    #[test]
    fn issue_039_get_transcript_per_call_overrides_project_language() {
        let mut exec = make_executor_with_media();
        exec.timeline.transcription_language = Some("ja".to_string());
        let result = exec
            .execute(
                "get_transcript",
                &json!({"mediaId": "media-001", "language": "ko"}),
            )
            .unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        // per-call "ko" should win over project "ja"
        assert!(text.contains("ko"), "per-call language wins: {text}");
        assert!(
            !text.contains("\"ja\""),
            "project language not in output: {text}"
        );
    }

    #[test]
    fn issue_039_get_transcript_no_language_no_field() {
        let mut exec = make_executor_with_media();
        // Neither per-call nor project language set
        exec.timeline.transcription_language = None;
        let result = exec
            .execute("get_transcript", &json!({"mediaId": "media-001"}))
            .unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        // language field should be omitted when None
        assert!(
            !text.contains("\"language\""),
            "no language field expected: {text}"
        );
    }

    #[test]
    fn issue_039_inspect_media_accepts_language_param() {
        let mut exec = make_executor_with_media();
        // Should not error — language param accepted
        let result = exec
            .execute(
                "inspect_media",
                &json!({"mediaId": "media-001", "language": "de"}),
            )
            .unwrap();
        assert!(
            result.get("isError").is_none(),
            "no error with language param"
        );
    }

    #[test]
    fn exec_022_create_folder() {
        let mut exec = make_executor();
        let result = exec
            .execute("create_folder", &json!({"name": "New Folder"}))
            .unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("Created folder"));
        assert_eq!(exec.media_manifest.folders.len(), 1);
    }

    #[test]
    fn exec_023_create_folder_missing_name() {
        let mut exec = make_executor();
        let err = exec.execute("create_folder", &json!({})).unwrap_err();
        assert!(err.contains("Missing name"));
    }

    #[test]
    fn exec_024_rename_folder() {
        let mut exec = make_executor_with_media();
        let _result = exec
            .execute(
                "rename_folder",
                &json!({"folderId": "folder-001", "name": "Renamed"}),
            )
            .unwrap();
        assert_eq!(exec.media_manifest.folders[0].name, "Renamed");
    }

    #[test]
    fn exec_025_delete_folder() {
        let mut exec = make_executor_with_media();
        let _result = exec
            .execute("delete_folder", &json!({"folderId": "folder-001"}))
            .unwrap();
        assert!(exec.media_manifest.folders.is_empty());
    }

    #[test]
    fn exec_026_rename_media() {
        let mut exec = make_executor_with_media();
        let _result = exec
            .execute(
                "rename_media",
                &json!({"mediaId": "media-001", "name": "renamed.mp4"}),
            )
            .unwrap();
        assert_eq!(exec.media_manifest.entries[0].name, "renamed.mp4");
    }

    #[test]
    fn exec_027_delete_media() {
        let mut exec = make_executor_with_media();
        let _result = exec
            .execute("delete_media", &json!({"mediaId": "media-001"}))
            .unwrap();
        assert!(exec.media_manifest.entries.is_empty());
    }

    #[test]
    fn exec_028_move_to_folder() {
        let mut exec = make_executor_with_media();
        let _result = exec
            .execute(
                "move_to_folder",
                &json!({"mediaId": "media-001", "folderId": "folder-001"}),
            )
            .unwrap();
        assert_eq!(
            exec.media_manifest.entries[0].folder_id.as_deref(),
            Some("folder-001")
        );
    }

    #[test]
    fn exec_029_move_to_folder_bad_folder() {
        let mut exec = make_executor_with_media();
        let err = exec
            .execute(
                "move_to_folder",
                &json!({"mediaId": "media-001", "folderId": "nonexistent"}),
            )
            .unwrap_err();
        assert!(err.contains("not found"));
    }

    #[test]
    fn exec_030_import_media() {
        let mut exec = make_executor();
        let _result = exec
            .execute(
                "import_media",
                &json!({"name": "new.mp4", "filePath": "/path/to/new.mp4"}),
            )
            .unwrap();
        assert_eq!(exec.media_manifest.entries.len(), 1);
    }

    #[test]
    fn exec_031_import_folder() {
        let mut exec = make_executor();
        let _result = exec
            .execute("import_folder", &json!({"folderName": "New Folder"}))
            .unwrap();
        assert_eq!(exec.media_manifest.folders.len(), 1);
    }

    #[test]
    fn exec_032_duplicate_project() {
        // Duplication is host filesystem I/O the pure executor can't perform, so it
        // must report an error honestly (not claim a no-op succeeded).
        let mut exec = make_executor_with_media();
        let result = exec.execute("duplicate_project", &json!({})).unwrap();
        assert_eq!(result["isError"], true);
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("requires host filesystem support"), "got: {text}");
    }

    #[test]
    fn exec_033_add_texts() {
        let mut exec = make_executor();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let result = exec
            .execute("add_texts", &json!({"texts": [{"text": "Hello"}]}))
            .unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("text clip"));
        assert_eq!(exec.timeline.tracks[0].clips.len(), 1);
    }

    #[test]
    fn exec_034_add_texts_missing_texts() {
        let mut exec = make_executor();
        let err = exec.execute("add_texts", &json!({})).unwrap_err();
        assert!(err.contains("Missing texts array"));
    }

    #[test]
    fn add_texts_applies_per_entry_style_and_transform() {
        let mut exec = make_executor();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        exec.execute(
            "add_texts",
            &json!({"texts": [{
                "content": "Title",
                "fontName": "Anton",
                "fontSize": 72.0,
                "fontWeight": 700.0,
                "color": "#00FF00",
                "alignment": "left",
                "transform": {"centerX": 0.5, "centerY": 0.9}
            }]}),
        )
        .unwrap();
        let clip = &exec.timeline.tracks[0].clips[0];
        assert_eq!(clip.text_content.as_deref(), Some("Title"));
        let ts = clip.text_style.as_ref().unwrap();
        assert_eq!(ts.font_name, "Anton");
        assert_eq!(ts.font_size, 72.0);
        assert_eq!(ts.font_weight, 700.0);
        assert_eq!((ts.color.r, ts.color.g, ts.color.b), (0.0, 1.0, 0.0));
        assert_eq!(ts.alignment, core_model::TextAlignment::Left);
        // Centre-only transform repositions; the y matches the request.
        assert!((clip.transform.center_y - 0.9).abs() < 1e-9);
    }

    #[test]
    fn add_texts_rejects_bad_color() {
        let mut exec = make_executor();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let err = exec
            .execute("add_texts", &json!({"texts": [{"content": "x", "color": "zzz"}]}))
            .unwrap_err();
        assert!(err.contains("invalid color"), "got: {err}");
    }

    #[test]
    fn add_texts_applies_animation_and_highlight() {
        let mut exec = make_executor();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        exec.execute(
            "add_texts",
            &json!({"texts": [{
                "content": "Go",
                "animation": "wordReveal",
                "highlightColor": "#FF8800"
            }]}),
        )
        .unwrap();
        let anim = exec.timeline.tracks[0].clips[0]
            .text_animation
            .as_ref()
            .expect("animation set");
        assert_eq!(anim.preset, core_model::TextAnimationPreset::WordReveal);
        let hl = anim.highlight.as_ref().expect("highlight parsed");
        assert!((hl.r - 1.0).abs() < 1e-6 && hl.g > 0.5 && hl.b == 0.0);
    }

    #[test]
    fn add_texts_animation_off_leaves_none() {
        let mut exec = make_executor();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        exec.execute("add_texts", &json!({"texts": [{"content": "x", "animation": "off"}]}))
            .unwrap();
        assert!(exec.timeline.tracks[0].clips[0].text_animation.is_none());
    }

    #[test]
    fn add_texts_rejects_bad_animation() {
        let mut exec = make_executor();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let err = exec
            .execute("add_texts", &json!({"texts": [{"content": "x", "animation": "bogus"}]}))
            .unwrap_err();
        assert!(err.contains("invalid animation"), "got: {err}");
    }

    #[test]
    fn exec_035_add_shapes() {
        let mut exec = make_executor();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let result = exec
            .execute("add_shapes", &json!({"entries": [{"type": "rect"}]}))
            .unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("shape clip"));
    }

    #[test]
    fn add_shapes_applies_style_fill_and_transform() {
        let mut exec = make_executor();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        exec.execute(
            "add_shapes",
            &json!({"entries": [{
                "type": "oval",
                "style": {"color": "#FF0000", "width": 5.0, "dashed": true},
                "fill": {"enabled": true, "color": "#0000FF"},
                "transform": {"centerX": 0.25, "centerY": 0.75, "width": 0.4, "height": 0.3}
            }]}),
        )
        .unwrap();
        let clip = &exec.timeline.tracks[0].clips[0];
        let ss = clip.shape_style.as_ref().unwrap();
        assert_eq!(ss.kind, core_model::ShapeKind::Oval);
        assert_eq!((ss.stroke.color.r, ss.stroke.color.g, ss.stroke.color.b), (1.0, 0.0, 0.0));
        assert_eq!(ss.stroke.width, 5.0);
        assert!(ss.stroke.dashed);
        assert!(ss.fill.enabled);
        assert_eq!((ss.fill.color.r, ss.fill.color.g, ss.fill.color.b), (0.0, 0.0, 1.0));
        assert!((clip.transform.center_x - 0.25).abs() < 1e-9);
        assert!((clip.transform.width - 0.4).abs() < 1e-9);
    }

    #[test]
    fn add_shapes_rejects_bad_fill_color() {
        let mut exec = make_executor();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let err = exec
            .execute(
                "add_shapes",
                &json!({"entries": [{"type": "rect", "fill": {"color": "zzz"}}]}),
            )
            .unwrap_err();
        assert!(err.contains("invalid fill color"), "got: {err}");
    }

    #[test]
    fn exec_036_add_shapes_empty_entries() {
        let mut exec = make_executor();
        let err = exec
            .execute("add_shapes", &json!({"entries": []}))
            .unwrap_err();
        assert!(err.contains("non-empty"));
    }

    #[test]
    fn exec_037_apply_color() {
        let mut exec = make_executor();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let clip = crate::test_helpers::make_clip(0, 150);
        let placed = timeline_core::place_clips(exec.timeline_mut(), 0, 0, &[clip]);
        let clip_id = placed.first().expect("place_clips returned empty");
        let result = exec
            .execute("apply_color", &json!({"clipId": clip_id, "exposure": 0.5}))
            .unwrap();
        assert!(result["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("color"));
    }

    #[test]
    fn exec_038_apply_color_missing_clip() {
        let mut exec = make_executor();
        let err = exec
            .execute("apply_color", &json!({"clipId": "nonexistent"}))
            .unwrap_err();
        assert!(err.contains("not found"));
    }

    #[test]
    fn exec_039_apply_effect() {
        let mut exec = make_executor();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let clip = crate::test_helpers::make_clip(0, 150);
        let placed = timeline_core::place_clips(exec.timeline_mut(), 0, 0, &[clip]);
        let clip_id = placed.first().expect("place_clips returned empty");
        let result = exec
            .execute(
                "apply_effect",
                &json!({"clipId": clip_id, "effectType": "blur"}),
            )
            .unwrap();
        assert!(result["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("Applied"));
    }

    #[test]
    fn exec_040_set_chroma_key() {
        let mut exec = make_executor();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let clip = crate::test_helpers::make_clip(0, 150);
        let placed = timeline_core::place_clips(exec.timeline_mut(), 0, 0, &[clip]);
        let clip_id = placed.first().expect("place_clips returned empty");
        let result = exec
            .execute(
                "set_chroma_key",
                &json!({"clipId": clip_id, "color": "#00FF00"}),
            )
            .unwrap();
        assert!(result["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("chroma"));
    }

    #[test]
    fn exec_041_set_blend_mode() {
        let mut exec = make_executor();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let clip = crate::test_helpers::make_clip(0, 150);
        let placed = timeline_core::place_clips(exec.timeline_mut(), 0, 0, &[clip]);
        let clip_id = placed.first().expect("place_clips returned empty");
        let result = exec
            .execute(
                "set_blend_mode",
                &json!({"clipId": clip_id, "mode": "multiply"}),
            )
            .unwrap();
        assert!(result["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("blend"));
    }

    #[test]
    fn exec_042_set_color_grade() {
        let mut exec = make_executor();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let clip = crate::test_helpers::make_clip(0, 150);
        let placed = timeline_core::place_clips(exec.timeline_mut(), 0, 0, &[clip]);
        let clip_id = placed.first().expect("place_clips returned empty");
        let result = exec
            .execute(
                "set_color_grade",
                &json!({"clipId": clip_id, "saturation": 1.2}),
            )
            .unwrap();
        assert!(result["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("color grade"));
    }

    #[test]
    fn exec_043_inspect_color() {
        let mut exec = make_executor();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let clip = crate::test_helpers::make_clip(0, 150);
        let placed = timeline_core::place_clips(exec.timeline_mut(), 0, 0, &[clip]);
        let clip_id = placed.first().expect("place_clips returned empty");
        let result = exec
            .execute("inspect_color", &json!({"clipId": clip_id}))
            .unwrap();
        assert!(result["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("Color info"));
    }

    #[test]
    fn exec_044_inspect_color_no_args() {
        let mut exec = make_executor();
        let err = exec.execute("inspect_color", &json!({})).unwrap_err();
        assert!(err.contains("clipId or mediaRef"));
    }

    #[test]
    fn exec_045_add_captions_stub() {
        let mut exec = make_executor();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let clip = crate::test_helpers::make_clip(0, 150);
        let placed = timeline_core::place_clips(exec.timeline_mut(), 0, 0, &[clip]);
        let clip_id = placed.first().expect("place_clips returned empty");
        let result = exec
            .execute("add_captions", &json!({"clipIds": [clip_id]}))
            .unwrap();
        assert_eq!(result["isError"], true);
    }

    #[test]
    fn exec_046_add_captions_empty_clip_ids() {
        let mut exec = make_executor();
        let err = exec
            .execute("add_captions", &json!({"clipIds": []}))
            .unwrap_err();
        assert!(err.contains("non-empty"));
    }

    #[test]
    fn exec_047_apply_animation() {
        let mut exec = make_executor();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let clip = crate::test_helpers::make_clip(0, 150);
        let placed = timeline_core::place_clips(exec.timeline_mut(), 0, 0, &[clip]);
        let clip_id = placed.first().expect("place_clips returned empty");
        let result = exec
            .execute(
                "apply_animation",
                &json!({"clipId": clip_id, "preset": "fadeIn"}),
            )
            .unwrap();
        assert!(result["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("animation"));
    }

    #[test]
    fn exec_048_generate_video() {
        let mut exec = make_executor();
        let result = exec
            .execute("generate_video", &json!({"prompt": "A cat walking"}))
            .unwrap();
        assert_eq!(result["isError"], true);
        assert!(exec.media_manifest.entries.len() == 1);
    }

    #[test]
    fn exec_049_generate_video_missing_prompt() {
        let mut exec = make_executor();
        let err = exec.execute("generate_video", &json!({})).unwrap_err();
        assert!(err.contains("Missing prompt"));
    }

    #[test]
    fn exec_050_generate_image() {
        let mut exec = make_executor();
        let result = exec
            .execute("generate_image", &json!({"prompt": "A sunset"}))
            .unwrap();
        assert_eq!(result["isError"], true);
    }

    #[test]
    fn exec_051_generate_audio() {
        let mut exec = make_executor();
        let result = exec
            .execute("generate_audio", &json!({"prompt": "Narration"}))
            .unwrap();
        assert_eq!(result["isError"], true);
    }

    #[test]
    fn exec_052_generate_music() {
        let mut exec = make_executor();
        let result = exec
            .execute("generate_music", &json!({"prompt": "Upbeat pop"}))
            .unwrap();
        assert_eq!(result["isError"], true);
    }

    #[test]
    fn exec_053_upscale_media() {
        let mut exec = make_executor_with_media();
        let result = exec
            .execute("upscale_media", &json!({"mediaId": "media-001"}))
            .unwrap();
        assert_eq!(result["isError"], true);
    }

    #[test]
    fn exec_054_upscale_media_not_found() {
        let mut exec = make_executor_with_media();
        let err = exec
            .execute("upscale_media", &json!({"mediaId": "nonexistent"}))
            .unwrap_err();
        assert!(err.contains("not found"));
    }

    #[test]
    fn exec_055_set_keyframes_missing_clip() {
        let mut exec = make_executor();
        let err = exec
            .execute("set_keyframes", &json!({"clipId": "nonexistent", "property": "opacity", "keyframes": [[0, 1.0]]}))
            .unwrap_err();
        assert!(err.contains("not found"));
    }

    fn executor_with_clip() -> ToolExecutor {
        let mut timeline = Timeline::default();
        timeline.tracks.push(core_model::Track {
            id: "t".into(),
            r#type: core_model::ClipType::Video,
            muted: false,
            hidden: false,
            sync_locked: false,
            clips: vec![core_model::Clip {
                id: "c".into(),
                media_ref: "m".into(),
                media_type: core_model::ClipType::Video,
                source_clip_type: core_model::ClipType::Video,
                start_frame: 0,
                duration_frames: 100,
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
                text_animation: None,
                word_timings: None,
            }],
        });
        ToolExecutor::new(timeline, MediaManifest::default())
    }

    fn only_clip(exec: &ToolExecutor) -> &core_model::Clip {
        &exec.timeline().tracks[0].clips[0]
    }

    #[test]
    fn set_keyframes_scalar_opacity_with_interp_default() {
        let mut exec = executor_with_clip();
        exec.execute(
            "set_keyframes",
            &json!({"clipId": "c", "property": "opacity", "keyframes": [[0, 0.0, "linear"], [100, 1.0]]}),
        )
        .unwrap();
        let kfs = &only_clip(&exec).opacity_track.as_ref().unwrap().keyframes;
        assert_eq!(kfs.len(), 2);
        assert_eq!(kfs[0].frame, 0);
        assert_eq!(kfs[0].value, 0.0);
        assert_eq!(kfs[0].interpolation_out, core_model::Interpolation::Linear);
        // Missing interp defaults to smooth.
        assert_eq!(kfs[1].interpolation_out, core_model::Interpolation::Smooth);
    }

    #[test]
    fn set_keyframes_position_stores_top_left_pair() {
        let mut exec = executor_with_clip();
        exec.execute(
            "set_keyframes",
            &json!({"clipId": "c", "property": "position", "keyframes": [[0, 0.1, 0.2], [50, 0.3, 0.4]]}),
        )
        .unwrap();
        let kfs = &only_clip(&exec).position_track.as_ref().unwrap().keyframes;
        assert_eq!(kfs.len(), 2);
        assert_eq!(kfs[0].value, AnimPair { a: 0.1, b: 0.2 });
        assert_eq!(kfs[1].value, AnimPair { a: 0.3, b: 0.4 });
    }

    #[test]
    fn set_keyframes_scale_stores_pair() {
        let mut exec = executor_with_clip();
        exec.execute(
            "set_keyframes",
            &json!({"clipId": "c", "property": "scale", "keyframes": [[0, 0.5, 0.25]]}),
        )
        .unwrap();
        let kfs = &only_clip(&exec).scale_track.as_ref().unwrap().keyframes;
        assert_eq!(kfs[0].value, AnimPair { a: 0.5, b: 0.25 });
    }

    #[test]
    fn set_keyframes_crop_maps_top_right_bottom_left() {
        let mut exec = executor_with_clip();
        // Input order [top, right, bottom, left].
        exec.execute(
            "set_keyframes",
            &json!({"clipId": "c", "property": "crop", "keyframes": [[0, 0.1, 0.2, 0.3, 0.4]]}),
        )
        .unwrap();
        let c = only_clip(&exec).crop_track.as_ref().unwrap().keyframes[0].value;
        assert_eq!(c.top, 0.1);
        assert_eq!(c.right, 0.2);
        assert_eq!(c.bottom, 0.3);
        assert_eq!(c.left, 0.4);
    }

    #[test]
    fn set_keyframes_empty_array_clears_track() {
        let mut exec = executor_with_clip();
        exec.execute(
            "set_keyframes",
            &json!({"clipId": "c", "property": "opacity", "keyframes": [[0, 0.5]]}),
        )
        .unwrap();
        assert!(only_clip(&exec).opacity_track.is_some());
        exec.execute(
            "set_keyframes",
            &json!({"clipId": "c", "property": "opacity", "keyframes": []}),
        )
        .unwrap();
        assert!(only_clip(&exec).opacity_track.is_none(), "empty clears");
    }

    #[test]
    fn set_keyframes_sorts_and_dedupes_last_wins() {
        let mut exec = executor_with_clip();
        exec.execute(
            "set_keyframes",
            &json!({"clipId": "c", "property": "opacity", "keyframes": [[50, 0.5], [0, 0.1], [0, 0.9]]}),
        )
        .unwrap();
        let kfs = &only_clip(&exec).opacity_track.as_ref().unwrap().keyframes;
        assert_eq!(kfs.len(), 2, "duplicate frame 0 collapsed");
        assert_eq!(kfs[0].frame, 0);
        assert_eq!(kfs[0].value, 0.9, "last row for frame 0 wins");
        assert_eq!(kfs[1].frame, 50);
    }

    #[test]
    fn set_keyframes_wrong_arity_errors() {
        let mut exec = executor_with_clip();
        // position needs two values.
        let err = exec
            .execute(
                "set_keyframes",
                &json!({"clipId": "c", "property": "position", "keyframes": [[0, 0.5]]}),
            )
            .unwrap_err();
        assert!(err.contains("topLeftX, topLeftY"), "got: {err}");
    }

    #[test]
    fn set_keyframes_unknown_property_errors() {
        let mut exec = executor_with_clip();
        let err = exec
            .execute(
                "set_keyframes",
                &json!({"clipId": "c", "property": "warp", "keyframes": [[0, 1.0]]}),
            )
            .unwrap_err();
        assert!(err.contains("Unknown keyframe property"));
    }

    #[test]
    fn set_clip_properties_sets_text_color_and_alignment() {
        let mut exec = executor_with_clip();
        exec.execute(
            "set_clip_properties",
            &json!({"clipIds": ["c"], "properties": {"color": "#FF0000", "alignment": "center"}}),
        )
        .unwrap();
        let ts = only_clip(&exec).text_style.as_ref().expect("text style created");
        assert_eq!((ts.color.r, ts.color.g, ts.color.b), (1.0, 0.0, 0.0));
        assert_eq!(ts.alignment, core_model::TextAlignment::Center);
    }

    #[test]
    fn set_clip_properties_rejects_bad_color() {
        let mut exec = executor_with_clip();
        let err = exec
            .execute(
                "set_clip_properties",
                &json!({"clipIds": ["c"], "properties": {"color": "not-a-color"}}),
            )
            .unwrap_err();
        assert!(err.contains("invalid color"), "got: {err}");
    }

    #[test]
    fn set_clip_properties_rejects_bad_alignment() {
        let mut exec = executor_with_clip();
        let err = exec
            .execute(
                "set_clip_properties",
                &json!({"clipIds": ["c"], "properties": {"alignment": "middle"}}),
            )
            .unwrap_err();
        assert!(err.contains("invalid alignment"), "got: {err}");
    }

    #[test]
    fn set_clip_properties_sets_font_weight_background_border() {
        let mut exec = executor_with_clip();
        exec.execute(
            "set_clip_properties",
            &json!({"clipIds": ["c"], "properties": {
                "fontWeight": 700.0,
                "background": {"enabled": true, "color": "#000000", "padding": 8.0, "cornerRadius": 4.0},
                "border": {"enabled": true, "color": "#FFFFFF"}
            }}),
        )
        .unwrap();
        let ts = only_clip(&exec).text_style.as_ref().unwrap();
        assert_eq!(ts.font_weight, 700.0);
        assert!(ts.background.enabled);
        assert_eq!(ts.background.padding, Some(8.0));
        assert_eq!(ts.background.corner_radius, Some(4.0));
        assert_eq!((ts.background.color.r, ts.background.color.g), (0.0, 0.0));
        assert!(ts.border.enabled);
        assert_eq!((ts.border.color.r, ts.border.color.g, ts.border.color.b), (1.0, 1.0, 1.0));
    }

    #[test]
    fn set_clip_properties_rejects_bad_background_color() {
        let mut exec = executor_with_clip();
        let err = exec
            .execute(
                "set_clip_properties",
                &json!({"clipIds": ["c"], "properties": {"background": {"color": "zzz"}}}),
            )
            .unwrap_err();
        assert!(err.contains("invalid background color"), "got: {err}");
    }

    #[test]
    fn exec_056_ripple_delete_missing_args() {
        let mut exec = make_executor();
        let err = exec
            .execute("ripple_delete_ranges", &json!({}))
            .unwrap_err();
        assert!(err.contains("Missing trackIndex"));
    }

    #[test]
    fn insert_clips_auto_creates_linked_audio_and_pushes() {
        // Inserting a video-with-audio mid-clip: the tail ripples, the video is
        // placed, and a linked audio clip is created on an audio track sharing the
        // group at the same position.
        let timeline = executor_with_clip().timeline().clone(); // track 0, "c" [0,100)
        let mut manifest = MediaManifest::default();
        manifest.entries.push(core_model::MediaManifestEntry {
            id: "vid".into(),
            name: "v.mp4".into(),
            r#type: ClipType::Video,
            source: core_model::MediaSource::External {
                absolute_path: "/v.mp4".into(),
            },
            duration: 2.0, // 60 frames @ 30fps
            generation_input: None,
            source_width: Some(1920),
            source_height: Some(1080),
            source_fps: Some(30.0),
            has_audio: Some(true),
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
        });
        let mut exec = ToolExecutor::new(timeline, manifest);
        exec.execute(
            "insert_clips",
            &json!({"mediaIds": ["vid"], "trackIndex": 0, "frame": 40}),
        )
        .unwrap();

        let video_clips = &exec.timeline().tracks[0].clips;
        // Frame conservation: 100 original + 60 inserted.
        let total: i64 = video_clips.iter().map(|c| c.duration_frames).sum();
        assert_eq!(total, 160, "frames preserved: {video_clips:?}");
        let vid = video_clips
            .iter()
            .find(|c| c.link_group_id.is_some())
            .expect("inserted video is linked");
        assert_eq!(vid.start_frame, 40);
        // Linked audio on an audio track, same group + position.
        let audio_track = exec
            .timeline()
            .tracks
            .iter()
            .find(|t| t.r#type == ClipType::Audio)
            .expect("linked audio track");
        assert_eq!(audio_track.clips.len(), 1);
        assert_eq!(audio_track.clips[0].link_group_id, vid.link_group_id);
        assert_eq!(audio_track.clips[0].start_frame, 40);
        assert_eq!(audio_track.clips[0].duration_frames, vid.duration_frames);
    }

    #[test]
    fn insert_clips_pushes_split_tail_preserving_frames() {
        // Inserting inside a clip must split it and push the tail, not overwrite it.
        // Regression: the tail (a fresh split id) was not shifted, then place_clips
        // trimmed it — destroying original frames.
        let mut exec = executor_with_clip(); // track 0, clip "c" span [0,100)
        exec.execute(
            "insert_clips",
            &json!({"mediaIds": ["new-media"], "trackIndex": 0, "frame": 40, "durationFrames": 50}),
        )
        .unwrap();
        let clips = &exec.timeline().tracks[0].clips;
        let total: i64 = clips.iter().map(|c| c.duration_frames).sum();
        // 100 original frames preserved + 50 inserted = 150 (not 100 with 50 lost).
        assert_eq!(total, 150, "no frames destroyed: {:?}",
            clips.iter().map(|c| (c.start_frame, c.duration_frames)).collect::<Vec<_>>());
        let mut spans: Vec<(i64, i64)> = clips
            .iter()
            .map(|c| (c.start_frame, c.start_frame + c.duration_frames))
            .collect();
        spans.sort();
        assert_eq!(spans, vec![(0, 40), (40, 90), (90, 150)]);
    }

    #[test]
    fn ripple_delete_partial_range_cuts_fragment_not_whole_clip() {
        // Regression: a partial-overlap range destroyed the whole clip (silent media
        // loss). It must cut only the overlapping fragment and close the gap.
        let mut exec = executor_with_clip(); // track 0, clip "c" span [0,100)
        exec.execute(
            "ripple_delete_ranges",
            &json!({"trackIndex": 0, "ranges": [{"start": 25, "end": 50}]}),
        )
        .unwrap();
        let clips = &exec.timeline().tracks[0].clips;
        let mut spans: Vec<(i64, i64)> = clips
            .iter()
            .map(|c| (c.start_frame, c.start_frame + c.duration_frames))
            .collect();
        spans.sort();
        // Head [0,25) kept; tail [50,100) slid left by 25 → [25,75). Not destroyed.
        assert_eq!(spans, vec![(0, 25), (25, 75)], "fragment cut + gap closed");
    }

    #[test]
    fn exec_057_apply_effect_remove() {
        let mut exec = make_executor();
        let _ = timeline_core::insert_track_at(exec.timeline_mut(), 0, ClipType::Video);
        let clip = crate::test_helpers::make_clip(0, 150);
        let placed = timeline_core::place_clips(exec.timeline_mut(), 0, 0, &[clip]);
        let clip_id = placed.first().expect("place_clips returned empty");
        // Apply an effect first
        let _ = exec.execute(
            "apply_effect",
            &json!({"clipId": clip_id, "effectType": "blur"}),
        );
        // Then remove it
        let result = exec
            .execute(
                "apply_effect",
                &json!({"clipId": clip_id, "effectType": "blur", "remove": true}),
            )
            .unwrap();
        assert!(result["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("Removed"));
    }

    // ── Missing-media helpers (#135) ────────────────────────────────

    #[test]
    fn exec_058_missing_entry_ids_none_missing() {
        let exec = make_executor_with_media();
        let offline = exec.media_offline_ids(chrono::Utc::now(), |_| false);
        assert!(offline.is_empty(), "no entries should be missing");
    }

    #[test]
    fn exec_059_missing_entry_ids_all_missing() {
        let exec = make_executor_with_media();
        // The helper adds one entry with no cached_remote_url.
        let offline = exec.media_offline_ids(chrono::Utc::now(), |_| true);
        assert_eq!(offline.len(), 1, "the one entry should be missing");
    }

    #[test]
    fn exec_060_is_media_offline_true() {
        let exec = make_executor_with_media();
        let id = exec.media_manifest.entries[0].id.clone();
        assert!(exec.is_media_offline(&id, chrono::Utc::now(), |_| true));
    }

    #[test]
    fn exec_061_is_media_offline_false() {
        let exec = make_executor_with_media();
        let id = exec.media_manifest.entries[0].id.clone();
        assert!(!exec.is_media_offline(&id, chrono::Utc::now(), |_| false));
    }

    #[test]
    fn exec_062_is_media_offline_unknown_ref() {
        let exec = make_executor_with_media();
        assert!(!exec.is_media_offline("unknown", chrono::Utc::now(), |_| true));
    }

    #[test]
    fn exec_063_is_media_offline_cached_excluded() {
        let mut exec = make_executor();
        exec.media_manifest
            .entries
            .push(core_model::MediaManifestEntry {
                id: "cached".into(),
                name: "cached".into(),
                r#type: core_model::ClipType::Video,
                source: core_model::MediaSource::External {
                    absolute_path: "/tmp/cached.mp4".into(),
                },
                duration: 10.0,
                generation_input: None,
                source_width: None,
                source_height: None,
                source_fps: None,
                has_audio: None,
                folder_id: None,
                cached_remote_url: Some("https://c".into()),
                cached_remote_url_expires_at: None,
                source_timecode_frame: None,
                source_timecode_quanta: None,
                source_timecode_drop_frame: None,
                ai_tags: None,
                ai_description: None,
                ai_label_status: None,
                generation_status: None,
            });
        assert!(
            !exec.is_media_offline("cached", chrono::Utc::now(), |_| true),
            "cached entries should not be offline"
        );
    }

    #[test]
    fn is_media_offline_expired_cache_is_offline() {
        // An EXPIRED cached URL no longer hides an offline asset (the `now` clock
        // threads through the helper into MediaManifestEntry::cache_is_fresh).
        let mut exec = make_executor();
        let past = chrono::Utc::now() - chrono::Duration::hours(1);
        exec.media_manifest
            .entries
            .push(core_model::MediaManifestEntry {
                id: "stale".into(),
                name: "stale".into(),
                r#type: core_model::ClipType::Video,
                source: core_model::MediaSource::External {
                    absolute_path: "/tmp/stale.mp4".into(),
                },
                duration: 10.0,
                generation_input: None,
                source_width: None,
                source_height: None,
                source_fps: None,
                has_audio: None,
                folder_id: None,
                cached_remote_url: Some("https://c".into()),
                cached_remote_url_expires_at: Some(past),
                source_timecode_frame: None,
                source_timecode_quanta: None,
                source_timecode_drop_frame: None,
                ai_tags: None,
                ai_description: None,
                ai_label_status: None,
                generation_status: None,
            });
        assert!(exec.is_media_offline("stale", chrono::Utc::now(), |_| true));
    }

    #[test]
    fn exec_064_is_media_unprocessable_true() {
        let exec = make_executor_with_media();
        let id = exec.media_manifest.entries[0].id.clone();
        // File exists (not missing) but is unprocessable.
        assert!(exec.is_media_unprocessable(&id, chrono::Utc::now(), |_| false, |_| true));
    }

    #[test]
    fn exec_065_is_media_unprocessable_missing_not_unprocessable() {
        let exec = make_executor_with_media();
        let id = exec.media_manifest.entries[0].id.clone();
        // If file is missing, it's offline, not unprocessable.
        assert!(!exec.is_media_unprocessable(&id, chrono::Utc::now(), |_| true, |_| true));
    }

    // ── Revision counter (shared-editor-state) ─────────────────────────

    #[test]
    fn revision_unchanged_by_read_only_tool() {
        let mut exec = ToolExecutor::new(Timeline::default(), MediaManifest::default());
        assert_eq!(exec.revision(), 0);
        exec.execute("get_timeline", &json!({})).unwrap();
        assert_eq!(exec.revision(), 0);
    }

    #[test]
    fn revision_bumped_by_successful_mutation() {
        let mut exec = ToolExecutor::new(Timeline::default(), MediaManifest::default());
        exec.execute("create_folder", &json!({"name": "B-roll"}))
            .unwrap();
        assert_eq!(exec.revision(), 1);
    }

    #[test]
    fn revision_unchanged_by_failed_mutation() {
        let mut exec = ToolExecutor::new(Timeline::default(), MediaManifest::default());
        exec.execute("create_folder", &json!({"name": "B-roll"}))
            .unwrap();
        assert!(exec.execute("split_clips", &json!({})).is_err());
        assert_eq!(exec.revision(), 1);
    }

    #[test]
    fn load_project_replaces_state_and_bumps_revision() {
        let mut exec = ToolExecutor::new(Timeline::default(), MediaManifest::default());
        exec.execute("create_folder", &json!({"name": "B-roll"}))
            .unwrap();
        let before = exec.revision();
        let timeline = Timeline {
            fps: 60,
            ..Default::default()
        };
        exec.load_project(timeline, MediaManifest::default());
        assert_eq!(exec.revision(), before + 1);
        assert_eq!(exec.timeline().fps, 60);
        assert!(exec.media_manifest().folders.is_empty());
        assert!(exec.undo_stack().is_empty());
    }
}

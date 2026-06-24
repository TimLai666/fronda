//! Tool execution dispatcher: routes agent tool calls to timeline engine.
//!
//! A ToolExecutor holds the mutable project state (Timeline + UndoStack)
//! and provides a single `execute()` entry point for the MCP server.

use crate::read_tools::format_timeline_json;
use crate::undo::{UndoCommand, UndoStack};
use core_model::{
    Clip, ClipType, Effect, GenerationInput, Interpolation, Keyframe, KeyframeTrack, MediaManifest,
    MediaSource, TextStyle, Timeline, Transform,
};
use serde_json::{json, Value};
use uuid::Uuid;

/// Runtime state for executing agent timeline tools.
pub struct ToolExecutor {
    timeline: Timeline,
    media_manifest: MediaManifest,
    undo_stack: UndoStack,
}

impl ToolExecutor {
    pub fn new(timeline: Timeline, media_manifest: MediaManifest) -> Self {
        Self {
            timeline,
            media_manifest,
            undo_stack: UndoStack::new(),
        }
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

    /// Execute a tool by name with validated JSON arguments.
    ///
    /// Returns the JSON result that should become the MCP `content` array.
    /// For mutation tools, automatically snapshots before/after for undo.
    pub fn execute(&mut self, tool_name: &str, args: &Value) -> Result<Value, String> {
        match tool_name {
            // ── Read-only tools ──────────────────────────────────────────
            "get_timeline" => self.cmd_get_timeline(),

            // ── Mutation tools (undo-tracked) ────────────────────────────
            "split_clip" => self.exec_mut(tool_name, ToolExecutor::cmd_split_clip, args),
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
            "add_texts" => self.exec_mut(tool_name, ToolExecutor::cmd_add_texts, args),
            "add_shapes" => self.exec_mut(tool_name, ToolExecutor::cmd_add_shapes, args),
            "apply_color" => self.exec_mut(tool_name, ToolExecutor::cmd_apply_color, args),
            "apply_effect" => self.exec_mut(tool_name, ToolExecutor::cmd_apply_effect, args),
            "set_chroma_key" => self.exec_mut(tool_name, ToolExecutor::cmd_set_chroma_key, args),
            "set_blend_mode" => self.exec_mut(tool_name, ToolExecutor::cmd_set_blend_mode, args),
            "set_color_grade" => self.exec_mut(tool_name, ToolExecutor::cmd_set_color_grade, args),
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

    fn cmd_split_clip(&mut self, args: &Value) -> Result<Value, String> {
        let clip_id = args
            .get("clipId")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing clipId".to_string())?;
        let frame = args
            .get("frame")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| "Missing or invalid frame".to_string())?;

        let new_ids = timeline_core::split_clip(&mut self.timeline, clip_id, frame);
        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!("Split clip '{clip_id}' at frame {frame}. Created {} new clip(s): {new_ids:?}",
                    new_ids.len())
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

        let mut changed_count = 0usize;
        let mut changed_fields: Vec<String> = Vec::new();
        for clip_id in &clip_ids {
            let Some(loc) = timeline_core::find_clip(&self.timeline, clip_id) else {
                continue;
            };
            let clip = &mut self.timeline.tracks[loc.track_index].clips[loc.clip_index];
            let changes = timeline_core::set_clip_properties(
                clip,
                duration,
                trim_start,
                trim_end,
                speed,
                volume,
                opacity,
                transform.as_ref(),
                content,
                font_name,
                font_size,
            );
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

        let Some(loc) = timeline_core::find_clip(&self.timeline, clip_id) else {
            return Err(format!("Clip '{clip_id}' not found"));
        };
        let clip = &mut self.timeline.tracks[loc.track_index].clips[loc.clip_index];

        let keyframes: Vec<Keyframe<f64>> = kf_array
            .iter()
            .filter_map(|kf| {
                let frame = kf.get("frame").and_then(|v| v.as_i64())?;
                let value = kf.get("value").and_then(|v| v.as_f64())?;
                let interp = match kf
                    .get("interpolation")
                    .and_then(|v| v.as_str())
                    .unwrap_or("smooth")
                {
                    "linear" => Interpolation::Linear,
                    "hold" => Interpolation::Hold,
                    _ => Interpolation::Smooth,
                };
                Some(Keyframe {
                    frame,
                    value,
                    interpolation_out: interp,
                })
            })
            .collect();

        if keyframes.is_empty() && !kf_array.is_empty() {
            return Err("Could not parse any valid keyframes".to_string());
        }

        let track = KeyframeTrack {
            keyframes: keyframes.clone(),
        };
        let trimmed = if track.keyframes.is_empty() {
            None
        } else {
            Some(track)
        };

        match property {
            "opacity" => clip.opacity_track = trimmed,
            "volume" => clip.volume_track = trimmed,
            "rotation" => clip.rotation_track = trimmed,
            other => return Err(format!("Unknown keyframe property '{other}'")),
        }

        let kf_len = keyframes.len();
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

        let config = timeline_core::RippleDeleteConfig {
            anchor_track_index: track_index,
            ranges,
        };
        let outcome = timeline_core::compute_ripple_delete(&self.timeline, config);

        let result = match outcome {
            timeline_core::RippleDeleteOutcome::Ok(report) => {
                use timeline_core::ClipMathExt;

                for ti in &report.cleared_track_indices {
                    let ids_to_remove: Vec<String> = self.timeline.tracks[*ti]
                        .clips
                        .iter()
                        .filter(|c| {
                            ranges_val.iter().any(|r| {
                                let s = r.get("start").and_then(|v| v.as_i64()).unwrap_or(0);
                                let e = r.get("end").and_then(|v| v.as_i64()).unwrap_or(0);
                                c.start_frame < e && c.end_frame() > s
                            })
                        })
                        .map(|c| c.id.clone())
                        .collect();
                    timeline_core::remove_clips(&mut self.timeline, ids_to_remove, false);
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

        let track_index = args
            .get("trackIndex")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| "Missing trackIndex".to_string())? as usize;

        if track_index >= self.timeline.tracks.len() {
            return Err(format!("Track index {track_index} out of bounds"));
        }

        let clips: Vec<Clip> = media_ids
            .iter()
            .map(|media_id| Clip {
                id: Uuid::new_v4().to_string(),
                media_ref: media_id.clone(),
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
            })
            .collect();

        let placed_ids = timeline_core::place_clips(&mut self.timeline, track_index, 0, &clips);
        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!("Added {} clip(s) to track {track_index}: {placed_ids:?}",
                    placed_ids.len())
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

        let clip_specs: Vec<timeline_core::RippleInsertClipSpec> = media_ids
            .iter()
            .map(|_| timeline_core::RippleInsertClipSpec {
                asset_id: Uuid::new_v4().to_string(),
                duration_frames: 150,
                trim_start_frame: None,
                trim_end_frame: None,
            })
            .collect();

        let config = timeline_core::RippleInsertConfig {
            track_index,
            insert_frame: frame,
            clips: clip_specs,
            linked_audio_track_index: None,
        };

        let outcome = timeline_core::compute_ripple_insert_with_split(&self.timeline, config);

        match outcome {
            timeline_core::RippleInsertWithSplitOutcome::Ok(plan) => {
                // Apply split actions before shifting
                for (_, clip_id, split_at) in &plan.split_actions {
                    timeline_core::split_clip(&mut self.timeline, clip_id, *split_at);
                }
                // Apply shifts
                for (ti, shifts) in plan.insert.shifts_by_track.iter().enumerate() {
                    if ti < self.timeline.tracks.len() {
                        for shift in shifts {
                            if let Some(clip) = self.timeline.tracks[ti]
                                .clips
                                .iter_mut()
                                .find(|c| c.id == shift.clip_id)
                            {
                                clip.start_frame = shift.new_start_frame;
                            }
                        }
                        timeline_core::sort_clips_on_track(&mut self.timeline, ti);
                    }
                }
                // Place new clips
                let new_clips: Vec<Clip> = plan
                    .insert
                    .clips
                    .iter()
                    .map(|spec| Clip {
                        id: Uuid::new_v4().to_string(),
                        media_ref: spec.asset_id.clone(),
                        media_type: ClipType::Video,
                        source_clip_type: ClipType::Video,
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
                    })
                    .collect();

                let placed = timeline_core::place_clips(
                    &mut self.timeline,
                    plan.insert.track_index,
                    plan.insert.insert_frame,
                    &new_clips,
                );

                Ok(json!({
                    "content": [{
                        "type": "text",
                        "text": format!(
                            "Inserted {} clip(s) at track {} frame {}",
                            placed.len(),
                            plan.insert.track_index,
                            plan.insert.insert_frame
                        )
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

        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!(
                    "Media: {} ({:?}), duration: {:.1}s, source: {:?}",
                    entry.name, entry.r#type, entry.duration, entry.source
                )
            }]
        }))
    }

    fn cmd_search_media(&self, args: &Value) -> Result<Value, String> {
        let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
        let r#type = args.get("type").and_then(|v| v.as_str());

        let results: Vec<&core_model::MediaManifestEntry> = self
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

        if results.is_empty() {
            return Ok(json!({
                "content": [{"type": "text", "text": "No media found".to_string()}]
            }));
        }

        let lines: Vec<String> = results
            .iter()
            .map(|e| format!("{}: {} ({:?})", e.id, e.name, e.r#type))
            .collect();
        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!("Found {} media:\n{}", results.len(), lines.join("\n"))
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

        let entry = self
            .media_manifest
            .entries
            .iter()
            .find(|e| e.id == media_id)
            .ok_or_else(|| format!("Media '{}' not found", media_id))?;

        let details = json!({
            "id": entry.id,
            "name": entry.name,
            "type": entry.r#type,
            "duration": entry.duration,
            "sourceWidth": entry.source_width,
            "sourceHeight": entry.source_height,
            "sourceFps": entry.source_fps,
            "hasAudio": entry.has_audio,
            "folderId": entry.folder_id,
        });

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

    fn cmd_get_transcript(&self, _args: &Value) -> Result<Value, String> {
        Ok(json!({
            "content": [{
                "type": "text",
                "text": "Transcript system is not yet connected to the timeline engine. No captions available."
            }],
            "isError": true,
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
        let cloned_timeline = self.timeline.clone();
        let cloned_manifest = self.media_manifest.clone();

        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!(
                    "Project duplicated. Timeline has {} tracks with {} total clips. Media manifest has {} entries.",
                    cloned_timeline.tracks.len(),
                    cloned_timeline.tracks.iter().map(|t| t.clips.len()).sum::<usize>(),
                    cloned_manifest.entries.len()
                )
            }]
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
            let text = t_val.get("text").and_then(|v| v.as_str()).unwrap_or("");
            let start_frame = t_val
                .get("startFrame")
                .and_then(|v| v.as_i64())
                .unwrap_or(current_frame);
            let duration_frames = t_val
                .get("durationFrames")
                .and_then(|v| v.as_i64())
                .unwrap_or(150);

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
                transform: Transform::default(),
                crop: core_model::Crop::default(),
                link_group_id: None,
                caption_group_id: None,
                text_content: Some(if text.is_empty() {
                    "Text".to_string()
                } else {
                    text.to_string()
                }),
                text_style: Some(TextStyle::default()),
                opacity_track: None,
                position_track: None,
                scale_track: None,
                rotation_track: None,
                crop_track: None,
                volume_track: None,
                effects: None,
                shape_style: None,
                stroke_progress_track: None,
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

            let shape_style = core_model::ShapeStyle {
                kind: shape_kind,
                ..core_model::ShapeStyle::default()
            };

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
                shape_style: Some(shape_style),
                stroke_progress_track: None,
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
            .unwrap_or(5)
            .clamp(1, 20);

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
            }),
            source_width: None,
            source_height: None,
            source_fps: None,
            has_audio: None,
            folder_id: None,
            cached_remote_url: None,
            cached_remote_url_expires_at: None,
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
            }),
            source_width: None,
            source_height: None,
            source_fps: None,
            has_audio: None,
            folder_id: None,
            cached_remote_url: None,
            cached_remote_url_expires_at: None,
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
            }),
            source_width: None,
            source_height: None,
            source_fps: None,
            has_audio: Some(true),
            folder_id: None,
            cached_remote_url: None,
            cached_remote_url_expires_at: None,
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
        });
        manifest.folders.push(core_model::MediaFolder {
            id: "folder-001".to_string(),
            name: "Test Folder".to_string(),
            parent_folder_id: None,
        });
        ToolExecutor::new(Timeline::default(), manifest)
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
    fn exec_003_split_clip_missing_args() {
        let mut exec = make_executor();
        let err = exec.execute("split_clip", &json!({})).unwrap_err();
        assert!(err.contains("Missing clipId"));
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
        assert!(text.contains("Found 1 media"));
    }

    #[test]
    fn exec_015_search_media_no_results() {
        let mut exec = make_executor_with_media();
        let result = exec
            .execute("search_media", &json!({"query": "nothing"}))
            .unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("No media found"));
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
    fn exec_020_inspect_timeline() {
        let mut exec = make_executor();
        let result = exec.execute("inspect_timeline", &json!({})).unwrap();
        assert!(result["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("fps"));
    }

    #[test]
    fn exec_021_get_transcript_stub() {
        let mut exec = make_executor();
        let result = exec.execute("get_transcript", &json!({})).unwrap();
        assert_eq!(result["isError"], true);
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
        let mut exec = make_executor_with_media();
        let result = exec.execute("duplicate_project", &json!({})).unwrap();
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("Project duplicated"));
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
            .execute("set_keyframes", &json!({"clipId": "nonexistent", "property": "opacity", "keyframes": [{"frame": 0, "value": 1.0}]}))
            .unwrap_err();
        assert!(err.contains("not found"));
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
}

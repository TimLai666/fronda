//! Tool execution dispatcher: routes agent tool calls to timeline engine.
//!
//! A ToolExecutor holds the mutable project state (Timeline + UndoStack)
//! and provides a single `execute()` entry point for the MCP server.

use crate::undo::{UndoCommand, UndoStack};
use core_model::{Clip, ClipType, Interpolation, Keyframe, KeyframeTrack, Timeline, Transform};
use serde_json::{json, Value};
use uuid::Uuid;

/// Runtime state for executing agent timeline tools.
pub struct ToolExecutor {
    timeline: Timeline,
    undo_stack: UndoStack,
}

impl ToolExecutor {
    pub fn new(timeline: Timeline) -> Self {
        Self {
            timeline,
            undo_stack: UndoStack::new(),
        }
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
            "undo" => self.cmd_undo(),
            "redo" => self.cmd_redo(),

            // ── Recognised but not yet wired ─────────────────────────────
            "add_texts" | "add_captions" | "add_shapes" | "apply_animation" | "apply_color"
            | "apply_effect" | "set_chroma_key" | "set_blend_mode" | "set_color_grade"
            | "inspect_color" | "create_folder" | "rename_folder" | "delete_folder"
            | "rename_media" | "delete_media" | "move_to_folder" | "import_media"
            | "import_folder" | "generate_video" | "generate_image" | "generate_audio"
            | "generate_music" | "upscale_media" | "duplicate_project" | "get_media"
            | "get_transcript" | "search_media" | "list_folders" | "list_models"
            | "inspect_media" | "inspect_timeline" => Ok(json!({
                "content": [{
                    "type": "text",
                    "text": format!("Tool '{tool_name}' is recognised but not yet wired to the timeline engine")
                }],
                "isError": true,
            })),

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
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_model::Timeline;

    fn make_executor() -> ToolExecutor {
        ToolExecutor::new(Timeline::default())
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
    fn exec_011_not_wired_tools_return_notice() {
        let mut exec = make_executor();
        let result = exec.execute("generate_video", &json!({})).unwrap();
        assert_eq!(result["isError"], true);
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("recognised but not yet wired"));
    }

    #[test]
    fn exec_012_set_keyframes_missing_clip() {
        let mut exec = make_executor();
        let err = exec
            .execute(
                "set_keyframes",
                &json!({"clipId": "nonexistent", "property": "opacity", "keyframes": [{"frame": 0, "value": 1.0}]}),
            )
            .unwrap_err();
        assert!(err.contains("not found"));
    }

    #[test]
    fn exec_013_ripple_delete_missing_args() {
        let mut exec = make_executor();
        let err = exec
            .execute("ripple_delete_ranges", &json!({}))
            .unwrap_err();
        assert!(err.contains("Missing trackIndex"));
    }
}

//! Mutation tool input validation (MUT-001 to MUT-023).
//!
//! Validates tool inputs before delegating to timeline_core editing functions.
//! Pure validation — no side effects.

use serde_json::Value;

/// Result of validating a tool input.
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationResult<T> {
    Ok(T),
    Error(String),
}

impl<T> ValidationResult<T> {
    pub fn into_ok(self) -> Option<T> {
        match self {
            ValidationResult::Ok(v) => Some(v),
            ValidationResult::Error(_) => None,
        }
    }

    pub fn into_error(self) -> Option<String> {
        match self {
            ValidationResult::Ok(_) => None,
            ValidationResult::Error(e) => Some(e),
        }
    }
}

/// Volume ceiling in linear gain: the single source of truth is the
/// inspector's dB scale (Swift VolumeScale.ceilingDb = +15 dB) — the tool
/// layer must accept the whole UI-reachable range.
pub fn volume_ceiling_linear() -> f64 {
    timeline_core::linear_from_db(timeline_core::VOLUME_CEILING_DB)
}

pub const MAX_TOOL_FRAME: i64 = 1_000_000_000;

/// Reject a frame-valued arg above `MAX_TOOL_FRAME` (upstream error shape).
pub fn require_frame_in_bounds(value: i64, label: &str) -> Result<(), String> {
    if value > MAX_TOOL_FRAME {
        Err(format!(
            "{label} {value} exceeds the maximum supported frame ({MAX_TOOL_FRAME})"
        ))
    } else {
        Ok(())
    }
}

/// Parsed and validated `split_clip` input.
#[derive(Debug, Clone, PartialEq)]
pub struct SplitClipInput {
    pub clip_id: String,
    pub frame: i64,
}

/// MUT-016: Validate `split_clip` input.
///
/// UNWIRED: the live tool is the #186 batch `split_clips` (`splits[{clipId,
/// atFrame}]` or `trackIndex`+`frames[]`); its per-cut checks live inline in
/// `cmd_split_clips`. This validates the legacy single-clip shape only.
pub fn validate_split_clip(input: &Value) -> ValidationResult<SplitClipInput> {
    let clip_id = match input.get("clipId").and_then(|v| v.as_str()) {
        Some(id) if !id.is_empty() => id.to_string(),
        _ => return ValidationResult::Error("split_clip: missing or empty 'clipId'".into()),
    };

    let frame = match input.get("frame").and_then(|v| v.as_i64()) {
        Some(f) if f >= 0 => f,
        Some(_) => {
            return ValidationResult::Error("split_clip: 'frame' must be non-negative".into())
        }
        None => return ValidationResult::Error("split_clip: missing or invalid 'frame'".into()),
    };
    if let Err(e) = require_frame_in_bounds(frame, "frame") {
        return ValidationResult::Error(format!("split_clip: {e}"));
    }

    ValidationResult::Ok(SplitClipInput { clip_id, frame })
}

/// Parsed and validated `set_clip_properties` input.
#[derive(Debug, Clone, PartialEq)]
pub struct SetClipPropertiesInput {
    pub clip_ids: Vec<String>,
    pub properties: Value,
    /// Text-only fields detected in properties (MUT-010).
    pub text_only_fields: Vec<String>,
    /// Whether setting scalar volume/opacity clears existing keyframes (MUT-011).
    pub clears_keyframes: bool,
    /// Timing-related properties detected (speed, durationFrames, trimStart, trimEnd) (MUT-012).
    pub timing_properties: Vec<String>,
}

/// MUT-009: `set_clip_properties` applies the same property set to every clip.
/// MUT-010: text-only fields rejected when any target is non-text.
/// MUT-011: Setting scalar volume/opacity clears existing keyframes.
/// MUT-012: timing changes propagate to linked partners.
///
/// The `clip_types` parameter provides the type of each target clip
/// (e.g. "video", "text", "audio"). When unavailable (None), text-only
/// field validation (MUT-010) is skipped.
pub fn validate_set_clip_properties(
    input: &Value,
    clip_types: Option<Vec<String>>,
) -> ValidationResult<SetClipPropertiesInput> {
    let clip_ids = match input.get("clipIds").and_then(|v| v.as_array()) {
        Some(arr) if !arr.is_empty() => {
            let ids: Vec<String> = arr
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
            if ids.is_empty() {
                return ValidationResult::Error(
                    "set_clip_properties: 'clipIds' must contain at least one valid string".into(),
                );
            }
            ids
        }
        _ => {
            return ValidationResult::Error(
                "set_clip_properties: missing or empty 'clipIds'".into(),
            )
        }
    };

    // v2: property fields are flat at the top level; the legacy nested
    // 'properties' object still validates for older callers.
    let properties = match input.get("properties") {
        Some(v) if v.is_object() => v.clone(),
        Some(_) => {
            return ValidationResult::Error(
                "set_clip_properties: 'properties' must be a JSON object".into(),
            )
        }
        None => input.clone(),
    };

    let mut text_only_fields: Vec<String> = Vec::new();
    let mut clears_keyframes = false;
    let mut timing_properties: Vec<String> = Vec::new();

    if let Some(obj) = properties.as_object() {
        // MUT-010: detect text-only fields and reject if any non-text clip targeted.
        // Names match the executor + Swift (content/color/alignment/background/border,
        // plus fontWeight for PR #65 and background/border for Issue #18).
        let text_fields = [
            "content",
            "fontSize",
            "fontName",
            "fontWeight",
            "alignment",
            "color",
            "background",
            "border",
        ];
        for field in &text_fields {
            if obj.contains_key(*field) {
                text_only_fields.push(field.to_string());
            }
        }
        if let Some(ref types) = clip_types {
            let has_non_text = types.iter().any(|t| t != "text");
            if has_non_text && !text_only_fields.is_empty() {
                return ValidationResult::Error(format!(
                    "set_clip_properties: text-only fields {:?} rejected for non-text clips",
                    text_only_fields
                ));
            }
        }

        // Issue #18: validate background / border color sub-fields.
        for fill_key in &["background", "border"] {
            if let Some(fill) = obj.get(*fill_key).and_then(|v| v.as_object()) {
                if let Some(color_val) = fill.get("color").and_then(|v| v.as_str()) {
                    if let Err(e) = crate::hex_color_parser::parse_hex_color(color_val) {
                        return ValidationResult::Error(format!(
                            "set_clip_properties: '{}.color' is not a valid hex color: {e}",
                            fill_key
                        ));
                    }
                }
            } else if obj.contains_key(*fill_key) {
                let val = obj.get(*fill_key).unwrap();
                if !val.is_object() {
                    return ValidationResult::Error(format!(
                        "set_clip_properties: '{}' must be an object with 'enabled' and 'color' fields",
                        fill_key
                    ));
                }
            }
        }

        // MUT-011: scalar volume/opacity (number, not object) clears keyframes
        clears_keyframes = obj.get("volume").and_then(|v| v.as_f64()).is_some()
            || obj.get("opacity").and_then(|v| v.as_f64()).is_some();

        // MUT-012: detect timing properties for linked-partner propagation
        let timing_keys = ["speed", "durationFrames", "trimStartFrame", "trimEndFrame"];
        for key in &timing_keys {
            if obj.contains_key(*key) {
                timing_properties.push(key.to_string());
            }
        }

        // PR #144: validate numeric ranges for speed, volume, opacity, trim
        if let Some(speed) = obj.get("speed").and_then(|v| v.as_f64()) {
            if speed <= 0.0 {
                return ValidationResult::Error(format!(
                    "set_clip_properties: 'speed' must be positive, got {speed}"
                ));
            }
        }
        if let Some(vol) = obj.get("volume").and_then(|v| v.as_f64()) {
            // Ceiling = Swift's inspector boost limit (+15 dB ~= 5.6234) -
            // the reachable state space, not #144's 0..1 agent-only bound.
            // The Rust inspector writes THROUGH this tool layer (Swift's
            // bypasses it), so 0..1 would silently break dB boosts.
            let ceiling = volume_ceiling_linear();
            if !(0.0..=ceiling).contains(&vol) {
                return ValidationResult::Error(format!(
                    "set_clip_properties: 'volume' must be between 0 and {ceiling:.4} (+15 dB), got {vol}"
                ));
            }
        }
        if let Some(opacity) = obj.get("opacity").and_then(|v| v.as_f64()) {
            if !(0.0..=1.0).contains(&opacity) {
                return ValidationResult::Error(format!(
                    "set_clip_properties: 'opacity' must be between 0 and 1, got {opacity}"
                ));
            }
        }
        if let Some(trim) = obj.get("trimStartFrame").and_then(|v| v.as_f64()) {
            if trim < 0.0 {
                return ValidationResult::Error(format!(
                    "set_clip_properties: 'trimStartFrame' must be >= 0, got {trim}"
                ));
            }
        }
        if let Some(trim) = obj.get("trimEndFrame").and_then(|v| v.as_f64()) {
            if trim < 0.0 {
                return ValidationResult::Error(format!(
                    "set_clip_properties: 'trimEndFrame' must be >= 0, got {trim}"
                ));
            }
        }
        // Upstream #264/#265: bound frame-valued properties.
        for key in ["durationFrames", "trimStartFrame", "trimEndFrame"] {
            if let Some(v) = obj.get(key).and_then(|v| v.as_i64()) {
                if let Err(e) = require_frame_in_bounds(v, key) {
                    return ValidationResult::Error(format!("set_clip_properties: {e}"));
                }
            }
        }
    }

    ValidationResult::Ok(SetClipPropertiesInput {
        clip_ids,
        properties,
        text_only_fields,
        clears_keyframes,
        timing_properties,
    })
}

/// Parsed and validated `set_keyframes` input. `keyframes` is one row per
/// keyframe: `(frame, values, interp)`, where `values` has the property's arity
/// (1 for scalars, 2 for position/scale, 4 for crop). Matches the executor's
/// row format exactly (shared parser).
#[derive(Debug, Clone, PartialEq)]
pub struct SetKeyframesInput {
    pub clip_id: String,
    pub property: String,
    pub keyframes: Vec<(i64, Vec<f64>, core_model::Interpolation)>,
}

/// MUT-013: replaces the full keyframe track for one (clipId, property) pair.
/// MUT-014: empty arrays clear the track.
/// MUT-015: keyframe rows are sorted; duplicate frames are last-write-wins.
///
/// Shares the executor's `[frame, ...values, interp?]` row parsing so validation
/// and execution never diverge; supports all six keyframable properties.
pub fn validate_set_keyframes(input: &Value) -> ValidationResult<SetKeyframesInput> {
    let clip_id = match input.get("clipId").and_then(|v| v.as_str()) {
        Some(id) if !id.is_empty() => id.to_string(),
        _ => return ValidationResult::Error("set_keyframes: missing or empty 'clipId'".into()),
    };

    let property = match input.get("property").and_then(|v| v.as_str()) {
        Some(p) if !p.is_empty() => p.to_string(),
        _ => return ValidationResult::Error("set_keyframes: missing or empty 'property'".into()),
    };

    let Some((arity, labels)) = crate::tool_exec::keyframe_property_arity(&property) else {
        return ValidationResult::Error(format!(
            "set_keyframes: unknown property '{property}' (expected opacity, volume, rotation, position, scale, or crop)"
        ));
    };

    let Some(kf_array) = input.get("keyframes").and_then(|v| v.as_array()) else {
        return ValidationResult::Error("set_keyframes: missing 'keyframes' array".into());
    };

    match crate::tool_exec::parse_keyframe_rows(kf_array, arity, labels) {
        Ok(keyframes) => ValidationResult::Ok(SetKeyframesInput {
            clip_id,
            property,
            keyframes,
        }),
        Err(e) => ValidationResult::Error(format!("set_keyframes: {e}")),
    }
}

/// Parsed and validated `remove_clips` input.
#[derive(Debug, Clone, PartialEq)]
pub struct RemoveClipsInput {
    pub clip_ids: Vec<String>,
    pub ripple: bool,
}

/// MUT-005: expands to linked groups.
pub fn validate_remove_clips(input: &Value) -> ValidationResult<RemoveClipsInput> {
    let clip_ids = match input.get("clipIds").and_then(|v| v.as_array()) {
        Some(arr) if !arr.is_empty() => arr
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect(),
        _ => return ValidationResult::Error("remove_clips: missing or empty 'clipIds'".into()),
    };

    let ripple = input
        .get("ripple")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    ValidationResult::Ok(RemoveClipsInput { clip_ids, ripple })
}

/// One parsed `add_clips` entry (tool-surface-v2 shape).
#[derive(Debug, Clone, PartialEq)]
pub struct AddClipEntryInput {
    pub media_ref: String,
    pub track_index: Option<usize>,
    pub start_frame: i64,
}

/// Parsed and validated `add_clips` input.
#[derive(Debug, Clone, PartialEq)]
pub struct AddClipsInput {
    pub entries: Vec<AddClipEntryInput>,
}

/// MUT-002: mixed explicit/omitted trackIndex rejected.
/// MUT-003: auto-create tracks when all entries omit trackIndex.
/// tool-surface-v2: entries carry mediaRef + startFrame; endFrame and source
/// are mutually exclusive.
pub fn validate_add_clips(input: &Value) -> ValidationResult<AddClipsInput> {
    let Some(arr) = input
        .get("entries")
        .and_then(|v| v.as_array())
        .filter(|a| !a.is_empty())
    else {
        return ValidationResult::Error("add_clips: missing or empty 'entries'".into());
    };
    let mut entries: Vec<AddClipEntryInput> = Vec::with_capacity(arr.len());
    for (i, e) in arr.iter().enumerate() {
        let Some(media_ref) = e
            .get("mediaRef")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
        else {
            return ValidationResult::Error(format!("add_clips: entries[{i}] missing 'mediaRef'"));
        };
        let Some(start_frame) = e.get("startFrame").and_then(|v| v.as_i64()) else {
            return ValidationResult::Error(format!(
                "add_clips: entries[{i}] missing 'startFrame'"
            ));
        };
        if start_frame < 0 {
            return ValidationResult::Error(format!(
                "add_clips: entries[{i}].startFrame must be >= 0"
            ));
        }
        if let Err(err) = require_frame_in_bounds(start_frame, "startFrame") {
            return ValidationResult::Error(format!("add_clips: entries[{i}]: {err}"));
        }
        if e.get("endFrame").is_some() && e.get("source").is_some() {
            return ValidationResult::Error(format!(
                "add_clips: entries[{i}]: endFrame and source are mutually exclusive"
            ));
        }
        let track_index = e
            .get("trackIndex")
            .and_then(|v| v.as_u64())
            .map(|i| i as usize);
        entries.push(AddClipEntryInput {
            media_ref: media_ref.to_string(),
            track_index,
            start_frame,
        });
    }
    let with_track = entries.iter().filter(|e| e.track_index.is_some()).count();
    if with_track != 0 && with_track != entries.len() {
        return ValidationResult::Error(
            "add_clips: set trackIndex on every entry or on none — mixing is rejected".into(),
        );
    }
    ValidationResult::Ok(AddClipsInput { entries })
}

/// Validate hex color strings (MUT-023).
///
/// Accepts #RGB, #RRGGBB, #RRGGBBAA.
/// Trims surrounding whitespace/newlines.
/// Rejects embedded/internal whitespace.
pub fn validate_hex_color(input: &str) -> ValidationResult<String> {
    match crate::hex_color_parser::parse_hex_color(input) {
        Ok(s) => ValidationResult::Ok(s),
        Err(e) => ValidationResult::Error(e),
    }
}

// === MUT-004: insert_clips ================================================

/// Parsed and validated `insert_clips` input.
#[derive(Debug, Clone, PartialEq)]
pub struct InsertClipsInput {
    pub media_refs: Vec<String>,
    pub track_index: usize,
    pub at_frame: i64,
}

/// MUT-004 (tool-surface-v2 shape): trackIndex + atFrame + entries laid
/// end-to-end; durationFrames and source are mutually exclusive per entry.
pub fn validate_insert_clips(input: &Value) -> ValidationResult<InsertClipsInput> {
    let track_index = match input.get("trackIndex").and_then(|v| v.as_u64()) {
        Some(idx) => idx as usize,
        None => return ValidationResult::Error("insert_clips: missing 'trackIndex'".into()),
    };

    let Some(arr) = input
        .get("entries")
        .and_then(|v| v.as_array())
        .filter(|a| !a.is_empty())
    else {
        return ValidationResult::Error("insert_clips: missing or empty 'entries'".into());
    };
    let mut media_refs: Vec<String> = Vec::with_capacity(arr.len());
    for (i, e) in arr.iter().enumerate() {
        let Some(media_ref) = e
            .get("mediaRef")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
        else {
            return ValidationResult::Error(format!(
                "insert_clips: entries[{i}] missing 'mediaRef'"
            ));
        };
        if e.get("durationFrames").is_some() && e.get("source").is_some() {
            return ValidationResult::Error(format!(
                "insert_clips: entries[{i}]: durationFrames and source are mutually exclusive"
            ));
        }
        media_refs.push(media_ref.to_string());
    }

    let at_frame = match input.get("atFrame").and_then(|v| v.as_i64()) {
        Some(f) if f >= 0 => f,
        Some(_) => {
            return ValidationResult::Error("insert_clips: 'atFrame' must be non-negative".into())
        }
        None => {
            return ValidationResult::Error("insert_clips: missing or invalid 'atFrame'".into())
        }
    };
    if let Err(e) = require_frame_in_bounds(at_frame, "atFrame") {
        return ValidationResult::Error(format!("insert_clips: {e}"));
    }

    ValidationResult::Ok(InsertClipsInput {
        media_refs,
        track_index,
        at_frame,
    })
}

// === MUT-006 (tool-surface-v2): manage_tracks ==============================

/// One parsed `manage_tracks` set entry.
#[derive(Debug, Clone, PartialEq)]
pub struct ManageTrackSetInput {
    pub index: i64,
    pub muted: Option<bool>,
    pub hidden: Option<bool>,
    pub sync_locked: Option<bool>,
}

/// Parsed and validated `manage_tracks` input (replaces remove_tracks).
#[derive(Debug, Clone, PartialEq)]
pub struct ManageTracksInput {
    /// (index, to) pairs, applied in order against the live track list.
    pub reorder: Vec<(i64, i64)>,
    pub set: Vec<ManageTrackSetInput>,
    pub remove: Vec<i64>,
}

/// MUT-006 (v2): shape-validate manage_tracks. Indexes must be non-negative
/// integers; every set entry needs at least one flag; an empty call (all
/// three arrays absent/empty) is refused.
pub fn validate_manage_tracks(input: &Value) -> ValidationResult<ManageTracksInput> {
    let mut reorder: Vec<(i64, i64)> = Vec::new();
    if let Some(arr) = input.get("reorder").and_then(|v| v.as_array()) {
        for (i, entry) in arr.iter().enumerate() {
            let (Some(index), Some(to)) = (
                entry.get("index").and_then(|v| v.as_i64()),
                entry.get("to").and_then(|v| v.as_i64()),
            ) else {
                return ValidationResult::Error(format!(
                    "manage_tracks: reorder[{i}] needs integer 'index' and 'to'."
                ));
            };
            if index < 0 || to < 0 {
                return ValidationResult::Error(format!(
                    "manage_tracks: reorder[{i}] indexes must be non-negative."
                ));
            }
            reorder.push((index, to));
        }
    }
    let mut set: Vec<ManageTrackSetInput> = Vec::new();
    if let Some(arr) = input.get("set").and_then(|v| v.as_array()) {
        for (i, entry) in arr.iter().enumerate() {
            let Some(index) = entry.get("index").and_then(|v| v.as_i64()) else {
                return ValidationResult::Error(format!(
                    "manage_tracks: set[{i}] needs an integer 'index'."
                ));
            };
            if index < 0 {
                return ValidationResult::Error(format!(
                    "manage_tracks: set[{i}].index must be non-negative."
                ));
            }
            let muted = entry.get("muted").and_then(|v| v.as_bool());
            let hidden = entry.get("hidden").and_then(|v| v.as_bool());
            let sync_locked = entry.get("syncLocked").and_then(|v| v.as_bool());
            if muted.is_none() && hidden.is_none() && sync_locked.is_none() {
                return ValidationResult::Error(format!(
                    "manage_tracks: set[{i}] needs at least one of muted, hidden, or syncLocked."
                ));
            }
            set.push(ManageTrackSetInput {
                index,
                muted,
                hidden,
                sync_locked,
            });
        }
    }
    let mut remove: Vec<i64> = Vec::new();
    if let Some(arr) = input.get("remove").and_then(|v| v.as_array()) {
        for (i, entry) in arr.iter().enumerate() {
            let Some(index) = entry.as_i64() else {
                return ValidationResult::Error(format!(
                    "manage_tracks: remove[{i}] must be an integer track index."
                ));
            };
            if index < 0 {
                return ValidationResult::Error(format!(
                    "manage_tracks: remove[{i}] must be non-negative."
                ));
            }
            remove.push(index);
        }
    }
    if reorder.is_empty() && set.is_empty() && remove.is_empty() {
        return ValidationResult::Error(
            "manage_tracks: pass at least one of reorder, set, or remove.".into(),
        );
    }
    ValidationResult::Ok(ManageTracksInput {
        reorder,
        set,
        remove,
    })
}

// === MUT-007: move_clips ==================================================

/// Parsed and validated `move_clips` input.
#[derive(Debug, Clone, PartialEq)]
pub struct MoveClipsInput {
    pub clip_ids: Vec<String>,
    pub to_track: Option<usize>,
    pub to_frame: Option<i64>,
}

/// MUT-007: Requires at least one of `toTrack` or `toFrame`.
/// tool-surface-v2: the primary shape is `moves: [{clipId, toTrack?,
/// toFrame?}]`; the legacy clipIds/toTrack/toFrame shape still validates.
pub fn validate_move_clips(input: &Value) -> ValidationResult<MoveClipsInput> {
    if let Some(arr) = input.get("moves").and_then(|v| v.as_array()) {
        if arr.is_empty() {
            return ValidationResult::Error("move_clips: 'moves' must be non-empty".into());
        }
        for (i, m) in arr.iter().enumerate() {
            if m.get("clipId").and_then(|v| v.as_str()).is_none() {
                return ValidationResult::Error(format!("move_clips: moves[{i}] missing 'clipId'"));
            }
            let to_track = m.get("toTrack").and_then(|v| v.as_i64());
            let to_frame = m.get("toFrame").and_then(|v| v.as_i64());
            if to_track.is_none() && to_frame.is_none() {
                return ValidationResult::Error(format!(
                    "move_clips: moves[{i}] requires at least one of 'toTrack' or 'toFrame'"
                ));
            }
            if let Some(f) = to_frame {
                if let Err(e) = require_frame_in_bounds(f, "toFrame") {
                    return ValidationResult::Error(format!("move_clips: moves[{i}]: {e}"));
                }
            }
        }
        return ValidationResult::Ok(MoveClipsInput {
            clip_ids: arr
                .iter()
                .filter_map(|m| m.get("clipId").and_then(|v| v.as_str()).map(String::from))
                .collect(),
            to_track: None,
            to_frame: None,
        });
    }
    let clip_ids = match input.get("clipIds").and_then(|v| v.as_array()) {
        Some(arr) if !arr.is_empty() => arr
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect(),
        _ => {
            return ValidationResult::Error("move_clips: missing or empty 'moves'".into());
        }
    };

    let to_track = input
        .get("toTrack")
        .and_then(|v| v.as_u64())
        .map(|i| i as usize);
    let to_frame = input.get("toFrame").and_then(|v| v.as_i64());
    if let Some(f) = to_frame {
        if let Err(e) = require_frame_in_bounds(f, "toFrame") {
            return ValidationResult::Error(format!("move_clips: {e}"));
        }
    }

    if to_track.is_none() && to_frame.is_none() {
        return ValidationResult::Error(
            "move_clips: requires at least one of 'toTrack' or 'toFrame'".into(),
        );
    }

    ValidationResult::Ok(MoveClipsInput {
        clip_ids,
        to_track,
        to_frame,
    })
}

// === MUT-008: move_clips linked partner ===================================

/// MUT-008: move_clips linked partner behavior.
///
/// At runtime, when a clip is moved, its linked partners follow
/// the same frame delta. This validation-level function only
/// checks that clip_ids are non-empty. The actual linked-partner
/// follow behavior requires timeline_core integration.
pub fn validate_move_clips_linked(clip_ids: &[String]) -> ValidationResult<Vec<String>> {
    if clip_ids.is_empty() {
        return ValidationResult::Error("move_clips_linked: 'clipIds' must not be empty".into());
    }
    ValidationResult::Ok(clip_ids.to_vec())
}

// === Upstream #176: duplicate_clips =======================================

/// Parsed and validated `duplicate_clips` input: per-entry (clipId, toFrame,
/// optional toTrack). Full existence/compatibility checks happen against the
/// live timeline in the executor; this is the pure shape/range gate.
#[derive(Debug, Clone, PartialEq)]
pub struct DuplicateClipsInput {
    pub entries: Vec<(String, Option<usize>, i64)>,
}

/// Requires a non-empty `entries` array; each entry needs a `clipId` and a
/// non-negative in-bounds `toFrame`; `toTrack`, when present, must be a
/// non-negative index.
pub fn validate_duplicate_clips(input: &Value) -> ValidationResult<DuplicateClipsInput> {
    let arr = match input.get("entries").and_then(|v| v.as_array()) {
        Some(a) if !a.is_empty() => a,
        _ => {
            return ValidationResult::Error(
                "duplicate_clips: 'entries' must be a non-empty array".into(),
            )
        }
    };
    let mut entries = Vec::with_capacity(arr.len());
    for (i, e) in arr.iter().enumerate() {
        let clip_id = match e.get("clipId").and_then(|v| v.as_str()) {
            Some(s) if !s.is_empty() => s.to_string(),
            _ => {
                return ValidationResult::Error(format!(
                    "duplicate_clips: entries[{i}] missing 'clipId'"
                ))
            }
        };
        let to_frame = match e.get("toFrame").and_then(|v| v.as_i64()) {
            Some(f) => f,
            None => {
                return ValidationResult::Error(format!(
                    "duplicate_clips: entries[{i}] missing or invalid 'toFrame'"
                ))
            }
        };
        if to_frame < 0 {
            return ValidationResult::Error(format!(
                "duplicate_clips: entries[{i}] toFrame must be >= 0 (got {to_frame})"
            ));
        }
        if let Err(e) = require_frame_in_bounds(to_frame, "toFrame") {
            return ValidationResult::Error(format!("duplicate_clips: entries[{i}]: {e}"));
        }
        let to_track = match e.get("toTrack") {
            None => None,
            Some(v) => match v.as_i64() {
                Some(t) if t >= 0 => Some(t as usize),
                _ => {
                    return ValidationResult::Error(format!(
                        "duplicate_clips: entries[{i}] toTrack must be a non-negative track index"
                    ))
                }
            },
        };
        entries.push((clip_id, to_track, to_frame));
    }
    ValidationResult::Ok(DuplicateClipsInput { entries })
}

// === MUT-017/018: ripple_delete_ranges ====================================

/// Parsed and validated `ripple_delete_ranges` input.
#[derive(Debug, Clone, PartialEq)]
pub struct RippleDeleteRangesInput {
    pub clip_id: Option<String>,
    pub track_index: Option<usize>,
    pub ranges: Vec<(i64, i64)>,
}

/// MUT-017: Requires exactly one of `clipId` or `trackIndex`.
/// MUT-018: Accepts optional `seconds` field for clip-scoped mode.
///
/// UNWIRED: this models Swift's clip-scoped contract (`startFrame`/`endFrame`
/// range keys); the live executor + schema take `trackIndex` + `ranges`
/// with `start`/`end` keys and have no clip-scoped mode yet.
pub fn validate_ripple_delete_ranges(input: &Value) -> ValidationResult<RippleDeleteRangesInput> {
    let clip_id = input
        .get("clipId")
        .and_then(|v| v.as_str())
        .map(String::from);
    let track_index = input
        .get("trackIndex")
        .and_then(|v| v.as_u64())
        .map(|i| i as usize);

    match (&clip_id, &track_index) {
        (Some(_), Some(_)) => {
            return ValidationResult::Error(
                "ripple_delete_ranges: cannot specify both 'clipId' and 'trackIndex'".into(),
            );
        }
        (None, None) => {
            return ValidationResult::Error(
                "ripple_delete_ranges: requires either 'clipId' or 'trackIndex'".into(),
            );
        }
        _ => {}
    }

    // Parse range entries from either `ranges` or `seconds` array.
    let ranges = if let Some(arr) = input.get("ranges").and_then(|v| v.as_array()) {
        let pairs: Vec<(i64, i64)> = arr
            .iter()
            .filter_map(|item| {
                let start = item.get("startFrame").and_then(|v| v.as_i64())?;
                let end = item.get("endFrame").and_then(|v| v.as_i64())?;
                Some((start, end))
            })
            .collect();
        if pairs.is_empty() {
            return ValidationResult::Error(
                "ripple_delete_ranges: 'ranges' must contain at least one valid range".into(),
            );
        }
        pairs
    } else if let Some(arr) = input.get("seconds").and_then(|v| v.as_array()) {
        // seconds mode only valid for clip-scoped
        if track_index.is_some() {
            return ValidationResult::Error(
                "ripple_delete_ranges: 'seconds' only valid with 'clipId' (clip-scoped)".into(),
            );
        }
        let pairs: Vec<(i64, i64)> = arr
            .iter()
            .filter_map(|item| {
                let start = item.get("startFrame").and_then(|v| v.as_i64())?;
                let end = item.get("endFrame").and_then(|v| v.as_i64())?;
                Some((start, end))
            })
            .collect();
        if pairs.is_empty() {
            return ValidationResult::Error(
                "ripple_delete_ranges: 'seconds' must contain at least one valid range".into(),
            );
        }
        pairs
    } else if clip_id.is_some() {
        // Clip-scoped: neither ranges nor seconds provided → empty means full clip delete
        Vec::new()
    } else {
        return ValidationResult::Error(
            "ripple_delete_ranges: requires 'ranges' array for track-scoped delete".into(),
        );
    };

    ValidationResult::Ok(RippleDeleteRangesInput {
        clip_id,
        track_index,
        ranges,
    })
}

// === MUT-019/020: add_texts ===============================================

/// Parsed and validated text entry for `add_texts`. Fields the executor
/// defaults stay optional: `content` (Swift key, preferred) or `text`,
/// `startFrame` (default: appended after the last clip), `durationFrames`
/// (default: 150).
#[derive(Debug, Clone, PartialEq)]
pub struct TextInput {
    pub text: Option<String>,
    pub start_frame: Option<i64>,
    pub duration_frames: Option<i64>,
}

/// Parsed and validated `add_texts` input.
#[derive(Debug, Clone, PartialEq)]
pub struct AddTextsInput {
    pub texts: Vec<TextInput>,
    pub track_index: Option<usize>,
}

/// MUT-019: Validate `add_texts` input. Auto-creates visual track when all
/// entries omit trackIndex.
/// MUT-020: Rejects audio tracks.
///
/// The `track_type` parameter identifies the target track's type
/// ("audio", "video", "text" etc.). When `track_type` is "audio",
/// the function returns an error since text cannot be added to
/// audio tracks. Pass `None` when track type information is
/// unavailable at validation time.
pub fn validate_add_texts(
    input: &Value,
    track_type: Option<String>,
) -> ValidationResult<AddTextsInput> {
    // MUT-020: reject audio tracks
    if let Some(ref tt) = track_type {
        if tt == "audio" {
            return ValidationResult::Error("add_texts: cannot add text to audio track".into());
        }
    }

    let texts = match input.get("texts").and_then(|v| v.as_array()) {
        Some(arr) if !arr.is_empty() => {
            let mut parsed: Vec<TextInput> = Vec::with_capacity(arr.len());
            for entry in arr {
                let text = entry
                    .get("content")
                    .or_else(|| entry.get("text"))
                    .and_then(|v| v.as_str())
                    .map(String::from);
                let start_frame = entry.get("startFrame").and_then(|v| v.as_i64());
                let duration_frames = entry.get("durationFrames").and_then(|v| v.as_i64());
                // One bad entry rejects the whole call (no partial state).
                if let Some(f) = start_frame {
                    if f < 0 {
                        return ValidationResult::Error(format!(
                            "add_texts: startFrame must be >= 0 (got {f})"
                        ));
                    }
                    if let Err(e) = require_frame_in_bounds(f, "startFrame") {
                        return ValidationResult::Error(format!("add_texts: {e}"));
                    }
                }
                if let Some(d) = duration_frames {
                    if d < 1 {
                        return ValidationResult::Error(format!(
                            "add_texts: durationFrames must be >= 1 (got {d})"
                        ));
                    }
                    if let Err(e) = require_frame_in_bounds(d, "durationFrames") {
                        return ValidationResult::Error(format!("add_texts: {e}"));
                    }
                }
                parsed.push(TextInput {
                    text,
                    start_frame,
                    duration_frames,
                });
            }
            parsed
        }
        _ => {
            return ValidationResult::Error("add_texts: missing or empty 'texts' array".into());
        }
    };

    let track_index = input
        .get("trackIndex")
        .and_then(|v| v.as_u64())
        .map(|i| i as usize);

    ValidationResult::Ok(AddTextsInput { texts, track_index })
}

// === MUT-021: add_captions ================================================

/// Parsed and validated `add_captions` input.
#[derive(Debug, Clone, PartialEq)]
pub struct AddCaptionsInput {
    pub clip_ids: Option<Vec<String>>,
    /// Optional BCP-47 language tag. PR #40.
    pub language: Option<String>,
    /// Optional max words per caption group (1-12). PR #92.
    pub words_per_caption: Option<u32>,
}

/// MUT-021: Supports explicit `clipIds` or auto-detect.
/// When `clipIds` is None, captions are auto-detected from the timeline.
pub fn validate_add_captions(input: &Value) -> ValidationResult<AddCaptionsInput> {
    let clip_ids = match input.get("clipIds").and_then(|v| v.as_array()) {
        Some(arr) => {
            let ids: Vec<String> = arr
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
            if ids.is_empty() {
                return ValidationResult::Error(
                    "add_captions: 'clipIds' must contain at least one valid string".into(),
                );
            }
            Some(ids)
        }
        None => None,
    };

    let language = input
        .get("language")
        .and_then(|v| v.as_str())
        .map(String::from);

    let words_per_caption = input
        .get("wordsPerCaption")
        .and_then(|v| v.as_u64())
        .map(|n| n.clamp(1, 12) as u32);

    ValidationResult::Ok(AddCaptionsInput {
        clip_ids,
        language,
        words_per_caption,
    })
}

// === AddShapes (PR #46) ==================================================

/// Parsed `add_shapes` input. PR #46.
#[derive(Debug, Clone, PartialEq)]
pub struct AddShapesInput {
    pub entries: Vec<Value>,
}

pub fn validate_add_shapes(input: &Value) -> ValidationResult<AddShapesInput> {
    let entries = match input.get("entries").and_then(|v| v.as_array()) {
        Some(arr) if !arr.is_empty() => arr.clone(),
        _ => {
            return ValidationResult::Error(
                "add_shapes: 'entries' must be a non-empty array".into(),
            );
        }
    };
    ValidationResult::Ok(AddShapesInput { entries })
}

// === ApplyAnimation (PR #46) ==============================================

/// Parsed `apply_animation` input. PR #46.
#[derive(Debug, Clone, PartialEq)]
pub struct ApplyAnimationInput {
    pub clip_id: String,
    pub preset: String,
    pub window_frames: Option<String>,
    pub intensity: Option<String>,
}

pub fn validate_apply_animation(input: &Value) -> ValidationResult<ApplyAnimationInput> {
    let clip_id = match input.get("clipId").and_then(|v| v.as_str()) {
        Some(id) if !id.is_empty() => id.to_string(),
        _ => {
            return ValidationResult::Error("apply_animation: 'clipId' is required".into());
        }
    };
    let preset = match input.get("preset").and_then(|v| v.as_str()) {
        Some(p) if !p.is_empty() => p.to_string(),
        _ => {
            return ValidationResult::Error("apply_animation: 'preset' is required".into());
        }
    };
    let window_frames = input
        .get("windowFrames")
        .and_then(|v| v.as_str())
        .map(String::from);
    let intensity = input
        .get("intensity")
        .and_then(|v| v.as_str())
        .map(String::from);

    ValidationResult::Ok(ApplyAnimationInput {
        clip_id,
        preset,
        window_frames,
        intensity,
    })
}

// === ApplyColor (PR #8) ===================================================

/// Parsed `apply_color` input. PR #8.
#[derive(Debug, Clone, PartialEq)]
pub struct ApplyColorInput {
    pub clip_id: String,
    pub exposure: Option<f64>,
    pub contrast: Option<f64>,
    pub saturation: Option<f64>,
    pub vibrance: Option<f64>,
    pub temperature: Option<f64>,
    pub tint: Option<f64>,
    pub highlights: Option<f64>,
    pub shadows: Option<f64>,
    pub blacks: Option<f64>,
    pub whites: Option<f64>,
    pub reset: Option<bool>,
}

pub fn validate_apply_color(input: &Value) -> ValidationResult<ApplyColorInput> {
    // v2: clipIds array (the legacy singular clipId still validates).
    let clip_id = match input.get("clipIds").and_then(|v| v.as_array()) {
        Some(arr) => match arr.iter().filter_map(|v| v.as_str()).next() {
            Some(first) if !arr.is_empty() => first.to_string(),
            _ => {
                return ValidationResult::Error(
                    "apply_color: 'clipIds' must contain at least one clip id".into(),
                )
            }
        },
        None => match input.get("clipId").and_then(|v| v.as_str()) {
            Some(id) if !id.is_empty() => id.to_string(),
            _ => {
                return ValidationResult::Error("apply_color: 'clipIds' is required".into());
            }
        },
    };
    ValidationResult::Ok(ApplyColorInput {
        clip_id,
        exposure: input.get("exposure").and_then(|v| v.as_f64()),
        contrast: input.get("contrast").and_then(|v| v.as_f64()),
        saturation: input.get("saturation").and_then(|v| v.as_f64()),
        vibrance: input.get("vibrance").and_then(|v| v.as_f64()),
        temperature: input.get("temperature").and_then(|v| v.as_f64()),
        tint: input.get("tint").and_then(|v| v.as_f64()),
        highlights: input.get("highlights").and_then(|v| v.as_f64()),
        shadows: input.get("shadows").and_then(|v| v.as_f64()),
        blacks: input.get("blacks").and_then(|v| v.as_f64()),
        whites: input.get("whites").and_then(|v| v.as_f64()),
        reset: input.get("reset").and_then(|v| v.as_bool()),
    })
}

// === ApplyEffect (PR #8) ===================================================

/// Parsed `apply_effect` input. PR #8.
#[derive(Debug, Clone, PartialEq)]
pub struct ApplyEffectInput {
    pub clip_id: String,
    pub effect_type: Option<String>,
    pub enabled: Option<bool>,
    pub remove: Option<Vec<String>>,
    pub intensity: Option<f64>,
}

pub fn validate_apply_effect(input: &Value) -> ValidationResult<ApplyEffectInput> {
    // v2: clipIds array + effects entries (the legacy singular clipId +
    // effectType shape still validates).
    let clip_id = match input.get("clipIds").and_then(|v| v.as_array()) {
        Some(arr) => match arr.iter().filter_map(|v| v.as_str()).next() {
            Some(first) if !arr.is_empty() => first.to_string(),
            _ => {
                return ValidationResult::Error(
                    "apply_effect: 'clipIds' must contain at least one clip id".into(),
                )
            }
        },
        None => match input.get("clipId").and_then(|v| v.as_str()) {
            Some(id) if !id.is_empty() => id.to_string(),
            _ => {
                return ValidationResult::Error("apply_effect: 'clipIds' is required".into());
            }
        },
    };
    let remove = input
        .get("remove")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .filter(|v: &Vec<String>| !v.is_empty());
    ValidationResult::Ok(ApplyEffectInput {
        clip_id,
        effect_type: input.get("type").and_then(|v| v.as_str()).map(String::from),
        enabled: input.get("enabled").and_then(|v| v.as_bool()),
        remove,
        intensity: input.get("intensity").and_then(|v| v.as_f64()),
    })
}

// === InspectColor (PR #8) ==================================================

/// Parsed `inspect_color` input. PR #8.
#[derive(Debug, Clone, PartialEq)]
pub struct InspectColorInput {
    pub clip_id: Option<String>,
    pub media_ref: Option<String>,
    pub reference: Option<String>,
}

pub fn validate_inspect_color(input: &Value) -> ValidationResult<InspectColorInput> {
    let clip_id = input
        .get("clipId")
        .and_then(|v| v.as_str())
        .map(String::from);
    let media_ref = input
        .get("mediaRef")
        .and_then(|v| v.as_str())
        .map(String::from);
    let reference = input
        .get("reference")
        .and_then(|v| v.as_str())
        .map(String::from);
    ValidationResult::Ok(InspectColorInput {
        clip_id,
        media_ref,
        reference,
    })
}

// === MUT-022 (tool-surface-v2): organize_media =============================

/// One parsed `organize_media` move entry.
#[derive(Debug, Clone, PartialEq)]
pub struct OrganizeMoveInput {
    pub items: Vec<String>,
    pub into: Option<String>,
}

/// One parsed `organize_media` rename entry.
#[derive(Debug, Clone, PartialEq)]
pub struct OrganizeRenameInput {
    pub item: String,
    pub name: String,
}

/// Parsed and validated `organize_media` input (replaces create_folder,
/// rename_folder, delete_folder, move_to_folder, rename_media, delete_media).
#[derive(Debug, Clone, PartialEq)]
pub struct OrganizeMediaInput {
    pub create_folders: Vec<String>,
    pub moves: Vec<OrganizeMoveInput>,
    pub renames: Vec<OrganizeRenameInput>,
    pub deletes: Vec<String>,
}

/// MUT-022 (v2): shape-validate organize_media. Items and paths must be
/// non-empty strings; every move needs items; every rename needs item + name;
/// an all-empty call is refused. Library resolution (asset id vs timeline id
/// vs folder path, ambiguity, cycles) happens in the executor.
pub fn validate_organize_media(input: &Value) -> ValidationResult<OrganizeMediaInput> {
    fn string_list(input: &Value, key: &str) -> Result<Vec<String>, String> {
        let Some(arr) = input.get(key).and_then(|v| v.as_array()) else {
            return Ok(Vec::new());
        };
        let mut out = Vec::new();
        for (i, v) in arr.iter().enumerate() {
            match v.as_str().map(str::trim).filter(|s| !s.is_empty()) {
                Some(s) => out.push(s.to_string()),
                None => {
                    return Err(format!(
                        "organize_media: {key}[{i}] must be a non-empty string."
                    ))
                }
            }
        }
        Ok(out)
    }
    let create_folders = match string_list(input, "createFolders") {
        Ok(v) => v,
        Err(e) => return ValidationResult::Error(e),
    };
    let deletes = match string_list(input, "deletes") {
        Ok(v) => v,
        Err(e) => return ValidationResult::Error(e),
    };
    let mut moves: Vec<OrganizeMoveInput> = Vec::new();
    if let Some(arr) = input.get("moves").and_then(|v| v.as_array()) {
        for (i, entry) in arr.iter().enumerate() {
            let items: Vec<String> = entry
                .get("items")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(str::trim))
                        .filter(|s| !s.is_empty())
                        .map(String::from)
                        .collect()
                })
                .unwrap_or_default();
            if items.is_empty() {
                return ValidationResult::Error(format!(
                    "organize_media: moves[{i}] needs a non-empty 'items' array."
                ));
            }
            let into = entry
                .get("into")
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(String::from);
            moves.push(OrganizeMoveInput { items, into });
        }
    }
    let mut renames: Vec<OrganizeRenameInput> = Vec::new();
    if let Some(arr) = input.get("renames").and_then(|v| v.as_array()) {
        for (i, entry) in arr.iter().enumerate() {
            let item = entry
                .get("item")
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty());
            let name = entry
                .get("name")
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty());
            let (Some(item), Some(name)) = (item, name) else {
                return ValidationResult::Error(format!(
                    "organize_media: renames[{i}] needs non-empty 'item' and 'name'."
                ));
            };
            renames.push(OrganizeRenameInput {
                item: item.to_string(),
                name: name.to_string(),
            });
        }
    }
    if create_folders.is_empty() && moves.is_empty() && renames.is_empty() && deletes.is_empty() {
        return ValidationResult::Error(
            "organize_media: pass at least one of createFolders, moves, renames, or deletes."
                .into(),
        );
    }
    ValidationResult::Ok(OrganizeMediaInput {
        create_folders,
        moves,
        renames,
        deletes,
    })
}

// === tool-surface-v2: close_project =========================================

/// Parsed and validated `close_project` input.
#[derive(Debug, Clone, PartialEq)]
pub struct CloseProjectInput {
    pub name: Option<String>,
    pub id: Option<String>,
    pub path: Option<String>,
}

/// Validate `close_project` input: name/id/path must be non-empty strings
/// when present; all-absent means "close the active project".
pub fn validate_close_project(input: &Value) -> ValidationResult<CloseProjectInput> {
    fn opt_string(input: &Value, key: &str) -> Result<Option<String>, String> {
        match input.get(key) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::String(s)) if !s.trim().is_empty() => Ok(Some(s.trim().to_string())),
            Some(_) => Err(format!(
                "close_project: '{key}' must be a non-empty string when present."
            )),
        }
    }
    let name = match opt_string(input, "name") {
        Ok(v) => v,
        Err(e) => return ValidationResult::Error(e),
    };
    let id = match opt_string(input, "id") {
        Ok(v) => v,
        Err(e) => return ValidationResult::Error(e),
    };
    let path = match opt_string(input, "path") {
        Ok(v) => v,
        Err(e) => return ValidationResult::Error(e),
    };
    ValidationResult::Ok(CloseProjectInput { name, id, path })
}

// === tool-surface-v2: import_media ==========================================

/// Parsed and validated `import_media` input (absorbs create_matte and
/// import_folder via source.matte / source.path-as-directory).
#[derive(Debug, Clone, PartialEq)]
pub struct ImportMediaInput {
    pub url: Option<String>,
    pub path: Option<String>,
    pub bytes: Option<String>,
    pub matte_hex: Option<String>,
    pub matte_aspect: Option<String>,
    pub mime_type: Option<String>,
    pub name: Option<String>,
    pub folder: Option<String>,
}

/// Validate `import_media` input: `source` must set exactly one of url, path,
/// bytes, or matte; matte needs `hex`; bytes needs `mimeType`.
pub fn validate_import_media(input: &Value) -> ValidationResult<ImportMediaInput> {
    let Some(source) = input.get("source").and_then(|v| v.as_object()) else {
        return ValidationResult::Error(
            "import_media: 'source' object is required — set exactly one of url, path, bytes, or matte."
                .into(),
        );
    };
    let get = |key: &str| {
        source
            .get(key)
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
    };
    let url = get("url");
    let path = get("path");
    let bytes = get("bytes");
    let matte = source.get("matte").and_then(|v| v.as_object());
    let set_count = [
        url.is_some(),
        path.is_some(),
        bytes.is_some(),
        matte.is_some(),
    ]
    .iter()
    .filter(|b| **b)
    .count();
    if set_count != 1 {
        return ValidationResult::Error(
            "import_media: source must set exactly one of url, path, bytes, or matte.".into(),
        );
    }
    let (matte_hex, matte_aspect) = match matte {
        Some(m) => {
            let hex = m
                .get("hex")
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(String::from);
            if hex.is_none() {
                return ValidationResult::Error(
                    "import_media: source.matte requires 'hex' (e.g. '#000000').".into(),
                );
            }
            (
                hex,
                m.get("aspectRatio")
                    .and_then(|v| v.as_str())
                    .map(String::from),
            )
        }
        None => (None, None),
    };
    let mime_type = get("mimeType");
    if bytes.is_some() && mime_type.is_none() {
        return ValidationResult::Error(
            "import_media: source.mimeType is required when bytes is set.".into(),
        );
    }
    ValidationResult::Ok(ImportMediaInput {
        url,
        path,
        bytes,
        matte_hex,
        matte_aspect,
        mime_type,
        name: input.get("name").and_then(|v| v.as_str()).map(String::from),
        folder: input
            .get("folder")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from),
    })
}

// === multicam-engine (upstream #283): manage_multicam / change_cam =========

/// One parsed `manage_multicam.create.members[]` entry.
#[derive(Debug, Clone, PartialEq)]
pub struct MulticamMemberInput {
    pub media_ref: String,
    pub kind: core_model::MulticamMemberKind,
    pub angle_label: Option<String>,
    pub offset_seconds: Option<f64>,
}

/// Parsed `manage_multicam.create`.
#[derive(Debug, Clone, PartialEq)]
pub struct ManageMulticamCreate {
    pub members: Vec<MulticamMemberInput>,
    pub name: Option<String>,
    pub master: Option<String>,
    pub start_frame: Option<i64>,
    pub search_window_seconds: Option<f64>,
}

/// Parsed and validated `manage_multicam` input (create wins when both
/// sections are present, mirroring the Swift executor's order).
#[derive(Debug, Clone, PartialEq)]
pub struct ManageMulticamInput {
    pub create: Option<ManageMulticamCreate>,
    pub ungroup_group_id: Option<String>,
}

/// Shape-validate `manage_multicam`: `create.members` needs >= 2 entries with
/// mediaRef + a valid kind and no duplicate mediaRefs; `ungroup` needs groupId.
pub fn validate_manage_multicam(input: &Value) -> ValidationResult<ManageMulticamInput> {
    if let Some(raw) = input.get("create") {
        let Some(body) = raw.as_object() else {
            return ValidationResult::Error("manage_multicam.create must be an object.".into());
        };
        let Some(raw_members) = body.get("members").and_then(|v| v.as_array()) else {
            return ValidationResult::Error(
                "create.members requires at least two entries (cameras and mics).".into(),
            );
        };
        if raw_members.len() < 2 {
            return ValidationResult::Error(
                "create.members requires at least two entries (cameras and mics).".into(),
            );
        }
        let mut members = Vec::with_capacity(raw_members.len());
        let mut seen_refs: std::collections::HashSet<String> = std::collections::HashSet::new();
        for (i, raw) in raw_members.iter().enumerate() {
            let Some(media_ref) = raw
                .get("mediaRef")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
            else {
                return ValidationResult::Error(format!(
                    "create.members[{i}]: mediaRef is required."
                ));
            };
            let Some(kind) = raw
                .get("kind")
                .and_then(|v| v.as_str())
                .and_then(core_model::MulticamMemberKind::from_str)
            else {
                return ValidationResult::Error(format!(
                    "create.members[{i}]: kind must be angle, mic, or both."
                ));
            };
            if !seen_refs.insert(media_ref.to_string()) {
                return ValidationResult::Error(format!(
                    "create.members[{i}]: duplicate mediaRef {media_ref}"
                ));
            }
            members.push(MulticamMemberInput {
                media_ref: media_ref.to_string(),
                kind,
                angle_label: raw
                    .get("angleLabel")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                offset_seconds: raw.get("offsetSeconds").and_then(|v| v.as_f64()),
            });
        }
        return ValidationResult::Ok(ManageMulticamInput {
            create: Some(ManageMulticamCreate {
                members,
                name: body.get("name").and_then(|v| v.as_str()).map(String::from),
                master: body
                    .get("master")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                start_frame: body.get("startFrame").and_then(|v| v.as_i64()),
                search_window_seconds: body.get("searchWindowSeconds").and_then(|v| v.as_f64()),
            }),
            ungroup_group_id: None,
        });
    }
    if let Some(raw) = input.get("ungroup") {
        let Some(body) = raw.as_object() else {
            return ValidationResult::Error("manage_multicam.ungroup must be an object.".into());
        };
        let Some(group_id) = body
            .get("groupId")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
        else {
            return ValidationResult::Error("groupId is required.".into());
        };
        return ValidationResult::Ok(ManageMulticamInput {
            create: None,
            ungroup_group_id: Some(group_id.to_string()),
        });
    }
    ValidationResult::Error("Pass create or ungroup.".into())
}

/// One parsed `change_cam.entries[]` row: full-frame (`angle`) or a layout.
#[derive(Debug, Clone, PartialEq)]
pub struct ChangeCamEntry {
    pub range: (i64, i64),
    pub angle: Option<String>,
    pub layout: Option<core_model::VideoLayout>,
    pub angles: Vec<String>,
}

/// Parsed and validated `change_cam` input.
#[derive(Debug, Clone, PartialEq)]
pub struct ChangeCamInput {
    pub group_id: Option<String>,
    pub clip_id: Option<String>,
    pub entries: Vec<ChangeCamEntry>,
}

/// Shape-validate `change_cam`: non-empty entries, each with a [start, end)
/// integer range (start < end) and EITHER angle OR layout + angles.
pub fn validate_change_cam(input: &Value) -> ValidationResult<ChangeCamInput> {
    let Some(raw_entries) = input
        .get("entries")
        .and_then(|v| v.as_array())
        .filter(|a| !a.is_empty())
    else {
        return ValidationResult::Error(
            "entries requires at least one {range, angle} entry.".into(),
        );
    };
    let mut entries = Vec::with_capacity(raw_entries.len());
    for (i, raw) in raw_entries.iter().enumerate() {
        let path = format!("entries[{i}]");
        let range = raw.get("range").and_then(|v| v.as_array()).and_then(|r| {
            if r.len() != 2 {
                return None;
            }
            let a = r[0].as_i64()?;
            let b = r[1].as_i64()?;
            (a < b).then_some((a, b))
        });
        let Some(range) = range else {
            return ValidationResult::Error(format!(
                "{path}: range must be [startFrame, endFrame) with start < end."
            ));
        };
        let angle = raw.get("angle").and_then(|v| v.as_str()).map(String::from);
        if let Some(layout_raw) = raw.get("layout").and_then(|v| v.as_str()) {
            if angle.is_some() {
                return ValidationResult::Error(format!(
                    "{path}: pass angle for a full-frame switch OR layout + angles, not both."
                ));
            }
            let layout = core_model::VideoLayout::from_str(layout_raw)
                .filter(|l| *l != core_model::VideoLayout::Full);
            let Some(layout) = layout else {
                let valid: Vec<&str> = core_model::VideoLayout::ALL
                    .iter()
                    .filter(|l| **l != core_model::VideoLayout::Full)
                    .map(|l| l.as_str())
                    .collect();
                return ValidationResult::Error(format!(
                    "{path}: unknown layout '{layout_raw}'. Valid: {}. For full frame, pass angle instead.",
                    valid.join(", ")
                ));
            };
            let angles: Vec<String> = raw
                .get("angles")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            if angles.is_empty() {
                let slots: Vec<&str> = layout.slots().iter().map(|s| s.id).collect();
                return ValidationResult::Error(format!(
                    "{path}: layout needs angles — angleLabels in slot order ({}); fewer than slots leaves cells empty.",
                    slots.join(", ")
                ));
            }
            entries.push(ChangeCamEntry {
                range,
                angle: None,
                layout: Some(layout),
                angles,
            });
        } else {
            let Some(angle) = angle else {
                return ValidationResult::Error(format!(
                    "{path}: angle is required for a full-frame switch."
                ));
            };
            entries.push(ChangeCamEntry {
                range,
                angle: Some(angle),
                layout: None,
                angles: Vec::new(),
            });
        }
    }
    ValidationResult::Ok(ChangeCamInput {
        group_id: input
            .get("groupId")
            .and_then(|v| v.as_str())
            .map(String::from),
        clip_id: input
            .get("clipId")
            .and_then(|v| v.as_str())
            .map(String::from),
        entries,
    })
}

// === Upstream #99: set_chroma_key ===========================================

/// Parsed and validated `set_chroma_key` input.
#[derive(Debug, Clone, PartialEq)]
pub struct SetChromaKeyInput {
    pub clip_id: String,
    pub enabled: Option<bool>,
    pub color: Option<String>,
    pub threshold: Option<f64>,
    pub smoothness: Option<f64>,
}

/// Validate `set_chroma_key` input.
pub fn validate_set_chroma_key(input: &Value) -> ValidationResult<SetChromaKeyInput> {
    let clip_id = match input.get("clipId").and_then(|v| v.as_str()) {
        Some(id) if !id.is_empty() => id.to_string(),
        _ => return ValidationResult::Error("set_chroma_key: missing or empty 'clipId'".into()),
    };
    let enabled = input.get("enabled").and_then(|v| v.as_bool());
    let color = input
        .get("color")
        .and_then(|v| v.as_str())
        .map(String::from);
    let threshold = input.get("threshold").and_then(|v| v.as_f64());
    let smoothness = input.get("smoothness").and_then(|v| v.as_f64());
    ValidationResult::Ok(SetChromaKeyInput {
        clip_id,
        enabled,
        color,
        threshold,
        smoothness,
    })
}

// === Upstream #99: set_blend_mode ===========================================

/// Parsed and validated `set_blend_mode` input.
#[derive(Debug, Clone, PartialEq)]
pub struct SetBlendModeInput {
    pub clip_id: String,
    pub mode: String,
}

/// Validate `set_blend_mode` input.
pub fn validate_set_blend_mode(input: &Value) -> ValidationResult<SetBlendModeInput> {
    let clip_id = match input.get("clipId").and_then(|v| v.as_str()) {
        Some(id) if !id.is_empty() => id.to_string(),
        _ => return ValidationResult::Error("set_blend_mode: missing or empty 'clipId'".into()),
    };
    let mode = match input.get("mode").and_then(|v| v.as_str()) {
        Some(m) => m.to_string(),
        _ => return ValidationResult::Error("set_blend_mode: missing 'mode'".into()),
    };
    ValidationResult::Ok(SetBlendModeInput { clip_id, mode })
}

// === Upstream #99: set_color_grade ==========================================

/// Parsed and validated `set_color_grade` input.
#[derive(Debug, Clone, PartialEq)]
pub struct SetColorGradeInput {
    pub clip_id: String,
    pub exposure: Option<f64>,
    pub contrast: Option<f64>,
    pub saturation: Option<f64>,
    pub temperature: Option<f64>,
}

/// Validate `set_color_grade` input.
pub fn validate_set_color_grade(input: &Value) -> ValidationResult<SetColorGradeInput> {
    let clip_id = match input.get("clipId").and_then(|v| v.as_str()) {
        Some(id) if !id.is_empty() => id.to_string(),
        _ => return ValidationResult::Error("set_color_grade: missing or empty 'clipId'".into()),
    };
    ValidationResult::Ok(SetColorGradeInput {
        clip_id,
        exposure: input.get("exposure").and_then(|v| v.as_f64()),
        contrast: input.get("contrast").and_then(|v| v.as_f64()),
        saturation: input.get("saturation").and_then(|v| v.as_f64()),
        temperature: input.get("temperature").and_then(|v| v.as_f64()),
    })
}

// === Upstream #6: generate_music ============================================

/// Parsed and validated `generate_music` input.
#[derive(Debug, Clone, PartialEq)]
pub struct GenerateMusicInput {
    pub prompt: String,
    pub duration: Option<f64>,
    pub style: Option<String>,
}

/// Validate `generate_music` input.
pub fn validate_generate_music(input: &Value) -> ValidationResult<GenerateMusicInput> {
    let prompt = match input.get("prompt").and_then(|v| v.as_str()) {
        Some(p) if !p.is_empty() => p.to_string(),
        _ => return ValidationResult::Error("generate_music: missing or empty 'prompt'".into()),
    };
    let duration = input.get("duration").and_then(|v| v.as_f64());
    let style = input
        .get("style")
        .and_then(|v| v.as_str())
        .map(String::from);
    ValidationResult::Ok(GenerateMusicInput {
        prompt,
        duration,
        style,
    })
}

// === Upstream #67: duplicate_project ========================================

/// Parsed and validated `duplicate_project` input.
#[derive(Debug, Clone, PartialEq)]
pub struct DuplicateProjectInput;

/// Validate `duplicate_project` input.
pub fn validate_duplicate_project(input: &Value) -> ValidationResult<DuplicateProjectInput> {
    let _ = input;
    ValidationResult::Ok(DuplicateProjectInput)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ---- Upstream #176: duplicate_clips ---------------------------------

    #[test]
    fn duplicate_clips_valid_entries_parse() {
        let input = json!({"entries": [
            {"clipId": "c1", "toFrame": 100},
            {"clipId": "c2", "toTrack": 1, "toFrame": 0}
        ]});
        let parsed = validate_duplicate_clips(&input)
            .into_ok()
            .expect("valid entries");
        assert_eq!(parsed.entries.len(), 2);
        assert_eq!(parsed.entries[0], ("c1".to_string(), None, 100));
        assert_eq!(parsed.entries[1], ("c2".to_string(), Some(1), 0));
    }

    #[test]
    fn duplicate_clips_empty_entries_rejected() {
        let err = validate_duplicate_clips(&json!({"entries": []}))
            .into_error()
            .unwrap();
        assert!(err.contains("non-empty"), "err={err}");
    }

    #[test]
    fn duplicate_clips_negative_to_frame_rejected() {
        let err = validate_duplicate_clips(&json!({"entries": [{"clipId": "c1", "toFrame": -3}]}))
            .into_error()
            .unwrap();
        assert!(err.contains("toFrame must be >= 0"), "err={err}");
    }

    #[test]
    fn duplicate_clips_missing_to_frame_rejected() {
        let err = validate_duplicate_clips(&json!({"entries": [{"clipId": "c1"}]}))
            .into_error()
            .unwrap();
        assert!(err.contains("toFrame"), "err={err}");
    }

    #[test]
    fn duplicate_clips_frame_ceiling_enforced() {
        let over = MAX_TOOL_FRAME + 1;
        let err =
            validate_duplicate_clips(&json!({"entries": [{"clipId": "c1", "toFrame": over}]}))
                .into_error()
                .unwrap();
        assert!(err.contains("maximum supported frame"), "err={err}");
    }

    // ---- MUT-016: split_clip --------------------------------------------

    #[test]
    fn mut_016_split_clip_valid() {
        let input = json!({"clipId": "clip-001", "frame": 50});
        let result = validate_split_clip(&input);
        let parsed = result.into_ok().expect("MUT-016: valid input");
        assert_eq!(parsed.clip_id, "clip-001");
        assert_eq!(parsed.frame, 50);
    }

    #[test]
    fn mut_016_split_clip_missing_clip_id() {
        let input = json!({"frame": 50});
        let result = validate_split_clip(&input);
        assert!(
            result.into_error().unwrap().contains("clipId"),
            "MUT-016: missing clipId"
        );
    }

    #[test]
    fn mut_016_split_clip_negative_frame() {
        let input = json!({"clipId": "c1", "frame": -1});
        let result = validate_split_clip(&input);
        assert!(result.into_error().is_some());
    }

    // ---- Upstream #264/#265: MAX_TOOL_FRAME ceiling -----------------------

    #[test]
    fn frame_bound_helper_accepts_ceiling_rejects_above() {
        assert!(require_frame_in_bounds(MAX_TOOL_FRAME, "frame").is_ok());
        assert!(require_frame_in_bounds(0, "frame").is_ok());
        let err = require_frame_in_bounds(MAX_TOOL_FRAME + 1, "frame").unwrap_err();
        assert!(err.contains("exceeds the maximum supported frame"));
    }

    #[test]
    fn frame_bound_split_insert_move_reject_i64_max() {
        assert!(
            validate_split_clip(&json!({"clipId": "c1", "frame": i64::MAX}))
                .into_error()
                .unwrap()
                .contains("exceeds the maximum supported frame")
        );
        assert!(validate_insert_clips(
            &json!({"trackIndex": 0, "entries": [{"mediaRef": "m1"}], "atFrame": i64::MAX})
        )
        .into_error()
        .unwrap()
        .contains("exceeds the maximum supported frame"));
        assert!(
            validate_move_clips(&json!({"clipIds": ["c1"], "toFrame": i64::MAX}))
                .into_error()
                .unwrap()
                .contains("exceeds the maximum supported frame")
        );
    }

    #[test]
    fn frame_bound_set_clip_properties_rejects_i64_max_timing() {
        for key in ["durationFrames", "trimStartFrame", "trimEndFrame"] {
            let input = json!({"clipIds": ["c1"], "properties": {key: i64::MAX}});
            let err = validate_set_clip_properties(&input, None)
                .into_error()
                .unwrap_or_else(|| panic!("{key} should be rejected"));
            assert!(
                err.contains("exceeds the maximum supported frame"),
                "{key}: {err}"
            );
        }
    }

    // ---- Upstream #212: slow speeds below 0.25x are legal ------------------

    #[test]
    fn speed_below_quarter_accepted_zero_rejected() {
        let ok = validate_set_clip_properties(
            &json!({"clipIds": ["c1"], "properties": {"speed": 0.1}}),
            None,
        );
        assert!(ok.into_ok().is_some(), "0.1x speed is valid");
        let err = validate_set_clip_properties(
            &json!({"clipIds": ["c1"], "properties": {"speed": 0.0}}),
            None,
        );
        assert!(err.into_error().is_some(), "0 speed rejected");
    }

    // ---- MUT-009: set_clip_properties -----------------------------------

    #[test]
    fn mut_009_set_clip_properties_valid() {
        let input = json!({
            "clipIds": ["c1", "c2"],
            "properties": {"speed": 2.0}
        });
        let result = validate_set_clip_properties(&input, None);
        let parsed = result.into_ok().expect("MUT-009: valid");
        assert_eq!(parsed.clip_ids.len(), 2);
        assert!(parsed.text_only_fields.is_empty());
        assert!(!parsed.clears_keyframes);
        // "speed" is a timing property (MUT-012)
        assert_eq!(parsed.timing_properties, vec!["speed"]);
    }

    #[test]
    fn mut_009_set_clip_properties_empty_ids() {
        let input = json!({
            "clipIds": [],
            "properties": {}
        });
        let result = validate_set_clip_properties(&input, None);
        assert!(result.into_error().is_some(), "MUT-009: empty ids rejected");
    }

    #[test]
    fn mut_009_set_clip_properties_flat_v2_shape_accepted() {
        // v2: fields sit at the top level; no 'properties' object required.
        let input = json!({"clipIds": ["c1"], "opacity": 0.5});
        let result = validate_set_clip_properties(&input, None);
        assert!(result.into_ok().is_some());
    }

    // ---- MUT-013/014/015: set_keyframes ----------------------------------

    #[test]
    fn mut_013_set_keyframes_valid() {
        let input = json!({
            "clipId": "c1",
            "property": "opacity",
            "keyframes": [[0, 1.0], [50, 0.5]]
        });
        let result = validate_set_keyframes(&input);
        let parsed = result.into_ok().expect("MUT-013: valid");
        assert_eq!(parsed.keyframes.len(), 2);
        assert_eq!(parsed.keyframes[0].0, 0);
        assert_eq!(parsed.keyframes[0].1, vec![1.0]);
    }

    #[test]
    fn mut_014_set_keyframes_empty_clears_track() {
        let input = json!({
            "clipId": "c1",
            "property": "opacity",
            "keyframes": []
        });
        let result = validate_set_keyframes(&input);
        let parsed = result.into_ok().expect("MUT-014: empty clears track");
        assert!(parsed.keyframes.is_empty());
    }

    #[test]
    fn mut_015_keyframes_sorted_deduped() {
        let input = json!({
            "clipId": "c1",
            "property": "opacity",
            "keyframes": [[50, 0.5], [0, 1.0], [50, 0.8]]
        });
        let result = validate_set_keyframes(&input);
        let parsed = result.into_ok().expect("MUT-015: sorted & deduped");
        assert_eq!(parsed.keyframes.len(), 2, "MUT-015: two unique frames");
        // Last-write-wins at frame 50: value should be 0.8
        assert_eq!(parsed.keyframes[1].0, 50);
        assert_eq!(parsed.keyframes[1].1, vec![0.8]);
    }

    #[test]
    fn mut_013_set_keyframes_position_pair() {
        let input = json!({
            "clipId": "c1",
            "property": "position",
            "keyframes": [[0, 0.1, 0.2]]
        });
        let parsed = validate_set_keyframes(&input)
            .into_ok()
            .expect("position pair valid");
        assert_eq!(parsed.keyframes[0].1, vec![0.1, 0.2]);
    }

    #[test]
    fn mut_013_set_keyframes_unknown_property_rejected() {
        let input = json!({"clipId": "c1", "property": "warp", "keyframes": [[0, 1.0]]});
        assert!(matches!(
            validate_set_keyframes(&input),
            ValidationResult::Error(_)
        ));
    }

    // ---- MUT-005: remove_clips ------------------------------------------

    #[test]
    fn mut_005_remove_clips_valid() {
        let input = json!({"clipIds": ["c1", "c2"], "ripple": true});
        let result = validate_remove_clips(&input);
        let parsed = result.into_ok().expect("MUT-005: valid");
        assert_eq!(parsed.clip_ids.len(), 2);
        assert!(parsed.ripple);
    }

    #[test]
    fn mut_005_remove_clips_default_no_ripple() {
        let input = json!({"clipIds": ["c1"]});
        let result = validate_remove_clips(&input);
        let parsed = result.into_ok().expect("MUT-005: default ripple=false");
        assert!(!parsed.ripple, "default ripple=false");
    }

    // ---- MUT-002/003: add_clips -----------------------------------------

    #[test]
    fn mut_002_add_clips_valid_with_track() {
        let input = json!({"entries": [
            {"mediaRef": "m1", "trackIndex": 0, "startFrame": 0},
            {"mediaRef": "m2", "trackIndex": 0, "startFrame": 60},
        ]});
        let result = validate_add_clips(&input);
        let parsed = result.into_ok().expect("MUT-002: valid with track");
        assert_eq!(parsed.entries[0].track_index, Some(0));
        assert_eq!(parsed.entries[1].start_frame, 60);
    }

    #[test]
    fn mut_002_add_clips_valid_without_track() {
        let input = json!({"entries": [{"mediaRef": "m1", "startFrame": 0}]});
        let result = validate_add_clips(&input);
        let parsed = result.into_ok().expect("MUT-002: valid without track");
        assert_eq!(parsed.entries[0].track_index, None);
    }

    #[test]
    fn mut_002_add_clips_rejects_empty() {
        let input = json!({"entries": []});
        let result = validate_add_clips(&input);
        assert!(result.into_error().is_some());
    }

    #[test]
    fn mut_002_add_clips_rejects_mixed_track_index() {
        let input = json!({"entries": [
            {"mediaRef": "m1", "trackIndex": 0, "startFrame": 0},
            {"mediaRef": "m2", "startFrame": 60},
        ]});
        let err = validate_add_clips(&input)
            .into_error()
            .expect("mixed rejected");
        assert!(err.contains("mixing"), "{err}");
    }

    #[test]
    fn mut_002_add_clips_rejects_end_frame_with_source() {
        let input = json!({"entries": [
            {"mediaRef": "m1", "startFrame": 0, "endFrame": 60, "source": [0.0, 2.0]},
        ]});
        let err = validate_add_clips(&input).into_error().expect("exclusive");
        assert!(err.contains("mutually exclusive"), "{err}");
    }

    // ---- MUT-023: hex color ---------------------------------------------

    #[test]
    fn mut_023_hex_color_valid_formats() {
        assert!(validate_hex_color("#fff").into_ok().is_some());
        assert!(validate_hex_color("#ffffff").into_ok().is_some());
        assert!(validate_hex_color("#ffffffff").into_ok().is_some());
        assert!(validate_hex_color("#FF00AA").into_ok().is_some());
    }

    #[test]
    fn mut_023_hex_color_trims_spaces() {
        let result = validate_hex_color("  #ff0000  ");
        assert_eq!(result.into_ok().unwrap(), "#ff0000");
    }

    #[test]
    fn mut_023_hex_color_rejects_internal_whitespace() {
        let result = validate_hex_color("#ff 0000");
        assert!(result.into_error().is_some());
    }

    #[test]
    fn mut_023_hex_color_rejects_invalid_length() {
        assert!(validate_hex_color("#ff").into_error().is_some());
        assert!(validate_hex_color("#fffff").into_error().is_some());
    }

    #[test]
    fn mut_023_hex_color_rejects_invalid_chars() {
        assert!(validate_hex_color("#gggggg").into_error().is_some());
    }

    #[test]
    fn mut_023_hex_color_rejects_no_hash() {
        assert!(validate_hex_color("ff0000").into_error().is_some());
    }

    // ---- MUT-004: insert_clips ------------------------------------------

    #[test]
    fn mut_004_insert_clips_valid() {
        let input = json!({
            "trackIndex": 1,
            "entries": [{"mediaRef": "m1"}, {"mediaRef": "m2"}],
            "atFrame": 120
        });
        let result = validate_insert_clips(&input);
        let parsed = result.into_ok().expect("MUT-004: valid");
        assert_eq!(parsed.track_index, 1);
        assert_eq!(parsed.media_refs, vec!["m1", "m2"]);
        assert_eq!(parsed.at_frame, 120);
    }

    #[test]
    fn mut_004_insert_clips_requires_track_index() {
        let input = json!({"entries": [{"mediaRef": "m1"}], "atFrame": 0});
        let result = validate_insert_clips(&input);
        let err = result.into_error().expect("MUT-004: missing trackIndex");
        assert!(err.contains("trackIndex"));
    }

    #[test]
    fn mut_004_insert_clips_requires_entries() {
        let input = json!({"trackIndex": 0, "atFrame": 0});
        let result = validate_insert_clips(&input);
        assert!(result.into_error().is_some());
    }

    #[test]
    fn mut_004_insert_clips_requires_non_negative_frame() {
        let input = json!({"trackIndex": 0, "mediaIds": ["m1"], "frame": -5});
        let result = validate_insert_clips(&input);
        assert!(result.into_error().is_some());
    }

    // ---- MUT-006 (v2): manage_tracks -------------------------------------

    #[test]
    fn mut_006_manage_tracks_valid_all_actions() {
        let input = json!({
            "reorder": [{"index": 2, "to": 0}],
            "set": [{"index": 1, "muted": true, "syncLocked": false}],
            "remove": [3],
        });
        let parsed = validate_manage_tracks(&input)
            .into_ok()
            .expect("MUT-006: valid");
        assert_eq!(parsed.reorder, vec![(2, 0)]);
        assert_eq!(parsed.set[0].index, 1);
        assert_eq!(parsed.set[0].muted, Some(true));
        assert_eq!(parsed.set[0].hidden, None);
        assert_eq!(parsed.set[0].sync_locked, Some(false));
        assert_eq!(parsed.remove, vec![3]);
    }

    #[test]
    fn mut_006_manage_tracks_empty_call_rejected() {
        let err = validate_manage_tracks(&json!({}))
            .into_error()
            .expect("empty call refused");
        assert!(err.contains("at least one of"), "{err}");
        assert!(
            validate_manage_tracks(&json!({"reorder": [], "set": [], "remove": []}))
                .into_error()
                .is_some()
        );
    }

    #[test]
    fn mut_006_manage_tracks_set_needs_a_flag() {
        let err = validate_manage_tracks(&json!({"set": [{"index": 0}]}))
            .into_error()
            .expect("flagless set refused");
        assert!(err.contains("at least one of muted"), "{err}");
    }

    #[test]
    fn mut_006_manage_tracks_negative_index_rejected() {
        assert!(validate_manage_tracks(&json!({"remove": [-1]}))
            .into_error()
            .is_some());
        assert!(
            validate_manage_tracks(&json!({"reorder": [{"index": -1, "to": 0}]}))
                .into_error()
                .is_some()
        );
        assert!(validate_manage_tracks(&json!({"reorder": [{"index": 0}]}))
            .into_error()
            .is_some());
    }

    // ---- MUT-007: move_clips --------------------------------------------

    #[test]
    fn mut_007_move_clips_valid_with_to_track() {
        let input = json!({"clipIds": ["c1"], "toTrack": 2});
        let result = validate_move_clips(&input);
        let parsed = result.into_ok().expect("MUT-007: with toTrack");
        assert_eq!(parsed.clip_ids, vec!["c1"]);
        assert_eq!(parsed.to_track, Some(2));
        assert_eq!(parsed.to_frame, None);
    }

    #[test]
    fn mut_007_move_clips_valid_with_to_frame() {
        let input = json!({"clipIds": ["c1"], "toFrame": 100});
        let result = validate_move_clips(&input);
        let parsed = result.into_ok().expect("MUT-007: with toFrame");
        assert_eq!(parsed.to_frame, Some(100));
        assert_eq!(parsed.to_track, None);
    }

    #[test]
    fn mut_007_move_clips_valid_with_both() {
        let input = json!({"clipIds": ["c1"], "toTrack": 2, "toFrame": 100});
        let result = validate_move_clips(&input);
        let parsed = result.into_ok().expect("MUT-007: with both");
        assert_eq!(parsed.to_track, Some(2));
        assert_eq!(parsed.to_frame, Some(100));
    }

    #[test]
    fn mut_007_move_clips_requires_at_least_one() {
        let input = json!({"clipIds": ["c1"]});
        let result = validate_move_clips(&input);
        let err = result
            .into_error()
            .expect("MUT-007: neither toTrack nor toFrame");
        assert!(err.contains("toTrack") || err.contains("toFrame"));
    }

    #[test]
    fn mut_007_move_clips_requires_clip_ids() {
        let input = json!({"toTrack": 0});
        let result = validate_move_clips(&input);
        assert!(result.into_error().is_some());
    }

    // ---- MUT-008: move_clips linked partner ------------------------------

    #[test]
    fn mut_008_move_clips_linked_valid() {
        let result = validate_move_clips_linked(&["c1".to_string(), "c2".to_string()]);
        let parsed = result.into_ok().expect("MUT-008: valid");
        assert_eq!(parsed.len(), 2);
    }

    #[test]
    fn mut_008_move_clips_linked_empty_rejected() {
        let result = validate_move_clips_linked(&[]);
        assert!(result.into_error().is_some());
    }

    // ---- MUT-010: text-only field validation ----------------------------

    #[test]
    fn mut_010_non_text_clip_rejects_text_fields() {
        let input = json!({
            "clipIds": ["c1"],
            "properties": {"content": "hello", "fontSize": 24, "opacity": 0.5}
        });
        let result = validate_set_clip_properties(&input, Some(vec!["video".to_string()]));
        let err = result
            .into_error()
            .expect("MUT-010: non-text clip rejects text fields");
        assert!(err.contains("content"));
    }

    #[test]
    fn mut_010_text_only_clip_allows_text_fields() {
        let input = json!({
            "clipIds": ["c1"],
            "properties": {"content": "hello", "fontSize": 24}
        });
        let result = validate_set_clip_properties(&input, Some(vec!["text".to_string()]));
        let parsed = result
            .into_ok()
            .expect("MUT-010: text clip allows text fields");
        assert_eq!(parsed.text_only_fields.len(), 2);
    }

    // ---- MUT-011: scalar volume/opacity clears keyframes -----------------

    #[test]
    fn mut_011_scalar_volume_clears_keyframes() {
        let input = json!({
            "clipIds": ["c1"],
            "properties": {"volume": 0.8}
        });
        let result = validate_set_clip_properties(&input, None);
        let parsed = result.into_ok().expect("MUT-011: scalar volume");
        assert!(parsed.clears_keyframes);
    }

    #[test]
    fn mut_011_scalar_opacity_clears_keyframes() {
        let input = json!({
            "clipIds": ["c1"],
            "properties": {"opacity": 0.5}
        });
        let result = validate_set_clip_properties(&input, None);
        let parsed = result.into_ok().expect("MUT-011: scalar opacity");
        assert!(parsed.clears_keyframes);
    }

    #[test]
    fn mut_011_keyframed_volume_no_clear() {
        let input = json!({
            "clipIds": ["c1"],
            "properties": {"volume": {"keyframes": [{"frame": 0, "value": 1.0}]}}
        });
        let result = validate_set_clip_properties(&input, None);
        let parsed = result.into_ok().expect("MUT-011: keyframed volume");
        assert!(
            !parsed.clears_keyframes,
            "object-typed volume does not clear"
        );
    }

    #[test]
    fn mut_011_no_scalar_no_clear() {
        let input = json!({
            "clipIds": ["c1"],
            "properties": {"speed": 2.0}
        });
        let result = validate_set_clip_properties(&input, None);
        let parsed = result.into_ok().expect("MUT-011: unrelated property");
        assert!(!parsed.clears_keyframes);
    }

    // ---- MUT-012: timing properties detection ---------------------------

    #[test]
    fn mut_012_detects_timing_properties() {
        let input = json!({
            "clipIds": ["c1"],
            "properties": {"speed": 2.0, "trimStartFrame": 10}
        });
        let result = validate_set_clip_properties(&input, None);
        let parsed = result.into_ok().expect("MUT-012: timing props");
        assert!(parsed.timing_properties.contains(&"speed".to_string()));
        assert!(parsed
            .timing_properties
            .contains(&"trimStartFrame".to_string()));
        assert_eq!(parsed.timing_properties.len(), 2);
    }

    #[test]
    fn mut_012_detects_all_timing_fields() {
        let input = json!({
            "clipIds": ["c1"],
            "properties": {
                "speed": 1.5,
                "durationFrames": 200,
                "trimStartFrame": 0,
                "trimEndFrame": 100
            }
        });
        let result = validate_set_clip_properties(&input, None);
        let parsed = result.into_ok().expect("MUT-012: all timing");
        assert_eq!(parsed.timing_properties.len(), 4);
    }

    #[test]
    fn mut_012_no_timing_properties() {
        let input = json!({
            "clipIds": ["c1"],
            "properties": {"opacity": 0.5, "volume": 1.0}
        });
        let result = validate_set_clip_properties(&input, None);
        let parsed = result.into_ok().expect("MUT-012: no timing");
        assert!(parsed.timing_properties.is_empty());
    }

    // ---- Issue #18: background / border validation ------------------

    #[test]
    fn issue_018_text_background_recognized_as_text_field() {
        let input = json!({
            "clipIds": ["clip-1"],
            "properties": {"background": {"enabled": true, "color": "#FF0000"}}
        });
        let result = validate_set_clip_properties(&input, Some(vec!["video".to_string()]));
        let err = result
            .into_error()
            .expect("background must be rejected for non-text clips");
        assert!(err.contains("background"), "err={err}");
    }

    #[test]
    fn issue_018_text_background_allowed_for_text_clips() {
        let input = json!({
            "clipIds": ["clip-1"],
            "properties": {"background": {"enabled": true, "color": "#FF0000"}}
        });
        let result = validate_set_clip_properties(&input, Some(vec!["text".to_string()]));
        result
            .into_ok()
            .expect("background must be accepted for text clips");
    }

    #[test]
    fn issue_018_text_border_recognized_as_text_field() {
        let input = json!({
            "clipIds": ["clip-1"],
            "properties": {"border": {"enabled": false, "color": "#000000"}}
        });
        let result = validate_set_clip_properties(&input, Some(vec!["video".to_string()]));
        let err = result.into_error().expect("border rejected for non-text");
        assert!(err.contains("border"), "err={err}");
    }

    #[test]
    fn issue_018_text_background_invalid_hex_rejected() {
        let input = json!({
            "clipIds": ["clip-1"],
            "properties": {"background": {"enabled": true, "color": "not-a-color"}}
        });
        let result = validate_set_clip_properties(&input, None);
        let err = result.into_error().expect("invalid hex must be rejected");
        assert!(err.contains("background.color"), "err={err}");
    }

    #[test]
    fn issue_018_text_border_invalid_hex_rejected() {
        let input = json!({
            "clipIds": ["clip-1"],
            "properties": {"border": {"enabled": true, "color": "#ZZZ"}}
        });
        let result = validate_set_clip_properties(&input, None);
        let err = result.into_error().expect("invalid hex must be rejected");
        assert!(err.contains("border.color"), "err={err}");
    }

    #[test]
    fn issue_018_text_background_non_object_rejected() {
        let input = json!({
            "clipIds": ["clip-1"],
            "properties": {"background": "red"}
        });
        let result = validate_set_clip_properties(&input, None);
        let err = result
            .into_error()
            .expect("non-object background must be rejected");
        assert!(err.contains("background"), "err={err}");
    }

    #[test]
    fn issue_018_font_weight_recognized_as_text_field() {
        let input = json!({
            "clipIds": ["clip-1"],
            "properties": {"fontWeight": 700}
        });
        let result = validate_set_clip_properties(&input, Some(vec!["video".to_string()]));
        let err = result
            .into_error()
            .expect("fontWeight rejected for non-text");
        assert!(err.contains("fontWeight"), "err={err}");
    }

    #[test]
    fn issue_018_text_background_color_valid_hex_accepted() {
        // Multiple hex formats should all be accepted
        for color in &["#FFF", "#FFFFFF", "#FFFFFF80"] {
            let input = json!({
                "clipIds": ["clip-1"],
                "properties": {"background": {"enabled": true, "color": color}}
            });
            let result = validate_set_clip_properties(&input, None);
            result
                .into_ok()
                .unwrap_or_else(|| panic!("color {} must be accepted", color));
        }
    }

    // ---- MUT-017/018: ripple_delete_ranges ------------------------------

    #[test]
    fn mut_017_ripple_delete_ranges_with_clip_id() {
        let input = json!({
            "clipId": "c1",
            "ranges": [
                {"startFrame": 0, "endFrame": 50},
                {"startFrame": 100, "endFrame": 150}
            ]
        });
        let result = validate_ripple_delete_ranges(&input);
        let parsed = result.into_ok().expect("MUT-017: clip-scoped");
        assert_eq!(parsed.clip_id, Some("c1".to_string()));
        assert_eq!(parsed.track_index, None);
        assert_eq!(parsed.ranges.len(), 2);
    }

    #[test]
    fn mut_017_ripple_delete_ranges_with_track_index() {
        let input = json!({
            "trackIndex": 2,
            "ranges": [{"startFrame": 0, "endFrame": 200}]
        });
        let result = validate_ripple_delete_ranges(&input);
        let parsed = result.into_ok().expect("MUT-017: track-scoped");
        assert_eq!(parsed.track_index, Some(2));
        assert_eq!(parsed.clip_id, None);
        assert_eq!(parsed.ranges.len(), 1);
    }

    #[test]
    fn mut_017_ripple_delete_ranges_rejects_both() {
        let input = json!({
            "clipId": "c1",
            "trackIndex": 0,
            "ranges": [{"startFrame": 0, "endFrame": 50}]
        });
        let result = validate_ripple_delete_ranges(&input);
        let err = result.into_error().expect("MUT-017: both rejected");
        assert!(err.contains("both"));
    }

    #[test]
    fn mut_017_ripple_delete_ranges_requires_one() {
        let input = json!({"ranges": [{"startFrame": 0, "endFrame": 50}]});
        let result = validate_ripple_delete_ranges(&input);
        let err = result.into_error().expect("MUT-017: neither rejected");
        assert!(err.contains("clipId") || err.contains("trackIndex"));
    }

    #[test]
    fn mut_018_ripple_delete_ranges_clip_scoped_seconds() {
        let input = json!({
            "clipId": "c1",
            "seconds": [
                {"startFrame": 10, "endFrame": 30},
                {"startFrame": 60, "endFrame": 90}
            ]
        });
        let result = validate_ripple_delete_ranges(&input);
        let parsed = result.into_ok().expect("MUT-018: seconds mode");
        assert_eq!(parsed.ranges.len(), 2);
    }

    #[test]
    fn mut_018_ripple_delete_ranges_clip_scoped_no_ranges() {
        // Clip-scoped with neither ranges nor seconds → empty ranges (full clip delete)
        let input = json!({"clipId": "c1"});
        let result = validate_ripple_delete_ranges(&input);
        let parsed = result.into_ok().expect("MUT-018: clip-scoped empty");
        assert!(parsed.ranges.is_empty());
    }

    // ---- MUT-019: add_texts ---------------------------------------------

    #[test]
    fn mut_019_add_texts_valid() {
        let input = json!({
            "texts": [
                {"text": "Hello", "startFrame": 0, "durationFrames": 100},
                {"text": "World", "startFrame": 100, "durationFrames": 50}
            ],
            "trackIndex": 1
        });
        let result = validate_add_texts(&input, None);
        let parsed = result.into_ok().expect("MUT-019: valid");
        assert_eq!(parsed.texts.len(), 2);
        assert_eq!(parsed.texts[0].text.as_deref(), Some("Hello"));
        assert_eq!(parsed.texts[0].start_frame, Some(0));
        assert_eq!(parsed.texts[0].duration_frames, Some(100));
        assert_eq!(parsed.track_index, Some(1));
    }

    #[test]
    fn mut_019_add_texts_executor_shape() {
        // Executor-authoritative shape: `content` preferred over `text`,
        // startFrame/durationFrames optional (defaulted at execution).
        let input = json!({"texts": [{"content": "C", "text": "T"}]});
        let parsed = validate_add_texts(&input, None)
            .into_ok()
            .expect("optional fields accepted");
        assert_eq!(parsed.texts[0].text.as_deref(), Some("C"));
        assert_eq!(parsed.texts[0].start_frame, None);
        assert_eq!(parsed.texts[0].duration_frames, None);
    }

    #[test]
    fn mut_019_add_texts_bad_entry_rejects_whole_call() {
        for (entry, needle) in [
            (json!({"content": "x", "startFrame": -5}), "startFrame"),
            (
                json!({"content": "x", "durationFrames": 0}),
                "durationFrames",
            ),
            (
                json!({"content": "x", "startFrame": MAX_TOOL_FRAME + 1}),
                "exceeds the maximum supported frame",
            ),
        ] {
            let input = json!({"texts": [json!({"content": "ok"}), entry]});
            let err = validate_add_texts(&input, None)
                .into_error()
                .expect("bad entry rejected");
            assert!(err.contains(needle), "want '{needle}' in: {err}");
        }
    }

    #[test]
    fn mut_019_add_texts_auto_create_visual_track() {
        let input = json!({
            "texts": [
                {"text": "Title", "startFrame": 0, "durationFrames": 200}
            ]
        });
        let result = validate_add_texts(&input, None);
        let parsed = result.into_ok().expect("MUT-019: auto-create");
        assert_eq!(parsed.track_index, None, "no trackIndex = auto-create");
        assert_eq!(parsed.texts.len(), 1);
    }

    #[test]
    fn mut_019_add_texts_missing_texts() {
        let input = json!({"trackIndex": 0});
        let result = validate_add_texts(&input, None);
        assert!(result.into_error().is_some());
    }

    // ---- MUT-020: add_texts rejects audio tracks ------------------------

    #[test]
    fn mut_020_add_texts_rejects_audio_track() {
        let input = json!({
            "texts": [
                {"text": "Subtitle", "startFrame": 0, "durationFrames": 100}
            ]
        });
        let result = validate_add_texts(&input, Some("audio".to_string()));
        let err = result.into_error().expect("MUT-020: audio rejected");
        assert!(err.contains("audio"));
    }

    #[test]
    fn mut_020_add_texts_allows_video_track() {
        let input = json!({
            "texts": [
                {"text": "Subtitle", "startFrame": 0, "durationFrames": 100}
            ]
        });
        let result = validate_add_texts(&input, Some("video".to_string()));
        assert!(result.into_ok().is_some(), "MUT-020: video allowed");
    }

    // ---- MUT-021: add_captions ------------------------------------------

    #[test]
    fn mut_021_add_captions_valid_with_clip_ids() {
        let input = json!({"clipIds": ["c1", "c2"]});
        let result = validate_add_captions(&input);
        let parsed = result.into_ok().expect("MUT-021: clipIds");
        assert_eq!(
            parsed.clip_ids,
            Some(vec!["c1".to_string(), "c2".to_string()])
        );
    }

    #[test]
    fn mut_021_add_captions_valid_auto_detect() {
        let input = json!({});
        let result = validate_add_captions(&input);
        let parsed = result.into_ok().expect("MUT-021: auto-detect");
        assert_eq!(parsed.clip_ids, None);
    }

    #[test]
    fn mut_021_add_captions_empty_ids_rejected() {
        let input = json!({"clipIds": []});
        let result = validate_add_captions(&input);
        assert!(result.into_error().is_some());
    }

    #[test]
    fn upstream_040_add_captions_with_language() {
        // PR #40: add_captions accepts optional language parameter
        let input = json!({"clipIds": ["c1"], "language": "fr-FR"});
        let result = validate_add_captions(&input);
        let parsed = result.into_ok().expect("language");
        assert_eq!(parsed.language, Some("fr-FR".to_string()));
        assert_eq!(parsed.clip_ids, Some(vec!["c1".to_string()]));
    }

    #[test]
    fn upstream_092_add_captions_with_words_per_caption() {
        // PR #92: add_captions accepts optional wordsPerCaption (clamped 1-12)
        let input = json!({"wordsPerCaption": 3});
        let result = validate_add_captions(&input);
        let parsed = result.into_ok().expect("wordsPerCaption");
        assert_eq!(parsed.words_per_caption, Some(3));
    }

    #[test]
    fn upstream_092_words_per_caption_clamped_to_range() {
        // PR #92: wordsPerCaption is clamped to 1-12
        let input = json!({"wordsPerCaption": 99});
        let result = validate_add_captions(&input);
        let parsed = result.into_ok().expect("wordsPerCaption clamped");
        assert_eq!(parsed.words_per_caption, Some(12));

        let input = json!({"wordsPerCaption": 0});
        let result = validate_add_captions(&input);
        let parsed = result.into_ok().expect("wordsPerCaption min");
        assert_eq!(parsed.words_per_caption, Some(1));
    }

    #[test]
    fn upstream_040_add_captions_language_optional() {
        // PR #40: language is optional, defaults to None
        let input = json!({"clipIds": ["c1"]});
        let result = validate_add_captions(&input);
        let parsed = result.into_ok().expect("no language");
        assert_eq!(parsed.language, None);
    }

    // ---- MUT-022 (v2): organize_media ------------------------------------

    #[test]
    fn mut_022_organize_media_valid_full_combo() {
        let input = json!({
            "createFolders": ["Hero shots/Takes"],
            "moves": [{"items": ["m1", "B-roll"], "into": "Archive"}, {"items": ["m2"]}],
            "renames": [{"item": "m1", "name": "Best take"}],
            "deletes": ["m3", "Old/Scraps"],
        });
        let parsed = validate_organize_media(&input)
            .into_ok()
            .expect("MUT-022: valid");
        assert_eq!(parsed.create_folders, vec!["Hero shots/Takes"]);
        assert_eq!(parsed.moves.len(), 2);
        assert_eq!(parsed.moves[0].into.as_deref(), Some("Archive"));
        assert_eq!(parsed.moves[1].into, None, "omitted into = project root");
        assert_eq!(parsed.renames[0].item, "m1");
        assert_eq!(parsed.renames[0].name, "Best take");
        assert_eq!(parsed.deletes, vec!["m3", "Old/Scraps"]);
    }

    #[test]
    fn mut_022_organize_media_empty_call_rejected() {
        let err = validate_organize_media(&json!({}))
            .into_error()
            .expect("empty call refused");
        assert!(err.contains("at least one of"), "{err}");
        assert!(validate_organize_media(
            &json!({"createFolders": [], "moves": [], "renames": [], "deletes": []})
        )
        .into_error()
        .is_some());
    }

    #[test]
    fn mut_022_organize_media_move_needs_items() {
        let err = validate_organize_media(&json!({"moves": [{"into": "X"}]}))
            .into_error()
            .expect("itemless move refused");
        assert!(err.contains("items"), "{err}");
    }

    #[test]
    fn mut_022_organize_media_rename_needs_item_and_name() {
        assert!(
            validate_organize_media(&json!({"renames": [{"item": "m1"}]}))
                .into_error()
                .is_some()
        );
        assert!(
            validate_organize_media(&json!({"renames": [{"name": "X"}]}))
                .into_error()
                .is_some()
        );
        assert!(
            validate_organize_media(&json!({"renames": [{"item": "m1", "name": "  "}]}))
                .into_error()
                .is_some(),
            "blank name refused"
        );
    }

    // ---- tool-surface-v2: close_project -----------------------------------

    #[test]
    fn close_project_all_absent_means_active() {
        let parsed = validate_close_project(&json!({}))
            .into_ok()
            .expect("no-arg close is valid");
        assert_eq!(
            parsed,
            CloseProjectInput {
                name: None,
                id: None,
                path: None
            }
        );
    }

    #[test]
    fn close_project_accepts_name_id_path_strings() {
        let parsed = validate_close_project(&json!({"name": "Demo"}))
            .into_ok()
            .unwrap();
        assert_eq!(parsed.name.as_deref(), Some("Demo"));
        assert!(validate_close_project(&json!({"id": 3}))
            .into_error()
            .is_some());
        assert!(validate_close_project(&json!({"path": ""}))
            .into_error()
            .is_some());
    }

    // ---- tool-surface-v2: import_media -------------------------------------

    #[test]
    fn import_media_requires_source_object() {
        let err = validate_import_media(&json!({"path": "/x.mp4"}))
            .into_error()
            .expect("bare path refused — source object required");
        assert!(err.contains("source"), "{err}");
    }

    #[test]
    fn import_media_exactly_one_source_kind() {
        assert!(validate_import_media(&json!({"source": {}}))
            .into_error()
            .is_some());
        assert!(validate_import_media(
            &json!({"source": {"path": "/x.mp4", "url": "https://x/y.mp4"}})
        )
        .into_error()
        .is_some());
        let parsed = validate_import_media(&json!({"source": {"path": "/x.mp4"}}))
            .into_ok()
            .expect("single path valid");
        assert_eq!(parsed.path.as_deref(), Some("/x.mp4"));
    }

    #[test]
    fn import_media_matte_requires_hex() {
        assert!(validate_import_media(&json!({"source": {"matte": {}}}))
            .into_error()
            .is_some());
        let parsed = validate_import_media(
            &json!({"source": {"matte": {"hex": "#000", "aspectRatio": "1:1"}}, "name": "Black"}),
        )
        .into_ok()
        .expect("matte valid");
        assert_eq!(parsed.matte_hex.as_deref(), Some("#000"));
        assert_eq!(parsed.matte_aspect.as_deref(), Some("1:1"));
        assert_eq!(parsed.name.as_deref(), Some("Black"));
    }

    #[test]
    fn import_media_bytes_requires_mime_type() {
        assert!(validate_import_media(&json!({"source": {"bytes": "aGk="}}))
            .into_error()
            .is_some());
        let parsed =
            validate_import_media(&json!({"source": {"bytes": "aGk=", "mimeType": "image/png"}}))
                .into_ok()
                .expect("bytes + mimeType valid");
        assert_eq!(parsed.mime_type.as_deref(), Some("image/png"));
    }

    // ---- Upstream #99: set_chroma_key ------------------------------------

    #[test]
    fn upstream_099_set_chroma_key_valid() {
        let input = json!({
            "clipId": "clip-001",
            "enabled": true,
            "color": "#00ff00",
            "threshold": 0.5,
            "smoothness": 0.1
        });
        let result = validate_set_chroma_key(&input);
        let parsed = result.into_ok().expect("set_chroma_key: valid");
        assert_eq!(parsed.clip_id, "clip-001");
        assert_eq!(parsed.enabled, Some(true));
        assert_eq!(parsed.color, Some("#00ff00".to_string()));
        assert!((parsed.threshold.unwrap() - 0.5).abs() < 1e-10);
        assert!((parsed.smoothness.unwrap() - 0.1).abs() < 1e-10);
    }

    #[test]
    fn upstream_099_set_chroma_key_minimal() {
        let input = json!({"clipId": "clip-001"});
        let result = validate_set_chroma_key(&input);
        let parsed = result.into_ok().expect("set_chroma_key: minimal");
        assert_eq!(parsed.clip_id, "clip-001");
        assert!(parsed.enabled.is_none());
        assert!(parsed.color.is_none());
        assert!(parsed.threshold.is_none());
        assert!(parsed.smoothness.is_none());
    }

    #[test]
    fn upstream_099_set_chroma_key_missing_clip_id() {
        let input = json!({"enabled": true});
        let result = validate_set_chroma_key(&input);
        let err = result.into_error().expect("set_chroma_key: missing clipId");
        assert!(err.contains("clipId"));
    }

    // ---- Upstream #99: set_blend_mode ------------------------------------

    #[test]
    fn upstream_099_set_blend_mode_valid() {
        let input = json!({"clipId": "c1", "mode": "multiply"});
        let result = validate_set_blend_mode(&input);
        let parsed = result.into_ok().expect("set_blend_mode: valid");
        assert_eq!(parsed.clip_id, "c1");
        assert_eq!(parsed.mode, "multiply");
    }

    #[test]
    fn upstream_099_set_blend_mode_missing_clip_id() {
        let input = json!({"mode": "screen"});
        let result = validate_set_blend_mode(&input);
        assert!(result.into_error().is_some());
    }

    #[test]
    fn upstream_099_set_blend_mode_missing_mode() {
        let input = json!({"clipId": "c1"});
        let result = validate_set_blend_mode(&input);
        let err = result.into_error().expect("set_blend_mode: missing mode");
        assert!(err.contains("mode"));
    }

    // ---- Upstream #99: set_color_grade -----------------------------------

    #[test]
    fn upstream_099_set_color_grade_valid() {
        let input = json!({
            "clipId": "c1",
            "exposure": 1.5,
            "contrast": 1.2,
            "saturation": 1.0,
            "temperature": 0.3
        });
        let result = validate_set_color_grade(&input);
        let parsed = result.into_ok().expect("set_color_grade: valid");
        assert_eq!(parsed.clip_id, "c1");
        assert!((parsed.exposure.unwrap() - 1.5).abs() < 1e-10);
        assert!((parsed.contrast.unwrap() - 1.2).abs() < 1e-10);
        assert!((parsed.saturation.unwrap() - 1.0).abs() < 1e-10);
        assert!((parsed.temperature.unwrap() - 0.3).abs() < 1e-10);
    }

    #[test]
    fn upstream_099_set_color_grade_minimal() {
        let input = json!({"clipId": "c1"});
        let result = validate_set_color_grade(&input);
        let parsed = result.into_ok().expect("set_color_grade: minimal");
        assert_eq!(parsed.clip_id, "c1");
        assert!(parsed.exposure.is_none());
        assert!(parsed.contrast.is_none());
        assert!(parsed.saturation.is_none());
        assert!(parsed.temperature.is_none());
    }

    #[test]
    fn upstream_099_set_color_grade_missing_clip_id() {
        let input = json!({"exposure": 0.0});
        let result = validate_set_color_grade(&input);
        let err = result
            .into_error()
            .expect("set_color_grade: missing clipId");
        assert!(err.contains("clipId"));
    }

    // ---- Upstream #6: generate_music -------------------------------------

    #[test]
    fn upstream_006_generate_music_valid() {
        let input = json!({
            "prompt": "upbeat electronic",
            "duration": 30.0,
            "style": "electronic"
        });
        let result = validate_generate_music(&input);
        let parsed = result.into_ok().expect("generate_music: valid");
        assert_eq!(parsed.prompt, "upbeat electronic");
        assert!((parsed.duration.unwrap() - 30.0).abs() < 1e-10);
        assert_eq!(parsed.style, Some("electronic".to_string()));
    }

    #[test]
    fn upstream_006_generate_music_minimal() {
        let input = json!({"prompt": "ambient pad"});
        let result = validate_generate_music(&input);
        let parsed = result.into_ok().expect("generate_music: minimal");
        assert_eq!(parsed.prompt, "ambient pad");
        assert!(parsed.duration.is_none());
        assert!(parsed.style.is_none());
    }

    #[test]
    fn upstream_006_generate_music_missing_prompt() {
        let input = json!({"duration": 30.0});
        let result = validate_generate_music(&input);
        let err = result.into_error().expect("generate_music: missing prompt");
        assert!(err.contains("prompt"));
    }

    // ---- Upstream #67: duplicate_project ---------------------------------

    #[test]
    fn upstream_067_duplicate_project_valid() {
        let input = json!({});
        let result = validate_duplicate_project(&input);
        assert!(
            result.into_ok().is_some(),
            "duplicate_project: always valid"
        );
    }

    #[test]
    fn upstream_067_duplicate_project_ignores_extra_fields() {
        let input = json!({"unknown": "value"});
        let result = validate_duplicate_project(&input);
        assert!(
            result.into_ok().is_some(),
            "duplicate_project: ignores extras"
        );
    }

    // ---- Upstream #46: add_shapes -----------------------------------------

    #[test]
    fn upstream_046_add_shapes_valid() {
        let input = json!({
            "entries": [
                {"kind": "rect", "startFrame": 0, "durationFrames": 100}
            ]
        });
        let result = validate_add_shapes(&input);
        let parsed = result.into_ok().expect("add_shapes: valid");
        assert_eq!(parsed.entries.len(), 1);
        assert_eq!(parsed.entries[0]["kind"], "rect");
    }

    #[test]
    fn upstream_046_add_shapes_empty_entries() {
        let input = json!({"entries": []});
        let result = validate_add_shapes(&input);
        let err = result.into_error().expect("add_shapes: empty entries");
        assert!(err.contains("entries"));
    }

    #[test]
    fn upstream_046_add_shapes_missing_entries() {
        let input = json!({});
        let result = validate_add_shapes(&input);
        let err = result.into_error().expect("add_shapes: missing entries");
        assert!(err.contains("entries"));
    }

    #[test]
    fn upstream_046_add_shapes_multiple_entries() {
        let input = json!({
            "entries": [
                {"kind": "rect", "startFrame": 0, "durationFrames": 50},
                {"kind": "arrow", "startFrame": 10, "durationFrames": 30}
            ]
        });
        let result = validate_add_shapes(&input);
        let parsed = result.into_ok().expect("add_shapes: multiple");
        assert_eq!(parsed.entries.len(), 2);
        assert_eq!(parsed.entries[1]["kind"], "arrow");
    }

    // ---- Upstream #46: apply_animation ------------------------------------

    #[test]
    fn upstream_046_apply_animation_valid() {
        let input = json!({
            "clipId": "c1",
            "preset": "fade-in"
        });
        let result = validate_apply_animation(&input);
        let parsed = result.into_ok().expect("apply_animation: valid");
        assert_eq!(parsed.clip_id, "c1");
        assert_eq!(parsed.preset, "fade-in");
    }

    #[test]
    fn upstream_046_apply_animation_with_options() {
        let input = json!({
            "clipId": "c1",
            "preset": "slide-in-left",
            "windowFrames": "10-60",
            "intensity": "strong"
        });
        let result = validate_apply_animation(&input);
        let parsed = result.into_ok().expect("apply_animation: with options");
        assert_eq!(parsed.window_frames, Some("10-60".to_string()));
        assert_eq!(parsed.intensity, Some("strong".to_string()));
    }

    #[test]
    fn upstream_046_apply_animation_missing_clip_id() {
        let input = json!({"preset": "fade-in"});
        let result = validate_apply_animation(&input);
        let err = result
            .into_error()
            .expect("apply_animation: missing clipId");
        assert!(err.contains("clipId"));
    }

    #[test]
    fn upstream_046_apply_animation_missing_preset() {
        let input = json!({"clipId": "c1"});
        let result = validate_apply_animation(&input);
        let err = result
            .into_error()
            .expect("apply_animation: missing preset");
        assert!(err.contains("preset"));
    }

    // ---- Upstream #8: apply_color -----------------------------------------

    #[test]
    fn upstream_008_apply_color_valid() {
        let input = json!({"clipId": "c1", "exposure": 0.5, "contrast": 1.2});
        let result = validate_apply_color(&input);
        let parsed = result.into_ok().expect("apply_color: valid");
        assert_eq!(parsed.clip_id, "c1");
        assert!((parsed.exposure.unwrap() - 0.5).abs() < 1e-10);
        assert!((parsed.contrast.unwrap() - 1.2).abs() < 1e-10);
        assert!(parsed.saturation.is_none());
    }

    #[test]
    fn upstream_008_apply_color_missing_clip_id() {
        let input = json!({"exposure": 0.5});
        let result = validate_apply_color(&input);
        assert!(result.into_error().unwrap().contains("clipId"));
    }

    #[test]
    fn upstream_008_apply_color_with_reset() {
        let input = json!({"clipId": "c1", "reset": true});
        let result = validate_apply_color(&input);
        let parsed = result.into_ok().expect("apply_color: reset");
        assert!(parsed.reset == Some(true));
    }

    // ---- Upstream #8: apply_effect ----------------------------------------

    #[test]
    fn upstream_008_apply_effect_valid() {
        let input = json!({
            "clipId": "c1",
            "type": "blur.gaussian",
            "intensity": 0.5
        });
        let result = validate_apply_effect(&input);
        let parsed = result.into_ok().expect("apply_effect: valid");
        assert_eq!(parsed.clip_id, "c1");
        assert_eq!(parsed.effect_type.unwrap(), "blur.gaussian");
        assert!((parsed.intensity.unwrap() - 0.5).abs() < 1e-10);
    }

    #[test]
    fn upstream_008_apply_effect_with_remove() {
        let input = json!({
            "clipId": "c1",
            "remove": ["blur.gaussian", "stylize.glow"]
        });
        let result = validate_apply_effect(&input);
        let parsed = result.into_ok().expect("apply_effect: remove");
        assert_eq!(parsed.remove.unwrap().len(), 2);
    }

    #[test]
    fn upstream_008_apply_effect_missing_clip_id() {
        let input = json!({"type": "blur.gaussian"});
        let result = validate_apply_effect(&input);
        assert!(result.into_error().unwrap().contains("clipId"));
    }

    // ---- Upstream #8: inspect_color ---------------------------------------

    #[test]
    fn upstream_008_inspect_color_with_clip_id() {
        let input = json!({"clipId": "c1"});
        let result = validate_inspect_color(&input);
        let parsed = result.into_ok().expect("inspect_color: clipId");
        assert_eq!(parsed.clip_id.unwrap(), "c1");
    }

    #[test]
    fn upstream_008_inspect_color_with_media_ref() {
        let input = json!({"mediaRef": "asset-vid-1"});
        let result = validate_inspect_color(&input);
        let parsed = result.into_ok().expect("inspect_color: mediaRef");
        assert_eq!(parsed.media_ref.unwrap(), "asset-vid-1");
    }

    #[test]
    fn upstream_008_inspect_color_with_reference() {
        let input = json!({"clipId": "c1", "reference": "asset-ref"});
        let result = validate_inspect_color(&input);
        let parsed = result.into_ok().expect("inspect_color: reference");
        assert_eq!(parsed.reference.unwrap(), "asset-ref");
    }
}

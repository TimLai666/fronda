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

/// Parsed and validated `split_clip` input.
#[derive(Debug, Clone, PartialEq)]
pub struct SplitClipInput {
    pub clip_id: String,
    pub frame: i64,
}

/// MUT-016: Validate `split_clip` input.
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

    ValidationResult::Ok(SplitClipInput { clip_id, frame })
}

/// Parsed and validated `set_clip_properties` input.
#[derive(Debug, Clone, PartialEq)]
pub struct SetClipPropertiesInput {
    pub clip_ids: Vec<String>,
    pub properties: Value,
}

/// MUT-009: `set_clip_properties` applies the same property set to every clip.
/// MUT-010: text-only fields rejected when any target is non-text.
/// MUT-011: Setting scalar volume/opacity clears existing keyframes.
pub fn validate_set_clip_properties(input: &Value) -> ValidationResult<SetClipPropertiesInput> {
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

    let properties = match input.get("properties") {
        Some(v) if v.is_object() => v.clone(),
        Some(_) => {
            return ValidationResult::Error(
                "set_clip_properties: 'properties' must be a JSON object".into(),
            )
        }
        None => {
            return ValidationResult::Error(
                "set_clip_properties: missing 'properties' object".into(),
            )
        }
    };

    ValidationResult::Ok(SetClipPropertiesInput {
        clip_ids,
        properties,
    })
}

/// Parsed and validated `set_keyframes` input.
#[derive(Debug, Clone, PartialEq)]
pub struct SetKeyframesInput {
    pub clip_id: String,
    pub property: String,
    pub keyframes: Vec<(i64, f64)>,
}

/// MUT-013: replaces the full keyframe track for one (clipId, property) pair.
/// MUT-014: empty arrays clear the track.
/// MUT-015: keyframe rows are sorted; duplicate frames are last-write-wins.
pub fn validate_set_keyframes(input: &Value) -> ValidationResult<SetKeyframesInput> {
    let clip_id = match input.get("clipId").and_then(|v| v.as_str()) {
        Some(id) if !id.is_empty() => id.to_string(),
        _ => return ValidationResult::Error("set_keyframes: missing or empty 'clipId'".into()),
    };

    let property = match input.get("property").and_then(|v| v.as_str()) {
        Some(p) if !p.is_empty() => p.to_string(),
        _ => return ValidationResult::Error("set_keyframes: missing or empty 'property'".into()),
    };

    let keyframes = match input.get("keyframes").and_then(|v| v.as_array()) {
        Some(arr) => {
            let mut pairs: Vec<(i64, f64)> = arr
                .iter()
                .filter_map(|kf| {
                    let frame = kf.get("frame").and_then(|v| v.as_i64())?;
                    let value = kf.get("value").and_then(|v| v.as_f64())?;
                    Some((frame, value))
                })
                .collect();

            // MUT-015: sort by frame, last-write-wins for duplicates.
            // dedup_by_key keeps the *first* of consecutive duplicates,
            // so reverse after sorting so the *last* value survives.
            pairs.sort_by_key(|&(frame, _)| frame);
            pairs.reverse();
            pairs.dedup_by_key(|&mut (frame, _)| frame);
            pairs.reverse();
            pairs
        }
        None => return ValidationResult::Error("set_keyframes: missing 'keyframes' array".into()),
    };

    ValidationResult::Ok(SetKeyframesInput {
        clip_id,
        property,
        keyframes,
    })
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

/// Parsed and validated `add_clips` input.
#[derive(Debug, Clone, PartialEq)]
pub struct AddClipsInput {
    pub media_ids: Vec<String>,
    pub track_index: Option<usize>,
}

/// MUT-002: mixed explicit/omitted trackIndex rejected.
/// MUT-003: auto-create tracks when all entries omit trackIndex.
pub fn validate_add_clips(input: &Value) -> ValidationResult<AddClipsInput> {
    let media_ids = match input.get("mediaIds").and_then(|v| v.as_array()) {
        Some(arr) if !arr.is_empty() => arr
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect(),
        _ => return ValidationResult::Error("add_clips: missing or empty 'mediaIds'".into()),
    };

    let track_index = input
        .get("trackIndex")
        .and_then(|v| v.as_u64())
        .map(|i| i as usize);

    ValidationResult::Ok(AddClipsInput {
        media_ids,
        track_index,
    })
}

/// Validate hex color strings (MUT-023).
///
/// Accepts #RGB, #RRGGBB, #RRGGBBAA.
/// Trims surrounding whitespace/newlines.
/// Rejects embedded/internal whitespace.
pub fn validate_hex_color(input: &str) -> ValidationResult<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return ValidationResult::Error("empty hex color".into());
    }

    // Check for internal whitespace
    let without_prefix = if let Some(rest) = trimmed.strip_prefix('#') {
        rest
    } else {
        return ValidationResult::Error("hex color must start with '#'".into());
    };

    if without_prefix.contains(|c: char| c.is_whitespace()) {
        return ValidationResult::Error("hex color contains internal whitespace".into());
    }

    match without_prefix.len() {
        3 | 6 | 8 => {
            if without_prefix.chars().all(|c| c.is_ascii_hexdigit()) {
                ValidationResult::Ok(trimmed.to_string())
            } else {
                ValidationResult::Error("hex color contains invalid characters".into())
            }
        }
        _ => ValidationResult::Error("hex color must be #RGB, #RRGGBB, or #RRGGBBAA".into()),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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

    #[test]
    fn mut_009_set_clip_properties_valid() {
        let input = json!({
            "clipIds": ["c1", "c2"],
            "properties": {"speed": 2.0}
        });
        let result = validate_set_clip_properties(&input);
        let parsed = result.into_ok().expect("MUT-009: valid");
        assert_eq!(parsed.clip_ids.len(), 2);
    }

    #[test]
    fn mut_009_set_clip_properties_empty_ids() {
        let input = json!({
            "clipIds": [],
            "properties": {}
        });
        let result = validate_set_clip_properties(&input);
        assert!(result.into_error().is_some(), "MUT-009: empty ids rejected");
    }

    #[test]
    fn mut_009_set_clip_properties_missing_properties() {
        let input = json!({"clipIds": ["c1"]});
        let result = validate_set_clip_properties(&input);
        assert!(result.into_error().is_some());
    }

    #[test]
    fn mut_013_set_keyframes_valid() {
        let input = json!({
            "clipId": "c1",
            "property": "opacity",
            "keyframes": [
                {"frame": 0, "value": 1.0},
                {"frame": 50, "value": 0.5}
            ]
        });
        let result = validate_set_keyframes(&input);
        let parsed = result.into_ok().expect("MUT-013: valid");
        assert_eq!(parsed.keyframes.len(), 2);
        assert_eq!(parsed.keyframes[0], (0, 1.0));
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
            "keyframes": [
                {"frame": 50, "value": 0.5},
                {"frame": 0, "value": 1.0},
                {"frame": 50, "value": 0.8}
            ]
        });
        let result = validate_set_keyframes(&input);
        let parsed = result.into_ok().expect("MUT-015: sorted & deduped");
        assert_eq!(parsed.keyframes.len(), 2, "MUT-015: two unique frames");
        // Last-write-wins at frame 50: value should be 0.8
        assert_eq!(parsed.keyframes[1], (50, 0.8));
    }

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

    #[test]
    fn mut_002_add_clips_valid_with_track() {
        let input = json!({"mediaIds": ["m1", "m2"], "trackIndex": 0});
        let result = validate_add_clips(&input);
        let parsed = result.into_ok().expect("MUT-002: valid with track");
        assert_eq!(parsed.track_index, Some(0));
    }

    #[test]
    fn mut_002_add_clips_valid_without_track() {
        let input = json!({"mediaIds": ["m1"]});
        let result = validate_add_clips(&input);
        let parsed = result.into_ok().expect("MUT-002: valid without track");
        assert_eq!(parsed.track_index, None);
    }

    #[test]
    fn mut_002_add_clips_rejects_empty() {
        let input = json!({"mediaIds": []});
        let result = validate_add_clips(&input);
        assert!(result.into_error().is_some());
    }

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
}

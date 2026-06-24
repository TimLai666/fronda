//! All 42 agent tool definitions with JSON input schemas (TDEF-001 to TDEF-003).

use serde::Serialize;
use serde_json::Value;

/// A single tool definition exposed to the agent/MCP surface.
///
/// TDEF-002: names remain snake_case.
/// TDEF-003: JSON schemas are part of the public contract.
#[derive(Debug, Clone, Serialize)]
pub struct ToolDefinition {
    pub name: &'static str,
    pub description: &'static str,
    pub input_schema: Value,
}

/// Returns all 42 tools exposed to the agent.
///
/// TDEF-001: exactly these 42 tools.
pub fn all_tools() -> Vec<ToolDefinition> {
    vec![
        add_captions(),
        add_clips(),
        add_shapes(),
        add_texts(),
        apply_animation(),
        apply_color(),
        apply_effect(),
        create_folder(),
        delete_folder(),
        delete_media(),
        duplicate_project(),
        generate_audio(),
        generate_image(),
        generate_music(),
        generate_video(),
        get_media(),
        get_timeline(),
        get_transcript(),
        import_folder(),
        import_media(),
        insert_clips(),
        inspect_color(),
        inspect_media(),
        inspect_timeline(),
        list_folders(),
        list_models(),
        move_clips(),
        move_to_folder(),
        remove_clips(),
        remove_tracks(),
        rename_folder(),
        rename_media(),
        ripple_delete_ranges(),
        search_media(),
        set_blend_mode(),
        set_chroma_key(),
        set_clip_properties(),
        set_color_grade(),
        set_keyframes(),
        split_clip(),
        undo(),
        upscale_media(),
    ]
}

/// TDEF-004: system instruction text for the agent.
pub const SYSTEM_INSTRUCTION: &str = r#"You are an AI assistant integrated into Fronda, a professional video editing application.

When helping the user edit their project:
- Call get_timeline once per session to understand the timeline state.
- Call get_media before referencing any media assets.
- Call list_models before any generation or upscale operation.
- Use inspect_media before describing any asset to the user.
- Generation operations require explicit user confirmation before execution.
- Keep replies terse and outcome-first.
- Always verify clip and track IDs exist before referencing them."#;

// ---------------------------------------------------------------------------
// Tool factory functions
// ---------------------------------------------------------------------------

fn add_captions() -> ToolDefinition {
    ToolDefinition {
        name: "add_captions",
        description: "Generate captions for clips in the timeline.",
        input_schema: object(&[
            ("trackId", string("Target track id")),
            ("clipIds", string("Optional specific clip ids to caption (comma-separated)")),
            ("language", string("Optional BCP-47 spoken language. Overrides project transcriptionLanguage for this call; falls back to system language if neither is set.")),
            ("wordsPerCaption", string("Optional max words per caption group (1-12, default 6). Upstream PR #92.")),
        ]),
    }
}

fn add_clips() -> ToolDefinition {
    ToolDefinition {
        name: "add_clips",
        description: "Add media clips to the end of the timeline.",
        input_schema: object(&[
            ("mediaIds", array("Media asset ids to add")),
            (
                "trackIndex",
                integer("Target track index (0-based, default: first visual/audio track)"),
            ),
        ]),
    }
}

fn add_texts() -> ToolDefinition {
    ToolDefinition {
        name: "add_texts",
        description: "Add text overlay clips to the timeline.",
        input_schema: object(&[
            ("text", string("Text content")),
            ("startFrame", integer("Start frame (inclusive)")),
            ("durationFrames", integer("Duration in frames")),
        ]),
    }
}

fn create_folder() -> ToolDefinition {
    ToolDefinition {
        name: "create_folder",
        description: "Create a new media folder.",
        input_schema: object(&[("name", string("Folder name"))]),
    }
}

fn delete_folder() -> ToolDefinition {
    ToolDefinition {
        name: "delete_folder",
        description: "Delete a media folder and its contents.",
        input_schema: object(&[("folderId", string("Folder id to delete"))]),
    }
}

fn delete_media() -> ToolDefinition {
    ToolDefinition {
        name: "delete_media",
        description: "Delete imported media from the project.",
        input_schema: object(&[("mediaId", string("Media id to delete"))]),
    }
}

fn generate_audio() -> ToolDefinition {
    ToolDefinition {
        name: "generate_audio",
        description: "Generate audio using the configured model.",
        input_schema: object(&[
            ("prompt", string("Description of the audio to generate")),
            ("duration", number("Duration in seconds")),
        ]),
    }
}

fn generate_image() -> ToolDefinition {
    ToolDefinition {
        name: "generate_image",
        description: "Generate an image using the configured model.",
        input_schema: object(&[("prompt", string("Description of the image to generate"))]),
    }
}

fn generate_video() -> ToolDefinition {
    ToolDefinition {
        name: "generate_video",
        description: "Generate a video clip using the configured model.",
        input_schema: object(&[
            ("prompt", string("Description of the video to generate")),
            ("duration", number("Duration in seconds")),
        ]),
    }
}

fn get_media() -> ToolDefinition {
    ToolDefinition {
        name: "get_media",
        description:
            "Return the media manifest as JSON. Pass optional folderId to scope to a folder.",
        input_schema: object(&[("folderId", string("Optional folder id to scope results"))]),
    }
}

fn get_timeline() -> ToolDefinition {
    ToolDefinition {
        name: "get_timeline",
        description: "Return project settings (fps, resolution, totalFrames, transcriptionLanguage) and timeline tracks as JSON.",
        input_schema: object(&[]),
    }
}

fn get_transcript() -> ToolDefinition {
    ToolDefinition {
        name: "get_transcript",
        description: "Return the transcript for a media asset. Transcription runs on-device and defaults to the system language — pass language when the audio is in another language.",
        input_schema: object(&[
            ("mediaId", string("Media asset id")),
            ("startFrame", string("Optional start frame for range-limited transcript")),
            ("endFrame", string("Optional end frame for range-limited transcript")),
            ("language", string("Optional BCP-47 spoken language (e.g. 'fr', 'ja', 'en-GB'). Overrides project transcriptionLanguage for this call; falls back to system language if neither is set.")),
        ]),
    }
}

fn import_media() -> ToolDefinition {
    ToolDefinition {
        name: "import_media",
        description: "Import a media file into the project. Supported extensions: .mov .mp4 .m4v (video), .mp3 .wav .aac .m4a .aiff .aif .aifc .flac (audio), .png .jpg .jpeg .tiff .heic .webp (image), .json .lottie (animation).",
        input_schema: object(&[("path", string("File path to import"))]),
    }
}

fn insert_clips() -> ToolDefinition {
    ToolDefinition {
        name: "insert_clips",
        description: "Insert clips at a specific frame position, pushing existing content later.",
        input_schema: object(&[
            ("mediaIds", array("Media asset ids to insert")),
            ("frame", integer("Insertion frame position")),
        ]),
    }
}

fn inspect_media() -> ToolDefinition {
    ToolDefinition {
        name: "inspect_media",
        description: "Inspect a media asset and return details. Transcription defaults to system language — pass language when the audio is in another language.",
        input_schema: object(&[
            ("mediaId", string("Media asset id to inspect")),
            ("language", string("Optional BCP-47 spoken language (e.g. 'fr', 'ja', 'en-GB'). Overrides project transcriptionLanguage for this call; falls back to system language if neither is set.")),
        ]),
    }
}

fn inspect_timeline() -> ToolDefinition {
    ToolDefinition {
        name: "inspect_timeline",
        description: "Return detailed timeline information.",
        input_schema: object(&[("range", string("Optional frame range (format: start-end)"))]),
    }
}

fn list_folders() -> ToolDefinition {
    ToolDefinition {
        name: "list_folders",
        description: "List all media folders.",
        input_schema: object(&[]),
    }
}

fn list_models() -> ToolDefinition {
    ToolDefinition {
        name: "list_models",
        description: "List available generation models.",
        input_schema: object(&[]),
    }
}

fn move_clips() -> ToolDefinition {
    ToolDefinition {
        name: "move_clips",
        description: "Move clips to a new position or track.",
        input_schema: object(&[
            ("clipIds", array("Clip ids to move")),
            ("frame", integer("Destination start frame")),
            ("trackIndex", integer("Optional destination track index")),
        ]),
    }
}

fn move_to_folder() -> ToolDefinition {
    ToolDefinition {
        name: "move_to_folder",
        description: "Move a media asset to a folder.",
        input_schema: object(&[
            ("mediaId", string("Media asset id to move")),
            ("folderId", string("Destination folder id")),
        ]),
    }
}

fn remove_clips() -> ToolDefinition {
    ToolDefinition {
        name: "remove_clips",
        description: "Remove clips from the timeline.",
        input_schema: object(&[
            ("clipIds", array("Clip ids to remove")),
            ("ripple", boolean("If true, ripple-close the gap")),
        ]),
    }
}

fn remove_tracks() -> ToolDefinition {
    ToolDefinition {
        name: "remove_tracks",
        description: "Remove tracks from the timeline.",
        input_schema: object(&[("trackIds", array("Track ids to remove"))]),
    }
}

fn rename_folder() -> ToolDefinition {
    ToolDefinition {
        name: "rename_folder",
        description: "Rename a media folder.",
        input_schema: object(&[
            ("folderId", string("Folder id to rename")),
            ("name", string("New folder name")),
        ]),
    }
}

fn rename_media() -> ToolDefinition {
    ToolDefinition {
        name: "rename_media",
        description: "Rename a media asset.",
        input_schema: object(&[
            ("mediaId", string("Media asset id to rename")),
            ("name", string("New display name")),
        ]),
    }
}

fn ripple_delete_ranges() -> ToolDefinition {
    ToolDefinition {
        name: "ripple_delete_ranges",
        description: "Delete frame ranges from the timeline with ripple.",
        input_schema: object(&[
            (
                "ranges",
                array("Array of {startFrame, endFrame} ranges to delete"),
            ),
            ("trackIndex", integer("Optional: scope to specific track")),
        ]),
    }
}

fn search_media() -> ToolDefinition {
    ToolDefinition {
        name: "search_media",
        description: "Search media assets by keyword.",
        input_schema: object(&[("query", string("Search query"))]),
    }
}

fn set_clip_properties() -> ToolDefinition {
    ToolDefinition {
        name: "set_clip_properties",
        description: "Set properties on one or more clips.",
        input_schema: object(&[
            ("clipIds", array("Clip ids to modify")),
            (
                "properties",
                object_any("Properties to set (transform, crop, speed, etc.)"),
            ),
        ]),
    }
}

fn set_keyframes() -> ToolDefinition {
    ToolDefinition {
        name: "set_keyframes",
        description: "Set keyframe values for clip properties.",
        input_schema: object(&[
            ("clipId", string("Clip id")),
            ("property", string("Property name to keyframe")),
            ("keyframes", array("Array of {frame, value} keyframes")),
        ]),
    }
}

fn split_clip() -> ToolDefinition {
    ToolDefinition {
        name: "split_clip",
        description: "Split a clip at the given frame.",
        input_schema: object(&[
            ("clipId", string("Clip id to split")),
            ("frame", integer("Frame position to split at")),
        ]),
    }
}

fn undo() -> ToolDefinition {
    ToolDefinition {
        name: "undo",
        description: "Undo the last timeline edit.",
        input_schema: object(&[]),
    }
}

fn upscale_media() -> ToolDefinition {
    ToolDefinition {
        name: "upscale_media",
        description: "Upscale a media asset.",
        input_schema: object(&[("mediaId", string("Media asset id to upscale"))]),
    }
}

fn import_folder() -> ToolDefinition {
    ToolDefinition {
        name: "import_folder",
        description: "Recursively import all supported media files from a directory.",
        input_schema: object(&[
            ("path", string("Directory path to import from")),
            (
                "recursive",
                boolean("If true, recursively scan subdirectories"),
            ),
        ]),
    }
}

fn set_chroma_key() -> ToolDefinition {
    ToolDefinition {
        name: "set_chroma_key",
        description: "Set chroma key (green screen) parameters on a clip.",
        input_schema: object(&[
            ("clipId", string("Clip id to apply chroma key to")),
            ("enabled", boolean("Enable or disable chroma key")),
            ("color", string("Key color as hex (#RRGGBB)")),
            ("threshold", number("Similarity threshold 0-1")),
            ("smoothness", number("Edge smoothness 0-1")),
        ]),
    }
}

fn set_blend_mode() -> ToolDefinition {
    ToolDefinition {
        name: "set_blend_mode",
        description: "Set the blend mode for a clip's compositing.",
        input_schema: object(&[
            ("clipId", string("Clip id")),
            (
                "mode",
                string("Blend mode: normal, multiply, screen, overlay, etc."),
            ),
        ]),
    }
}

fn set_color_grade() -> ToolDefinition {
    ToolDefinition {
        name: "set_color_grade",
        description: "Set color grade parameters on a clip.",
        input_schema: object(&[
            ("clipId", string("Clip id")),
            ("exposure", number("Exposure adjustment (-4 to 4)")),
            ("contrast", number("Contrast adjustment (0 to 4)")),
            ("saturation", number("Saturation (0 to 4)")),
            ("temperature", number("Temperature adjustment (-1 to 1)")),
        ]),
    }
}

fn generate_music() -> ToolDefinition {
    ToolDefinition {
        name: "generate_music",
        description: "Generate music using the configured model.",
        input_schema: object(&[
            ("prompt", string("Description of the music to generate")),
            ("duration", number("Duration in seconds")),
            (
                "style",
                string("Optional music style (e.g., cinematic, ambient, upbeat)"),
            ),
        ]),
    }
}

fn duplicate_project() -> ToolDefinition {
    ToolDefinition {
        name: "duplicate_project",
        description: "Duplicate the current project package.",
        input_schema: object(&[]),
    }
}

fn add_shapes() -> ToolDefinition {
    ToolDefinition {
        name: "add_shapes",
        description: "Add vector shape overlays (rect, oval, circle, arrow, line) to the timeline. PR #46.",
        input_schema: object(&[("entries", string("Array of shape entries. Each entry has: kind (rect|oval|circle|arrow|line), startFrame, durationFrames, optional trackIndex, optional transform (2D bounding box), optional style (color, width, dashed, arrowheadStyle), optional fill (enabled, color), optional endpoints (start/end Point2d with optional bezier controls), optional enterAnim/exitAnim/loopAnim presets."))]),
    }
}

fn apply_animation() -> ToolDefinition {
    ToolDefinition {
        name: "apply_animation",
        description: "Apply an animation preset to an existing clip. PR #46.",
        input_schema: object(&[
            ("clipId", string("Clip id to animate")),
            ("preset", string("Animation preset name: fade-in, pop-in, draw-on, slide-in-up, slide-in-down, slide-in-left, slide-in-right, fade-out, pop-out, un-draw, slide-out-up, slide-out-down, slide-out-left, slide-out-right, shake-subtle, shake-strong, spin")),
            ("windowFrames", string("Optional frame range for the animation (format: start-end)")),
            ("intensity", string("Optional animation intensity: subtle, normal, strong")),
        ]),
    }
}

fn apply_color() -> ToolDefinition {
    ToolDefinition {
        name: "apply_color",
        description: "Apply color grading parameters to a clip. MERGE semantics — only passed params change. PR #8.",
        input_schema: object(&[
            ("clipId", string("Clip id to grade")),
            ("exposure", number("Exposure adjustment (-4 to 4)")),
            ("contrast", number("Contrast adjustment (0 to 4)")),
            ("saturation", number("Saturation (0 to 4)")),
            ("vibrance", number("Vibrance adjustment (0 to 2)")),
            ("temperature", number("Temperature adjustment (-1 to 1)")),
            ("tint", number("Tint adjustment (-1 to 1)")),
            ("highlights", number("Highlight adjustment (-1 to 1)")),
            ("shadows", number("Shadow adjustment (-1 to 1)")),
            ("blacks", number("Black point adjustment (-1 to 1)")),
            ("whites", number("White point adjustment (-1 to 1)")),
            ("reset", boolean("If true, reset all color params to neutral before applying")),
        ]),
    }
}

fn apply_effect() -> ToolDefinition {
    ToolDefinition {
        name: "apply_effect",
        description: "Apply non-color effects (blur, sharpen, glow, grain, vignette) to a clip. PR #8.",
        input_schema: object(&[
            ("clipId", string("Clip id")),
            ("type", string("Effect type ID (e.g. 'blur.gaussian', 'stylize.glow', 'detail.sharpen', 'stylize.grain', 'stylize.vignette')")),
            ("enabled", boolean("Enable or disable the effect")),
            ("remove", array("Optional list of effect type IDs to remove from the clip")),
            ("intensity", number("Effect intensity (0 to 1)")),
        ]),
    }
}

fn inspect_color() -> ToolDefinition {
    ToolDefinition {
        name: "inspect_color",
        description: "Inspect color scopes (luma, RGB zones, hue histogram, saturation) of a graded clip or raw asset. PR #8.",
        input_schema: object(&[
            ("clipId", string("Optional clip id to inspect (must specify clipId or mediaRef)")),
            ("mediaRef", string("Optional media asset ref to inspect raw")),
            ("reference", string("Optional reference mediaRef for side-by-side comparison")),
        ]),
    }
}

// ---------------------------------------------------------------------------
// JSON Schema helpers
// ---------------------------------------------------------------------------

fn object(props: &[(&str, Value)]) -> Value {
    let mut map = serde_json::Map::new();
    map.insert("type".to_string(), Value::String("object".to_string()));
    let mut required = Vec::new();
    let mut properties = serde_json::Map::new();
    for (name, schema) in props {
        if !name.is_empty() {
            required.push(Value::String(name.to_string()));
        }
        properties.insert(name.to_string(), schema.clone());
    }
    map.insert("required".to_string(), Value::Array(required));
    map.insert("properties".to_string(), Value::Object(properties));
    Value::Object(map)
}

fn string(description: &str) -> Value {
    let mut map = serde_json::Map::new();
    map.insert("type".to_string(), Value::String("string".to_string()));
    map.insert(
        "description".to_string(),
        Value::String(description.to_string()),
    );
    Value::Object(map)
}

fn integer(description: &str) -> Value {
    let mut map = serde_json::Map::new();
    map.insert("type".to_string(), Value::String("integer".to_string()));
    map.insert(
        "description".to_string(),
        Value::String(description.to_string()),
    );
    Value::Object(map)
}

fn number(description: &str) -> Value {
    let mut map = serde_json::Map::new();
    map.insert("type".to_string(), Value::String("number".to_string()));
    map.insert(
        "description".to_string(),
        Value::String(description.to_string()),
    );
    Value::Object(map)
}

fn boolean(description: &str) -> Value {
    let mut map = serde_json::Map::new();
    map.insert("type".to_string(), Value::String("boolean".to_string()));
    map.insert(
        "description".to_string(),
        Value::String(description.to_string()),
    );
    Value::Object(map)
}

fn array(items_desc: &str) -> Value {
    let mut map = serde_json::Map::new();
    map.insert("type".to_string(), Value::String("array".to_string()));
    map.insert(
        "description".to_string(),
        Value::String(items_desc.to_string()),
    );
    Value::Object(map)
}

fn object_any(description: &str) -> Value {
    let mut map = serde_json::Map::new();
    map.insert("type".to_string(), Value::String("object".to_string()));
    map.insert(
        "description".to_string(),
        Value::String(description.to_string()),
    );
    Value::Object(map)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tdef_001_exactly_42_tools() {
        let tools = all_tools();
        assert_eq!(
            tools.len(),
            42,
            "TDEF-001: exactly 42 tools (39 + apply_color + apply_effect + inspect_color)"
        );
    }

    #[test]
    fn tdef_002_names_are_snake_case() {
        let tools = all_tools();
        for tool in &tools {
            assert!(
                !tool.name.contains('-'),
                "tool '{}' should not contain hyphens",
                tool.name
            );
            assert!(
                tool.name
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c == '_'),
                "tool '{}' has invalid characters",
                tool.name
            );
        }
    }

    #[test]
    fn tdef_002_all_names_are_unique() {
        let tools = all_tools();
        let mut names: Vec<&str> = tools.iter().map(|t| t.name).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), 42, "all 42 tool names must be unique");
    }

    #[test]
    fn tdef_003_each_tool_has_json_schema() {
        let tools = all_tools();
        for tool in &tools {
            let schema = &tool.input_schema;
            assert_eq!(
                schema.get("type").and_then(|v| v.as_str()),
                Some("object"),
                "tool '{}' schema must be type object",
                tool.name
            );
            assert!(
                schema.get("properties").is_some(),
                "tool '{}' schema must have properties",
                tool.name
            );
        }
    }

    #[test]
    fn tdef_003_schema_snapshot_get_timeline() {
        let tools = all_tools();
        let tool = tools.iter().find(|t| t.name == "get_timeline").unwrap();
        let json = serde_json::to_string_pretty(&tool.input_schema).unwrap();
        // get_timeline has no required params
        assert_eq!(
            serde_json::from_str::<Value>(&json)
                .unwrap()
                .pointer("/required")
                .and_then(|v| v.as_array())
                .map(|a| a.len()),
            Some(0)
        );
    }

    #[test]
    fn tdef_003_schema_snapshot_split_clip() {
        let tools = all_tools();
        let tool = tools.iter().find(|t| t.name == "split_clip").unwrap();
        let json = serde_json::to_string_pretty(&tool.input_schema).unwrap();
        let schema: Value = serde_json::from_str(&json).unwrap();
        let required: Vec<&str> = schema
            .pointer("/required")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
            .unwrap_or_default();
        assert!(required.contains(&"clipId"), "split_clip requires clipId");
        assert!(required.contains(&"frame"), "split_clip requires frame");
    }

    #[test]
    fn tdef_004_system_instruction_exists() {
        assert!(!SYSTEM_INSTRUCTION.is_empty());
        assert!(SYSTEM_INSTRUCTION.contains("Fronda"));
    }

    #[test]
    fn tdef_004_instruction_contract_key_guidance() {
        // TDEF-005: key guidance preserved
        assert!(SYSTEM_INSTRUCTION.contains("get_timeline once per session"));
        assert!(SYSTEM_INSTRUCTION.contains("get_media before referencing"));
        assert!(SYSTEM_INSTRUCTION.contains("list_models before any generation"));
        assert!(SYSTEM_INSTRUCTION.contains("inspect_media before describing"));
        assert!(SYSTEM_INSTRUCTION.contains("user confirmation before execution"));
        assert!(SYSTEM_INSTRUCTION.contains("terse and outcome-first"));
    }

    #[test]
    fn aida_005_006_tool_result_support() {
        // AID-005: Tool results support text and image blocks.
        // This is verified via core_model::ToolResultBlock.
        let text = serde_json::json!({"kind": "text", "text": "hello"});
        let img = serde_json::json!({"kind": "image", "base64": "abc", "mediaType": "image/png"});
        let text_block: core_model::ToolResultBlock = serde_json::from_value(text).unwrap();
        let img_block: core_model::ToolResultBlock = serde_json::from_value(img).unwrap();
        assert!(matches!(
            text_block,
            core_model::ToolResultBlock::Text { .. }
        ));
        assert!(matches!(
            img_block,
            core_model::ToolResultBlock::Image { .. }
        ));
    }
}

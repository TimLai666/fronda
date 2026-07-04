//! All 54 agent tool definitions with JSON input schemas (TDEF-001 to TDEF-003).
//! Issue #172: added create_project, open_project, delete_project (42 → 45).
//! Issue #174: added remove_silence (45 → 46).
//! Issue #157: added save_clip_preset, apply_clip_preset, list_clip_presets (46 → 49).
//! Issue #165/#158: added set_clip_noise_reduction, set_clip_audio_effects (49 → 51).
//! Issue #155: added create_compound_clip, dissolve_compound_clip (51 → 53).
//! Issue #154: added import_xml (53 → 54).

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

/// Returns all 54 tools exposed to the agent.
///
/// TDEF-001: tool set (42 original + Issues #172/174/157/165/#158/155/154 additions).
pub fn all_tools() -> Vec<ToolDefinition> {
    vec![
        add_captions(),
        add_clips(),
        apply_layout(),
        create_compound_clip(),
        dissolve_compound_clip(),
        import_xml(),
        add_shapes(),
        add_texts(),
        apply_animation(),
        apply_clip_preset(),
        apply_color(),
        apply_effect(),
        create_folder(),
        create_matte(),
        create_project(),
        delete_folder(),
        delete_media(),
        delete_project(),
        duplicate_project(),
        list_clip_presets(),
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
        open_project(),
        remove_clips(),
        remove_silence(),
        remove_tracks(),
        remove_words(),
        rename_folder(),
        rename_media(),
        ripple_delete_ranges(),
        search_media(),
        set_blend_mode(),
        set_chroma_key(),
        set_clip_properties(),
        set_color_grade(),
        set_keyframes(),
        set_project_settings(),
        read_skill(),
        save_clip_preset(),
        set_clip_audio_effects(),
        set_clip_noise_reduction(),
        split_clips(),
        undo(),
        upscale_media(),
    ]
}

/// TDEF-004: system instruction text for the agent.
pub const SYSTEM_INSTRUCTION: &str = r#"You are a creative AI assistant integrated into Fronda, an AI-native video editor. Help the user build and edit their project by calling the tools available to you.

# Core model
- The timeline has a fixed fps and resolution. All timing is in FRAMES, not seconds: frame = seconds × fps.
- Tracks are ordered and typed (video or audio). Video clips, images, and text overlays all live on video tracks; audio on audio tracks.
- A clip references a media asset and occupies [startFrame, startFrame + durationFrames) on its track.
- Clips carry trimStartFrame / trimEndFrame (source-media offsets, not timeline offsets), speed, volume, opacity, transform, and crop.
- Media assets live in the project library and are referenced by ID. IDs (clipId, mediaRef, folderId, captionGroupId) are short prefixes — pass them back exactly as given; never pad, complete, or guess a longer form.

# Always do
- Call get_timeline once per session (or after an out-of-band change) for fps, tracks, and existing clip frames. Don't re-read between your own edits — mutation tools return the IDs and frames that changed; re-read only after a failure that suggests your model is stale.
- Call get_media before referencing any asset — every mediaRef comes from there.
- Call list_models before any generation or upscale operation so the model you pick supports the duration, aspect ratio, references, voice, or asset type you need.
- Use inspect_media before describing any asset to the user — describe what you actually see, never paraphrase the filename. Work coarse to fine on long media: a storyboard overview, then transcript segments, then zoom into a window for exact frames and word boundaries.
- To find a moment across the library, call search_media before inspecting files one by one — hits are source-second ranges ready to convert into add_clips trims.
- Generation and upscale require credits: if get_timeline reports generation is unavailable, tell the user to sign in and subscribe rather than proposing those tools. Generation operations require explicit user confirmation before execution.

# Editing
Placements must match track type. The editing surface mirrors human gestures — one tool per gesture, applied to a selection:
- add_clips / insert_clips: place media. Clip type and full source length come from the asset; project fps is authoritative (it never changes to match a source). trimStartFrame trims the head, trimEndFrame the tail (mutually exclusive with durationFrames).
- move_clips: change track and/or startFrame. Linked partners follow the frame delta.
- set_clip_properties: apply the same values (duration, trim, speed, volume, opacity, transform, or text style) to one or more clipIds. Setting volume or opacity here clears existing keyframes on that property.
- set_keyframes: replace the keyframe track for one (clipId, property) pair. Frames are clip-relative; an empty array clears.
- split_clips: pass one or more cut points (each strictly inside its clip) in one call. Splits only insert boundaries; nothing shifts.
- remove_words: cut speech by the word — pass get_transcript indices (or exact `matches` tokens like "um"/"uh") to drop those words plus the surrounding pause; linked A/V partners are cut automatically and gaps close. Prefer this for anything you can point at in the transcript; re-read get_transcript afterwards.
- ripple_delete_ranges: cut spans out and close the gaps in one action — the fast path for non-word-aligned dead-air removal.
- apply_layout: for any multi-video composition (split screen, picture-in-picture, grid), assign a clip to each slot instead of hand-setting transforms; it fills every region without stretching.
- set_project_settings: change fps, resolution, or aspect ratio; existing clips re-fit and frame values rescale automatically.

Keep replies terse and outcome-first. Always verify clip and track IDs exist before referencing them."#;

/// The always-on skill index appended to the system prompt (upstream #199).
/// Empty when no skills are loaded. Mirrors Swift `SkillStore.promptIndex`.
pub fn skill_prompt_index(skills: &[crate::tool_exec::AgentSkill]) -> String {
    if skills.is_empty() {
        return String::new();
    }
    let lines = skills
        .iter()
        .map(|s| format!("- {}: {}", s.id, s.description))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "\n\n# Skills\nPlaybooks for specific tasks. Before a task that matches one, \
call read_skill(id) to load its full procedure, then follow it.\n{lines}"
    )
}

/// The system instruction with the skill index appended (upstream #199). The
/// app builds the agent request from this so the model knows what skills exist.
pub fn system_instruction_with_skills(skills: &[crate::tool_exec::AgentSkill]) -> String {
    format!("{SYSTEM_INSTRUCTION}{}", skill_prompt_index(skills))
}

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
        description: "Add media clips to the end of the timeline. Clip type and \
            full source length are taken from the media asset; project fps is \
            authoritative and is not changed to match the source. A video-with-audio \
            asset placed on a video track also gets a linked audio clip on an audio \
            track (created if needed).",
        input_schema: object(&[
            ("mediaIds", array("Media asset ids to add")),
            (
                "trackIndex",
                integer("Optional target track index (0-based). Omit to auto-create/reuse a video track for visual clips and an audio track for audio clips."),
            ),
            (
                "trimStartFrame",
                integer("Optional head trim (in-point), in project frames. Default 0."),
            ),
            (
                "trimEndFrame",
                integer("Optional tail trim (out-point), in project frames. \
                    Mutually exclusive with durationFrames. Omit both to place \
                    the full remaining source (extendable)."),
            ),
            (
                "durationFrames",
                integer("Optional visible duration, in project frames. Derived from \
                    the source when omitted. Mutually exclusive with trimEndFrame."),
            ),
        ]),
    }
}

fn add_texts() -> ToolDefinition {
    ToolDefinition {
        name: "add_texts",
        description: "Add one or more text overlay clips (titles, lower-thirds) in a \
            single undoable action. Pass a `texts` array; each entry takes: content \
            (the text), startFrame, durationFrames, and optional styling — fontName, \
            fontSize, fontWeight (400 = regular, 700 = bold), color ('#RGB' / \
            '#RRGGBB' / '#RRGGBBAA'), alignment ('left' / 'center' / 'right'), \
            transform ({centerX, centerY, width, height} in 0–1 normalized canvas \
            coords; centre-only shifts position), and animation ('off', 'fadeIn', \
            'popIn', 'slideUp', 'typewriter', 'wordReveal', 'wordSlide', 'wordPop', \
            'wordCycle', 'highlightPop', 'highlightBlock') with an optional \
            highlightColor hex for the per-word highlight presets. For captioning \
            spoken audio, prefer add_captions.",
        input_schema: object(&[(
            "texts",
            array(
                "Array of {content, startFrame, durationFrames, fontName?, fontSize?, color?, alignment?, transform?, animation?, highlightColor?}",
            ),
        )]),
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
            "Return the media manifest as JSON. Pass optional folderId to scope to a folder. \
             Also exposes generationStatus (preparing | generating | downloading | failed | none) \
             for async-generated assets — wait until 'none' before referencing them.",
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
            ("wordTimestamps", string("Legacy flag: tolerated and ignored for backward compatibility.")),
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
        description: "Insert clips at a specific frame position, pushing existing \
            content later. Clip type and source length come from the media asset; \
            project fps is authoritative and is not changed to match the source.",
        input_schema: object(&[
            ("mediaIds", array("Media asset ids to insert")),
            ("frame", integer("Insertion frame position")),
            (
                "trimStartFrame",
                integer("Optional head trim (in-point), in project frames. Default 0."),
            ),
            (
                "trimEndFrame",
                integer("Optional tail trim (out-point), in project frames. \
                    Mutually exclusive with durationFrames."),
            ),
            (
                "durationFrames",
                integer("Optional visible duration, in project frames. Derived from \
                    the source when omitted. Mutually exclusive with trimEndFrame."),
            ),
        ]),
    }
}

fn set_project_settings() -> ToolDefinition {
    ToolDefinition {
        name: "set_project_settings",
        description: "Change the project's frame rate, resolution, or aspect ratio. \
            Pass any combination of fps, explicit width+height, aspectRatio, and \
            quality. aspectRatio and explicit width/height are mutually exclusive; \
            quality scales the current aspect ratio (or the aspectRatio preset when \
            combined). Existing clips are re-fitted automatically: auto-fit transforms \
            reset to the new canvas, and all frame positions/durations rescale when \
            fps changes. Undoable.",
        input_schema: object_optional(&[
            (
                "fps",
                integer("Frame rate (1-120). Common: 24, 25, 30, 48, 50, 60."),
            ),
            (
                "width",
                integer("Canvas width in px. Use with height. Mutually exclusive with aspectRatio."),
            ),
            (
                "height",
                integer("Canvas height in px. Use with width. Mutually exclusive with aspectRatio."),
            ),
            (
                "aspectRatio",
                string("Preset aspect ratio: 16:9, 9:16, 1:1, 4:3, 2.4:1, or 9:14. Mutually exclusive with width/height."),
            ),
            (
                "quality",
                string("Resolution preset: 720p, 1080p, 2K, or 4K. Scales the short edge, preserving aspect."),
            ),
        ]),
    }
}

fn apply_layout() -> ToolDefinition {
    ToolDefinition {
        name: "apply_layout",
        description: "Arrange clips into a common multi-video layout (split screen, \
            picture-in-picture, grid) in one undoable action — the fast path for \
            composing several videos in one frame instead of hand-setting transforms. \
            Pick a named layout and assign a clip to each of its slots; the tool \
            computes every transform and crop so each clip fills its region without \
            stretching (source cropped to the slot's shape). Pass fit='fit' to \
            letterbox the whole source inside its slot instead (no crop). Crop is \
            centered by default; bias it with 'anchor' (top/bottom/left/right/...) or \
            continuous anchorX/anchorY (0-1). \
            Two modes (don't mix them across slots): Re-layout mode — give each slot a \
            'clipIds' array (or a single 'clipId'); only transforms and crop change, \
            timing and tracks are untouched. Clips sharing a slot may sit on the same \
            track; clips in different slots must be co-visible (overlap in time on \
            separate tracks). Place-new mode — give each slot a 'mediaRef' plus \
            top-level 'startFrame'/'durationFrames'; the tool creates one stacked video \
            track per slot and places a new clip in each, framed to its region. \
            Layouts and slots: full=main; side_by_side=left,right; \
            top_bottom=top,bottom; pip_bottom_right/pip_bottom_left/pip_top_right/\
            pip_top_left=main,inset; grid_2x2=top_left,top_right,bottom_left,\
            bottom_right; main_sidebar=main,sidebar; three_up=left,center,right.",
        input_schema: object_optional(&[
            ("layout", string("Layout name (e.g. side_by_side, grid_2x2, pip_bottom_right). Required.")),
            ("slots", array("Required. One entry per slot: {slot, then clipIds (array)/clipId to re-layout OR mediaRef to place new, optional anchor/anchorX/anchorY}. Every slot must be filled; each clip fills one slot.")),
            ("fit", string("'fill' (cover-crop, default) or 'fit' (letterbox, no crop).")),
            ("startFrame", integer("Place-new mode only: timeline frame for the new clips (default 0).")),
            ("durationFrames", integer("Place-new mode only: length of the new clips in frames (required, >= 1).")),
        ]),
    }
}

fn read_skill() -> ToolDefinition {
    ToolDefinition {
        name: "read_skill",
        description: "Load a skill's full SKILL.md procedure by id. The system \
            prompt lists available skills; before a task that matches one, call \
            read_skill(id) and follow the returned playbook.",
        input_schema: object(&[("id", string("Skill id (its folder name)."))]),
    }
}

fn inspect_media() -> ToolDefinition {
    ToolDefinition {
        name: "inspect_media",
        description: "Inspect a media asset and return details. Transcription defaults to system language — pass language when the audio is in another language.",
        input_schema: object(&[
            ("mediaId", string("Media asset id to inspect")),
            ("clipId", string("Optional clip id for cross-validation (READ-014)")),
            ("language", string("Optional BCP-47 spoken language (e.g. 'fr', 'ja', 'en-GB'). Overrides project transcriptionLanguage for this call; falls back to system language if neither is set.")),
            ("maxFrames", string("Optional max frames for storyboard (default 6, max 12, READ-015)")),
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
        description: "Delete frame ranges from the timeline with ripple. Sync-locked tracks are cut \
            in sync with the anchor and their gaps closed. List a track in \
            ignoreSyncLockTrackIndices to treat it as unlocked for this call — it is left in \
            place, neither cut nor shifted.",
        input_schema: object(&[
            (
                "ranges",
                array("Array of {start, end} frame ranges to delete"),
            ),
            ("trackIndex", integer("Optional: scope to specific track")),
            (
                "ignoreSyncLockTrackIndices",
                array("Optional: track indices to treat as unlocked (left in place) for this call"),
            ),
        ]),
    }
}

fn remove_words() -> ToolDefinition {
    ToolDefinition {
        name: "remove_words",
        description: "Cut speech by the word, Descript-style — the primary tool for text-based \
            editing (filler words, flubbed sentences, dropped retakes, tightening a ramble). Pass \
            `words` for precise get_transcript indices/ranges, or `matches` for exact filler tokens \
            like \"um\" and \"uh\". This resolves them to frames, removes the surrounding pause so \
            survivors don't end up double-spaced, merges adjacent removals, cuts linked A/V \
            partners, and closes the gaps. Words across multiple clips on ONE track are handled in \
            a single undoable action; if your selection spans multiple UNLINKED tracks the call is \
            refused — cut one track at a time, or link the tracks first. After it runs, indices \
            have shifted — re-read get_transcript before another remove_words.",
        input_schema: object_optional(&[
            (
                "words",
                array(
                    "Words to remove, by get_transcript index. Each element is a single index \
                    (e.g. 42) or an inclusive [startIndex, endIndex] span (e.g. [12, 18]). Mix \
                    freely: [3, [12, 18], 40]. Mutually exclusive with matches. Re-read after any edit.",
                ),
            ),
            (
                "matches",
                array(
                    "Exact single-word tokens to remove everywhere, case-insensitive with \
                    surrounding punctuation ignored, e.g. [\"um\", \"uh\", \"hmm\"]. Mutually \
                    exclusive with words. Avoid broad words like \"like\" unless the user wants every occurrence.",
                ),
            ),
            (
                "cutAggressiveness",
                string(
                    "How much silence to leave between the words on either side of a cut: 'tight' \
                    (snappy), 'balanced' (default, natural beat), or 'loose' (more breathing room). \
                    The removed words' own frames always go regardless.",
                ),
            ),
        ]),
    }
}

fn create_matte() -> ToolDefinition {
    ToolDefinition {
        name: "create_matte",
        description: "Add a solid-colour image (matte) to the media library — a plain colour fill \
            for backgrounds, lower-thirds, or letterbox bars. `hex` is required (e.g. '#000000'). \
            `aspectRatio` sets the size: 'Project' (default, matches the timeline) or a fixed ratio \
            (16:9, 9:16, 1:1, 4:3, 9:14, 2.4:1) fit to the timeline's short edge. Returns the new \
            mediaRef; place it with add_clips.",
        input_schema: object_optional(&[
            ("hex", string("Fill colour as '#RGB' / '#RRGGBB' (required).")),
            (
                "aspectRatio",
                string("Optional: 'Project' (default) or 16:9 / 9:16 / 1:1 / 4:3 / 9:14 / 2.4:1."),
            ),
            ("name", string("Optional asset name (default 'Matte').")),
            ("folderId", string("Optional folder id to place the asset in.")),
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
        description: "Apply property values to one or more clips in a single undoable \
            action. `properties` is an object; pass any combination of: \
            durationFrames, trimStartFrame, trimEndFrame, speed, volume (0-1), \
            opacity (0-1), transform ({centerX, centerY, width, height, rotation, \
            flipHorizontal, flipVertical} — partial merge, 0-1 normalized canvas \
            coords). For text clips only: content (string), fontName, fontSize, \
            fontWeight (400 = regular, 700 = bold), color ('#RGB' / '#RRGGBB' / \
            '#RRGGBBAA'), alignment ('left' / 'center' / 'right'), background and \
            border (each {enabled, color, padding?, cornerRadius?} for the caption \
            fill/stroke). Setting volume or opacity here clears any keyframe track \
            on that property.",
        input_schema: object(&[
            ("clipIds", array("Clip ids to modify")),
            (
                "properties",
                object_any(
                    "Properties to set: durationFrames, trimStartFrame, trimEndFrame, speed, volume, opacity, transform, and (text clips) content, fontName, fontSize, color, alignment",
                ),
            ),
        ]),
    }
}

fn set_keyframes() -> ToolDefinition {
    ToolDefinition {
        name: "set_keyframes",
        description: "Set animated keyframes on one property of one clip. Replaces the \
            existing keyframe track for that property (pass an empty array to clear). \
            Frames are CLIP-RELATIVE offsets (0 = first frame of the clip). Rows are \
            sorted by frame internally and the LAST row for any duplicate frame wins. \
            Each row is `[frame, ...values, interp?]` where interp ∈ {linear, hold, \
            smooth} (default smooth). Value layouts per property:\n\
            • volume `[frame, value]` — value is decibels (0 = unity)\n\
            • opacity `[frame, value]` — value 0.0–1.0\n\
            • rotation `[frame, degrees]` — clockwise degrees\n\
            • position `[frame, topLeftX, topLeftY]` — TOP-LEFT corner in 0–1 canvas \
            coords, NOT the centre\n\
            • scale `[frame, width, height]` — normalized 0–1 canvas size, NOT a factor\n\
            • crop `[frame, top, right, bottom, left]` — side insets in 0–1 of the source",
        input_schema: object(&[
            ("clipId", string("Clip id")),
            (
                "property",
                string("One of: opacity, volume, rotation, position, scale, crop"),
            ),
            (
                "keyframes",
                array("Rows of [frame, ...values, interp?] — see the tool description"),
            ),
        ]),
    }
}

fn split_clips() -> ToolDefinition {
    ToolDefinition {
        name: "split_clips",
        description: "Split clips at one or more cut points in a single undoable \
            action. A split only inserts a boundary — nothing trims or shifts. Pass \
            exactly one mode: 'splits' (array of {clipId, atFrame} in project frames) \
            or 'trackIndex' + 'frames' (cut one track at the given project frames, \
            each matched to the clip containing it). Every frame must fall strictly \
            between a clip's start and end; multiple cuts on the same clip are \
            allowed; duplicate points are ignored. Linked audio/video partners are \
            split at the same frame and their right halves regrouped. One bad cut \
            point rejects the whole call with no partial state.",
        input_schema: object_optional(&[
            (
                "splits",
                array("Explicit cuts; each item is an object {clipId, atFrame} with atFrame a project frame strictly inside the clip."),
            ),
            ("trackIndex", integer("Track to cut (use with 'frames').")),
            (
                "frames",
                array("Project frames to cut on trackIndex; each matched to the clip containing it."),
            ),
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

/// Like [`object`] but every property is optional (empty `required`). For tools
/// whose parameters are all optional or "exactly one of" — over-declaring them
/// required makes strict MCP clients reject valid calls.
fn object_optional(props: &[(&str, Value)]) -> Value {
    let mut map = serde_json::Map::new();
    map.insert("type".to_string(), Value::String("object".to_string()));
    map.insert("required".to_string(), Value::Array(Vec::new()));
    let mut properties = serde_json::Map::new();
    for (name, schema) in props {
        properties.insert(name.to_string(), schema.clone());
    }
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

// ── Issues #165/#158: audio effect MCP tools ─────────────────────────────────

fn set_clip_noise_reduction() -> ToolDefinition {
    ToolDefinition {
        name: "set_clip_noise_reduction",
        description: "Apply noise reduction to a clip's audio. \
            Uses on-device audio processing (VoiceProcessingIO / AVAudioUnitEQ). \
            No AI dependency. Issue #165.",
        input_schema: object(&[
            ("clipId", string("Clip id (must have an audio track)")),
            (
                "amount",
                number("Reduction strength 0.0 (off) to 1.0 (maximum). Default: 0.5."),
            ),
            (
                "sensitivity",
                number("Noise floor sensitivity 0.0 to 1.0. Default: 0.5."),
            ),
            (
                "smoothing",
                string("Temporal smoothing: 'low', 'medium', or 'high'. Default: 'medium'."),
            ),
            (
                "enabled",
                boolean("Enable or disable the effect. Default: true."),
            ),
        ]),
    }
}

fn set_clip_audio_effects() -> ToolDefinition {
    ToolDefinition {
        name: "set_clip_audio_effects",
        description: "Apply audio shaping effects (EQ, compressor, pitch, reverb) to a clip. \
            Issue #158.",
        input_schema: object(&[
            ("clipId", string("Clip id (must have an audio track)")),
            (
                "eq",
                object_any("Optional EQ settings: {lowGain, midGain, highGain} in dB (-12 to 12)"),
            ),
            (
                "compressor",
                object_any("Optional compressor: {threshold_db, ratio, attack_ms, release_ms}"),
            ),
            (
                "pitchShift",
                number("Pitch shift in semitones (-12 to 12). Default: 0."),
            ),
            (
                "reverb",
                object_any("Optional reverb: {roomSize 0-1, wetDry 0-1}"),
            ),
        ]),
    }
}

// ── Issue #154: XML import MCP tool ───────────────────────────────────────────

fn import_xml() -> ToolDefinition {
    ToolDefinition {
        name: "import_xml",
        description: "Import a timeline from an XML file (XMEML, FCPXML, Premiere Pro XML, \
            or DaVinci Resolve XML). The imported timeline is merged into the current project. \
            Issue #154.",
        input_schema: object(&[
            ("path", string("File system path to the XML file (.xml, .fcpxml)")),
            (
                "format",
                string("XML format hint: 'xmeml', 'fcpxml', 'premiere', or 'davinci'. \
                    Auto-detected from file extension and content if omitted."),
            ),
            (
                "preserveProjectFps",
                boolean("Keep the current project FPS rather than adopting the imported timeline's FPS. Default: false."),
            ),
        ]),
    }
}

// ── Issue #155: compound clip MCP tools ───────────────────────────────────────

fn create_compound_clip() -> ToolDefinition {
    ToolDefinition {
        name: "create_compound_clip",
        description: "Group selected clips into a compound clip (nested sequence). \
            The selected clips are replaced with a single compound clip on the timeline. \
            Issue #155.",
        input_schema: object(&[
            ("clipIds", array("Clip ids to nest into the compound clip")),
            ("name", string("Optional name for the compound clip")),
        ]),
    }
}

fn dissolve_compound_clip() -> ToolDefinition {
    ToolDefinition {
        name: "dissolve_compound_clip",
        description: "Dissolve a compound clip back to its constituent clips on the timeline. \
            Issue #155.",
        input_schema: object(&[("clipId", string("Id of the compound clip to dissolve"))]),
    }
}

// ── Issue #174: silence removal MCP tool ─────────────────────────────────────

fn remove_silence() -> ToolDefinition {
    ToolDefinition {
        name: "remove_silence",
        description: "Detect and ripple-delete silent regions in a clip using on-device RMS analysis. \
            No AI or transcription dependency. Issue #174.",
        input_schema: object(&[
            ("clipId", string("Clip id to process (must be a single audio or video clip)")),
            (
                "thresholdDb",
                number("RMS amplitude threshold in dBFS (e.g. -40.0). Regions below this are considered silent. Default: -40."),
            ),
            (
                "minSilenceSeconds",
                number("Minimum silence duration to remove in seconds. Default: 0.5."),
            ),
            (
                "edgePaddingSeconds",
                number("Seconds of padding to leave at each edge of a silent region. Default: 0.1."),
            ),
        ]),
    }
}

// ── Issue #157: named clip preset MCP tools ────────────────────────────────

fn save_clip_preset() -> ToolDefinition {
    ToolDefinition {
        name: "save_clip_preset",
        description: "Save the current settings of a clip as a named preset for reuse. \
            The preset captures color grade, transform, speed, and other per-clip properties. \
            Issue #157.",
        input_schema: object(&[
            ("clipId", string("Source clip id to capture settings from")),
            (
                "name",
                string("Preset name (e.g. 'Outdoor warm', 'Interview tight')"),
            ),
        ]),
    }
}

fn apply_clip_preset() -> ToolDefinition {
    ToolDefinition {
        name: "apply_clip_preset",
        description: "Apply a named preset to one or more clips. \
            Issue #157.",
        input_schema: object(&[
            ("presetName", string("Name of the preset to apply")),
            ("clipIds", array("Clip ids to apply the preset to")),
        ]),
    }
}

fn list_clip_presets() -> ToolDefinition {
    ToolDefinition {
        name: "list_clip_presets",
        description: "List all saved named clip presets. \
            Issue #157.",
        input_schema: object(&[]),
    }
}

// ── Issue #172: project lifecycle MCP tools ──────────────────────────────────

fn create_project() -> ToolDefinition {
    ToolDefinition {
        name: "create_project",
        description: "Create a new Palmier project at the given path with an optional name. \
            After creation the new project becomes the active project. \
            Issue #172.",
        input_schema: object(&[
            (
                "path",
                string("File system path where the new .palmier project should be created"),
            ),
            (
                "name",
                string("Optional project name (defaults to the file's stem)"),
            ),
        ]),
    }
}

fn open_project() -> ToolDefinition {
    ToolDefinition {
        name: "open_project",
        description: "Open an existing Palmier project by path. \
            The project becomes the active project. \
            Issue #172.",
        input_schema: object(&[(
            "path",
            string("File system path to an existing .palmier project package"),
        )]),
    }
}

fn delete_project() -> ToolDefinition {
    ToolDefinition {
        name: "delete_project",
        description: "Move a Palmier project to the Trash. \
            This closes the project if it is currently open. \
            Issue #172.",
        input_schema: object(&[(
            "path",
            string("File system path to the .palmier project to delete"),
        )]),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tdef_001_exactly_54_tools() {
        let tools = all_tools();
        assert_eq!(
            tools.len(),
            59,
            "TDEF-001: 59 tools (58 + create_matte, upstream #242)"
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
        assert_eq!(names.len(), 59, "all 59 tool names must be unique");
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
    fn tdef_003_schema_snapshot_split_clips() {
        let tools = all_tools();
        let tool = tools.iter().find(|t| t.name == "split_clips").unwrap();
        let json = serde_json::to_string_pretty(&tool.input_schema).unwrap();
        let schema: Value = serde_json::from_str(&json).unwrap();
        let props = schema
            .pointer("/properties")
            .and_then(|v| v.as_object())
            .expect("split_clips schema has properties");
        assert!(props.contains_key("splits"), "split_clips has splits");
        assert!(props.contains_key("trackIndex"), "split_clips has trackIndex");
        assert!(props.contains_key("frames"), "split_clips has frames");
        // Modes are exactly-one-of, so nothing is unconditionally required.
        let required = schema
            .pointer("/required")
            .and_then(|v| v.as_array())
            .expect("has required array");
        assert!(required.is_empty(), "split_clips has no required props");
    }

    #[test]
    fn system_instruction_with_skills_appends_index() {
        use crate::tool_exec::AgentSkill;
        // No skills → unchanged.
        assert_eq!(system_instruction_with_skills(&[]), SYSTEM_INSTRUCTION);
        // With skills → index appended.
        let skills = vec![AgentSkill {
            id: "captions".into(),
            name: "Captions".into(),
            description: "burn in captions".into(),
            body: String::new(),
        }];
        let prompt = system_instruction_with_skills(&skills);
        assert!(prompt.starts_with(SYSTEM_INSTRUCTION));
        assert!(prompt.contains("# Skills"));
        assert!(prompt.contains("- captions: burn in captions"));
        assert!(prompt.contains("read_skill(id)"));
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
    fn system_instruction_has_core_model_and_editing_sections() {
        // Expanded from a stub into a full editing guide (Swift AgentInstructions parity).
        assert!(SYSTEM_INSTRUCTION.contains("# Core model"));
        assert!(SYSTEM_INSTRUCTION.contains("# Editing"));
        assert!(SYSTEM_INSTRUCTION.contains("FRAMES"), "frame-based model stated");
        assert!(SYSTEM_INSTRUCTION.contains("apply_layout"), "layout gesture");
        assert!(SYSTEM_INSTRUCTION.contains("ripple_delete_ranges"));
        assert!(SYSTEM_INSTRUCTION.contains("set_keyframes"));
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

//! All 56 agent tool definitions with JSON input schemas (TDEF-001 to TDEF-003).
//! Issue #172: added create_project, open_project, delete_project (42 → 45).
//! Issue #174: added remove_silence (45 → 46).
//! Issue #157: added save_clip_preset, apply_clip_preset, list_clip_presets (46 → 49).
//! Issue #165/#158: added set_clip_noise_reduction, set_clip_audio_effects (49 → 51).
//! Issue #155: added create_compound_clip, dissolve_compound_clip (51 → 53).
//! Issue #154: added import_xml (53 → 54).
//! Issue #119: added sync_audio_clips (59 → 60).
//! Upstream #251: replaced the speculative set_clip_noise_reduction/set_clip_audio_effects
//! (never shipped upstream) with the real denoise_audio tool (60 → 59).
//! Upstream #255: added create_timeline, set_active_timeline, duplicate_timeline (59 → 62).
//! v0.6.1 surface alignment: sync_audio_clips renamed to upstream's sync_audio; the
//! speculative import_xml (never shipped upstream) removed (62 → 61).
//! v0.6.1 gap port: added upstream's update_text (61 → 62).
//! v0.6.1 gap port: added upstream's export_project via the ExportHost seam (62 → 63).
//! v0.6.1 gap port: added upstream's read-only get_projects via ProjectLister (63 → 64).
//! v0.6.1 nav port: open_project implemented + new_project added via ProjectNavigator;
//! the speculative create_project/delete_project stubs removed (64 → 63).
//! Upstream #152: added send_feedback via the FeedbackSender seam (63 → 64).
//! tool-surface-v2 phases 2-3 (upstream #263 @141c69b): added organize_media,
//! manage_tracks, close_project (+3); retired create_folder, rename_folder,
//! delete_folder, move_to_folder, rename_media, delete_media (→ organize_media),
//! remove_tracks (→ manage_tracks), create_matte + import_folder (→ import_media
//! source.matte/source.path-directory), duplicate_timeline (→ create_timeline
//! 'from') (−10). 64 + 3 − 10 = 57.
//! tool-surface-v2 phases 4-5: retired list_folders (→ get_media),
//! set_blend_mode (→ set_clip_properties.blendMode), set_chroma_key
//! (→ apply_effect key.chroma), set_color_grade (→ apply_color),
//! generate_music (→ generate_audio) (−5); added detect_beats (+1);
//! sync_audio renamed to sync_clips. 57 − 5 + 1 = 53 (design.md C-1:
//! 45 upstream + 8 Rust extensions).
//! multicam-engine (upstream #283): the three reserved multicam slots landed —
//! manage_multicam, change_cam, get_multicam (53 → 56 = 48 upstream + 8 Rust
//! extensions). Host split (C-1): shared 51 + 4 MCP-only project tools +
//! 1 in-app-only read_skill → MCP surface 55, in-app surface 52.
//! upstream-m-batch (#176): added duplicate_clips, a shared clip-mutation tool
//! (56 → 57 = 49 upstream + 8 Rust extensions). Host split: shared 52 + 4
//! MCP-only + 1 in-app-only → MCP surface 56, in-app surface 53.
//! manage-project-tool (upstream #299 @b8a1491d): get_projects/open_project/
//! new_project/close_project consolidated into the single manage_project
//! (action = list|open|create|close) (57 → 54 = 46 upstream + 8 Rust
//! extensions). Host split: shared 52 + 1 MCP-only + 1 in-app-only →
//! MCP surface 53, in-app surface 53.

use serde::Serialize;
use serde_json::Value;

/// Which host surface exposes a tool (tool-surface-v2 C-1): both hosts,
/// the MCP server only (project navigation), or the in-app agent only
/// (read_skill).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolHost {
    Shared,
    McpOnly,
    InAppOnly,
}

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

impl ToolDefinition {
    /// Host marker (tool-surface-v2 C-1).
    pub fn host(&self) -> ToolHost {
        tool_host(self.name)
    }
}

/// Host marker for a tool name (tool-surface-v2 C-1): manage_project is
/// MCP-only (the in-app agent always has its project open);
/// read_skill is in-app-only (skills live in the app's prompt).
pub fn tool_host(name: &str) -> ToolHost {
    match name {
        "manage_project" => ToolHost::McpOnly,
        "read_skill" => ToolHost::InAppOnly,
        _ => ToolHost::Shared,
    }
}

/// The MCP server surface (C-1): shared tools + manage_project (53).
pub fn mcp_tools() -> Vec<ToolDefinition> {
    all_tools()
        .into_iter()
        .filter(|t| t.host() != ToolHost::InAppOnly)
        .collect()
}

/// The in-app agent surface (C-1): shared tools + read_skill (53).
pub fn in_app_tools() -> Vec<ToolDefinition> {
    all_tools()
        .into_iter()
        .filter(|t| t.host() != ToolHost::McpOnly)
        .collect()
}

/// Returns all 54 tools across both host surfaces.
///
/// TDEF-001: tool set (see the header history; design.md C-1).
pub fn all_tools() -> Vec<ToolDefinition> {
    vec![
        add_captions(),
        add_clips(),
        apply_layout(),
        manage_project(),
        create_compound_clip(),
        create_timeline(),
        dissolve_compound_clip(),
        set_active_timeline(),
        add_shapes(),
        add_texts(),
        update_text(),
        apply_animation(),
        apply_clip_preset(),
        apply_color(),
        apply_effect(),
        duplicate_project(),
        list_clip_presets(),
        generate_audio(),
        generate_image(),
        generate_video(),
        export_project(),
        get_media(),
        get_timeline(),
        get_transcript(),
        import_media(),
        insert_clips(),
        inspect_color(),
        inspect_media(),
        inspect_timeline(),
        list_models(),
        manage_tracks(),
        manage_multicam(),
        change_cam(),
        get_multicam(),
        move_clips(),
        duplicate_clips(),
        organize_media(),
        remove_clips(),
        remove_silence(),
        sync_clips(),
        detect_beats(),
        remove_words(),
        ripple_delete_ranges(),
        search_media(),
        send_feedback(),
        set_clip_properties(),
        set_keyframes(),
        set_project_settings(),
        read_skill(),
        denoise_audio(),
        save_clip_preset(),
        split_clips(),
        undo(),
        upscale_media(),
    ]
}

/// Upstream v2 agent instructions (tool-surface-v2 design Appendix B-1,
/// verbatim from upstream/main@141c69b AgentInstructions.serverInstructions;
/// single adaptation: the product name reads Fronda) + the delimited Fronda
/// extension section (task 5.1).
pub const SERVER_INSTRUCTIONS: &str = r#"You are a creative AI assistant connected to Fronda, an AI-native video editor. Help the user build and edit their project by calling the tools this server exposes.

# Core model
- Timing: TIMELINE positions are project frames (startFrame, frames pairs, gaps, ranges); SOURCE positions are seconds (source spans, search hits, asset transcripts and durations). Tools convert between them — never multiply by fps yourself.
- Tracks are ordered and typed (video or audio); index 0 renders on top. For manage_tracks, use stable trackId values because indexes change. Video, images, and text use video tracks.
- A clip occupies frames [start, end). Placement takes startFrame + endFrame or source: [startSeconds, endSeconds]; lengths elsewhere are durationFrames. A video clip's linked audio is folded into it as audio: {id, track, …} — use that nested id to edit the audio side.
- A project can hold several timelines; exactly one is active and every read/edit tool targets it (get_media lists them; switch with set_active_timeline, then re-read). A nested timeline appears as a clip with mediaType 'sequence'.
- IDs are short prefixes — pass them back exactly as given, never padded or completed. Folders have no ids: they are paths ('B-roll/Sunset'), created on demand.

# Session
- Call get_timeline once per session (or after an out-of-band change). Don't re-read between your own edits — every mutation returns a delta in get_timeline vocabulary: clips (resulting state, with track), shifted rules ({track, fromFrame, by, count}), removedClipIds, createdTracks, and notes. Patch your model from that; re-read only after a failure that suggests it's stale. Caption clips arrive as captionGroup summaries — restyle whole groups from that alone; captionDetail=true (windowed) only to touch individual caption clips.
- Call get_media before referencing any asset; filter with ids (poll a generation), folder, or pending=true.
- Call list_models before any generate_* or upscale call. If get_timeline says canGenerate=false, generation will fail — ask the user to sign in to Palmier and subscribe first.
- Never describe an asset from its filename — inspect_media first. On long media work coarse to fine: overview=true storyboard, then transcript segments, then zoom with startSeconds/endSeconds.
- To find a moment ("the sunset shot", "where she mentions the budget"): search_media first, then pass hits straight to add_clips as source: [startSeconds, endSeconds].

# Editing
- Edits are undoable and effectively free — don't ask permission for individual edits; just say what changed.
- Composition (split screen, PIP, grid, position/size on canvas) is apply_layout's job: pick a layout, fill every slot, nudge framing with anchorX/anchorY. Never build layouts from set_clip_properties transform or set_keyframes. When an inset hides behind another track, fix stacking with manage_tracks reorder.
- Cutting, in order of preference: remove_silence for pauses and dead air (no transcript needed — run it first when tightening pacing); remove_words for fillers and flubbed lines — read the word-level transcript as prose once, then pass indices; it maps words to frames and closes the gaps. After a cut, indices shift — re-read get_transcript before the next remove_words. ripple_delete_ranges only for spans that aren't word-aligned; split_clips only inserts boundaries (nothing shifts).
- Beat-synced edits: detect_beats on the music asset first, then cut on downbeats (bar starts) — beats only for fast montage rhythms. Times are source seconds.
- Text: add_texts for authored overlays; add_captions transcribes the timeline's spoken audio (no targeting) — restyle with update_text and the returned captionGroupId. Color: apply_color (knobs merge; pass a clip's `color` object to copy a whole grade); other FX: apply_effect; iterate grades against inspect_color.
- Transcription language: omit unless the user names the spoken language. Cloud auto-detects; local is language-specific — pass BCP-47 (language='es') for non-English local runs, and if local output looks wrong, ask for the language and retry.
- A transcript summary is lossy: it hides reworded retakes and zero-width seam fragments (a word whose start equals the next word's start) — verify suspected fragments against the words, not the summary.

# Export
- export_project modes: video (default — H.264/H.265/ProRes, 720p–4K or Match Timeline), xml (Premiere), fcpxml (Resolve / Final Cut), palmier (self-contained package). Omit outputPath unless the user named a destination (default ~/Downloads). Video renders in the background — say so; a notification reports completion. The other modes finish inline.

# Generation
- Costs real money and is not undoable: propose prompt, model, duration, and aspect ratio, then wait for confirmation.
- Flow: images first — iterate stills until the user approves the look, then use the approved image as the video's startFrameMediaRef. Straight text-to-video only when asked or when no frame anchors the shot.
- Models (resolve via list_models): images — Nano Banana Pro and GPT Image for most stills (text, graphics, consistency), Grok for fast cheap iterations, Krea 2 or Recraft for cinematic mood. Video — Seedance 2.0 Fast at 720p while iterating, regular Seedance 2.0 for the approved take, Kling v3 if Seedance errors, Grok Imagine only for very simple scenes, Veo rarely.
- Generation and url/path imports return a placeholder id and run in the background. Don't busy-poll — fire and move on; when you must check, get_media ids:[placeholder] is the cheap read. On generationStatus 'failed', tell the user and ask before re-firing.
- Consistency: reuse referenceMediaRefs on images; startFrameMediaRef / endFrameMediaRef and the per-model reference*MediaRefs on video. Build base shots before derived ones; parallelize independent generations; organize related generations with a `folder` path on the call.
- Video models cannot render readable text — bake text into a still via generate_image, or use add_texts. Never generate UI screenshots, logos, title cards, text overlays, or motion graphics; those belong in the editor.
- import_media bridges external assets (url, path, or bytes) and makes solid-color mattes (source.matte with hex).
- Audio models (list_models type='audio'): TTS — the prompt is the exact words to speak; pass a supported voice, styleInstructions where offered. Music — the prompt describes style/mood/genre; lyrics with [Verse]/[Chorus] tags where supported (for Lyria 3 Pro, fold lyrics/tempo/language/vocal style into the prompt); instrumental only where supported.

# Prompt craft
- Images, 15–30 words: subject + setting + shot type + lighting/mood. Concrete nouns beat adjectives.
- Videos, 8–20 words: camera movement + subject action. With a startFrameMediaRef, don't re-describe the frame — spend the words on motion and sound. State dialogue, VO, SFX, and music explicitly; silent video is usually a bug.

# Feedback
- When a capability is missing or broken, a result is clearly wrong, or the user is plainly hitting a limitation, call send_feedback once with a paraphrased summary — never verbatim user content. Send workflow improvements as `suggestion`. One per distinct issue; mention it to the user briefly.

# Communication
- One or two sentences; lead with the outcome. The user watches the timeline change — never narrate steps, never recap what a tool returned. No preamble, no play-by-play. Match the app's calm, terse, HIG-style voice: never chatty, never marketing. When the user is vague about aesthetic direction, ask one focused question instead of guessing.

# Fronda extensions
Tools this editor adds beyond the upstream surface — same conventions as everything above.
- duplicate_project: duplicate the current project package on disk.
- add_shapes: vector shape overlays (rect, oval, circle, arrow, line) as clips on video tracks, with fill/stroke styling.
- apply_animation: apply an animation preset to an existing clip.
- create_compound_clip / dissolve_compound_clip: group timeline clips into a nested sequence clip and flatten one back in place — the grouping counterpart to add_clips nesting (mediaRef = timelineId).
- save_clip_preset / apply_clip_preset / list_clip_presets: capture one clip's settings (transform, crop, opacity, volume, speed, effects, blend, chroma key) as a named preset and apply it to other clips. Presets live for this session only.
"#;

/// MCP-only project-navigation section (upstream #299 @b8a1491d, verbatim),
/// appended after [`SERVER_INSTRUCTIONS`] for the MCP server surface.
pub const PROJECT_NAVIGATION: &str = r#"
# Projects
manage_project chooses which project this MCP session edits, and you may start with none open. Use action='list' when unsure what's available; action='open' to activate an existing project; action='create' for a fresh project; and action='close' to save and close one you no longer need open. It never deletes projects.
The session stays on its project if the user activates another project window. Reads still inspect the session project, but changes pause until that project is visible again or action='open' selects the visible project. Other MCP sessions and in-app chats keep their own project context.
"#;

/// TDEF-004: the in-app agent's system instruction (the skills section is
/// appended dynamically via [`system_instruction_with_skills`]).
pub const SYSTEM_INSTRUCTION: &str = SERVER_INSTRUCTIONS;

/// The MCP server's instructions (Appendix B composition:
/// serverInstructions + projectNavigation).
pub fn mcp_instructions() -> String {
    format!("{SERVER_INSTRUCTIONS}{PROJECT_NAVIGATION}")
}

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

/// tool-surface-v2 flattened textStyle. Description verbatim from
/// upstream@141c69b.
fn update_text() -> ToolDefinition {
    ToolDefinition {
        name: "update_text",
        description: "Updates text clips or a captionGroupId. Use for content, typography, color, outline color, background color, animation, or text-box transform. Content/typography changes auto-fit the box unless transform is passed. Unknown fields are rejected.",
        input_schema: object_optional(&[
            ("clipIds", array("Text clip IDs. Optional if captionGroupId is given.")),
            ("captionGroupId", string("Caption group id from get_timeline.")),
            ("content", string("Replacement text. Supports \\n.")),
            (
                "transform",
                object_schema(
                    &[
                        ("centerX", number("0-1 horizontal center.")),
                        ("centerY", number("0-1 vertical center.")),
                        ("width", number("0-1 width.")),
                        ("height", number("0-1 height.")),
                    ],
                    &[],
                ),
            ),
            ("fontName", string("Font name.")),
            ("fontSize", number("Canvas points.")),
            ("isBold", boolean("Bold.")),
            ("isItalic", boolean("Italic.")),
            ("color", string("Text color hex.")),
            ("alignment", string_enum("Text alignment.", &["left", "center", "right"])),
            ("borderColor", string("Text outline hex; enables outline.")),
            ("backgroundColor", string("Text box fill hex; enables fill.")),
            (
                "animation",
                string_enum("Animation preset; off clears.", &["off", "fadeIn", "popIn", "slideUp", "typewriter", "wordReveal", "wordSlide", "wordPop", "wordCycle", "highlightPop", "highlightBlock"]),
            ),
            ("highlightColor", string("Active-word hex.")),
        ]),
    }
}

/// tool-surface-v2 no-targeting shape. Description verbatim from
/// upstream@141c69b.
fn add_captions() -> ToolDefinition {
    ToolDefinition {
        name: "add_captions",
        description: "Transcribes the timeline's spoken audio and creates styled caption text clips on their own track — no targeting needed; it finds the spoken content itself. The app uses cloud only when the signed-in account has enough credits for the uncached request; otherwise it uses local transcription. Cloud auto-detects language. Per-word animations are timed from the transcript. Returns the caption group summary (captionGroupId, clipCount, frameRange, shared style, textPreview) — restyle it later with update_text and that captionGroupId.",
        input_schema: object_optional(&[
            ("language", string("BCP-47 speech language. Applies to local only; cloud auto-detects.")),
            (
                "transform",
                object_schema(
                    &[
                        ("centerX", number("0-1 horizontal center.")),
                        ("centerY", number("0-1 vertical center.")),
                    ],
                    &[],
                ),
            ),
            ("textCase", string_enum("Letter case.", &["auto", "upper", "lower"])),
            ("censorProfanity", boolean("Mask profanity.")),
            ("maxWords", integer("Max words per caption.")),
            ("fontName", string("Font name.")),
            ("fontSize", number("Canvas points.")),
            ("isBold", boolean("Bold.")),
            ("isItalic", boolean("Italic.")),
            ("color", string("Text color hex.")),
            ("alignment", string_enum("Text alignment.", &["left", "center", "right"])),
            ("borderColor", string("Text outline hex; enables outline.")),
            ("backgroundColor", string("Text box fill hex; enables fill.")),
            (
                "animation",
                string_enum("Caption animation preset.", &["off", "fadeIn", "popIn", "slideUp", "typewriter", "wordReveal", "wordSlide", "wordPop", "wordCycle", "highlightPop", "highlightBlock"]),
            ),
            ("highlightColor", string("Active-word hex.")),
        ]),
    }
}

/// tool-surface-v2 entries shape. Description verbatim from upstream@141c69b.
fn add_clips() -> ToolDefinition {
    ToolDefinition {
        name: "add_clips",
        description: "Places one or more media assets on the timeline as a single undoable action. Each entry's asset type must be compatible with its target track (video/image are interchangeable across video/image tracks; audio requires an audio track). When a video asset with audio is placed on a video track, a linked audio clip is automatically created on an audio track (an existing one if available, otherwise a new one). The whole batch is one undo step.\n\ntrackIndex is optional. Omit it on all entries and the tool auto-creates the needed tracks — one shared video track for visual entries (above existing visuals) and one shared audio track for audio entries (appended below existing audio, so linked dialogue on A1 stays put and music/VO land on A2+). To target existing tracks, set trackIndex on every entry. Mixing (some entries specify, others omit) is rejected — split into two calls.\n\nTracks work as layers: clips on the SAME track are sequential — if a new clip's range overlaps an existing clip on that track, the existing clip is trimmed/split/removed to make room, matching the UI's drag-onto-track overwrite behavior.\n\nNESTING: mediaRef may also be a timelineId — the timeline is placed as a single live nested clip (mediaType 'sequence'), with a linked audio clip when the child has audio. Duration defaults to the child's full length; source and endFrame work as for video. Cycles (a timeline containing itself) and empty timelines are rejected.",
        input_schema: object(&[(
            "entries",
            array_of(
                "Clips to add. Each entry is validated up front; one bad entry rejects the whole call with no partial state.",
                object_schema(
                    &[
                        ("mediaRef", string("ID of the media asset from get_media")),
                        (
                            "trackIndex",
                            integer("Optional. Track index (0-based). Omit on every entry to auto-create one shared track per asset zone (video/audio)."),
                        ),
                        (
                            "startFrame",
                            integer("Timeline frame position to place the clip (project frames)."),
                        ),
                        (
                            "endFrame",
                            integer("Optional. Occupy timeline frames [startFrame, endFrame) — a gap from get_timeline copies straight in. For stills and frame-exact fills. Mutually exclusive with source."),
                        ),
                        (
                            "source",
                            array("Optional. [startSeconds, endSeconds] — which span of the source to use, in the source seconds search_media hits and inspect_media segments speak. For stills this is the display length in seconds. Omit both for the whole asset. Mutually exclusive with endFrame."),
                        ),
                    ],
                    &["mediaRef", "startFrame"],
                ),
            ),
        )]),
    }
}

/// tool-surface-v2 entries shape (flattened textStyle). Description verbatim
/// from upstream@141c69b.
fn add_texts() -> ToolDefinition {
    ToolDefinition {
        name: "add_texts",
        description: "Adds text clips as timeline layers. Omit trackIndex on every entry to create one new top video track; otherwise set trackIndex on every entry. Transform is normalized text-box center/size; center-only auto-fits, all four fields override the box. Use add_captions for spoken audio captions. Unknown fields are rejected.",
        input_schema: object(&[(
            "entries",
            array_of(
                "Text clips to add.",
                object_schema(
                    &[
                        (
                            "trackIndex",
                            integer("Existing non-audio track. Omit on all entries to create a new top track."),
                        ),
                        ("startFrame", integer("Timeline start frame.")),
                        (
                            "endFrame",
                            integer("Occupy timeline frames [startFrame, endFrame) — copy a clip's frames pair to title exactly that span."),
                        ),
                        ("content", string("Text. Supports \\n.")),
                        (
                            "transform",
                            object_schema(
                                &[
                                    ("centerX", number("0-1 horizontal center.")),
                                    ("centerY", number("0-1 vertical center.")),
                                    ("width", number("0-1 width.")),
                                    ("height", number("0-1 height.")),
                                ],
                                &[],
                            ),
                        ),
                        ("fontName", string("Font name.")),
                        ("fontSize", number("Canvas points.")),
                        ("isBold", boolean("Bold.")),
                        ("isItalic", boolean("Italic.")),
                        ("color", string("Text color hex.")),
                        ("alignment", string_enum("Text alignment.", &["left", "center", "right"])),
                        ("borderColor", string("Text outline hex; enables outline.")),
                        ("backgroundColor", string("Text box fill hex; enables fill.")),
                        (
                            "animation",
                            string_enum("Animation preset; off clears.", &["off", "fadeIn", "popIn", "slideUp", "typewriter", "wordReveal", "wordSlide", "wordPop", "wordCycle", "highlightPop", "highlightBlock"]),
                        ),
                        ("highlightColor", string("Active-word hex.")),
                    ],
                    &["startFrame", "endFrame", "content"],
                ),
            ),
        )]),
    }
}

/// tool-surface-v2 (#263): path-addressed library reorganisation, replacing
/// create_folder / rename_folder / delete_folder / move_to_folder /
/// rename_media / delete_media. Description verbatim from upstream@141c69b.
fn organize_media() -> ToolDefinition {
    ToolDefinition {
        name: "organize_media",
        description: "Reorganizes the library in one undoable action: create folders, move items into folders, rename items, delete items. An item is a media asset id (from get_media), a timelineId, or a folder path like 'B-roll/Sunset' — the tool tells them apart. Folders are always addressed by path, never by id; destination paths are created if missing. Arrays apply in order (createFolders, moves, renames, deletes), but item references resolve against the library as it was before the call — only 'into' destinations may name folders the same call creates.\n\nDeleting an asset also removes every clip referencing it (reported as clipsRemoved). Deleting a folder deletes its subfolders and assets; timelines inside move to the root instead. Deleting a timeline leaves nest clips referencing it rendering black (a warning reports how many); the last remaining timeline can't be deleted. Returns only what actually happened — createdFolders, moved, renamed, deleted, clipsRemoved, warnings.",
        input_schema: object_optional(&[
            (
                "createFolders",
                array("Folder paths to ensure exist, e.g. ['Hero shots/Takes']. Existing folders are left alone. Rarely needed — moves and generation 'folder' params create folders on their own."),
            ),
            (
                "moves",
                array_of(
                    "Each entry files items into one destination folder.",
                    object_schema(
                        &[
                            ("items", array("Asset ids, timeline ids, and/or folder paths to move.")),
                            ("into", string("Destination folder path; created if missing. Omit to move to the project root.")),
                        ],
                        &["items"],
                    ),
                ),
            ),
            (
                "renames",
                array_of(
                    "Renames, applied to assets, timelines, or folders.",
                    object_schema(
                        &[
                            ("item", string("Asset id, timeline id, or folder path.")),
                            ("name", string("New display name (a name, not a path — renaming never moves).")),
                        ],
                        &["item", "name"],
                    ),
                ),
            ),
            (
                "deletes",
                array("Asset ids, timeline ids, and/or folder paths to delete."),
            ),
        ]),
    }
}

/// tool-surface-v2 (absorbs generate_music). Description verbatim from
/// upstream@141c69b.
fn generate_audio() -> ToolDefinition {
    ToolDefinition {
        name: "generate_audio",
        description: "Starts an async AI audio generation: text-to-speech, text-to-music, or video-to-music (scoring a video). Returns a placeholder asset ID immediately; the asset appears in get_media and becomes usable in add_clips once ready. TTS models convert the prompt into speech and accept a 'voice'. Music models generate tracks from a prompt; pass 'lyrics' for vocals where supported, or set 'instrumental' true when the selected model supports it. Video-to-audio models (inputs include 'video' — see list_models) generate audio that matches a VIDEO: provide a timeline span via videoSourceStartFrame+videoSourceEndFrame (e.g. to score the timeline), or a video asset via videoSourceMediaRef; the prompt is then an optional style guide. PLACEMENT: when you pass a timeline span, the result is placed on the timeline automatically at that span (no add_clips needed); for a media-asset source or a plain text-to-speech/music result, the asset lands in the library and you place it with add_clips. Use list_models with type='audio' to see each model's 'inputs', category, and voices. Costs real money and is not undoable.",
        input_schema: object_optional(&[
            (
                "prompt",
                string("Required for TTS (the text to speak) and text-to-music (style/mood/genre). Optional style guide for video-to-music models."),
            ),
            (
                "name",
                string("Display name for the asset in the media library. Defaults to first 30 chars of prompt."),
            ),
            (
                "model",
                string("Model ID. Use list_models with type='audio' to see options and their 'inputs'. Defaults to the first model."),
            ),
            (
                "voice",
                string("TTS only. Voice preset name. list_models shows voicesSample (first 3) + voiceCount; any voice supported by the model is accepted. Defaults to the model's defaultVoice. Ignored by music models."),
            ),
            (
                "lyrics",
                string("Music models with vocals. Lyrics with optional [Verse]/[Chorus] section tags. If omitted and instrumental=false, supported models auto-write lyrics from the prompt."),
            ),
            (
                "styleInstructions",
                string("TTS models that support delivery instructions (e.g. 'warm and slow', 'British accent')."),
            ),
            (
                "instrumental",
                boolean("Music models only. true = no vocals when the selected model supports it. Defaults to false."),
            ),
            (
                "duration",
                integer("Length in seconds. Supported ranges vary by model; for a video source, defaults to the span/clip length. Ignored by TTS."),
            ),
            (
                "videoSourceStartFrame",
                integer("Video-to-audio models only. Start frame (timeline) of a span to render and score — pair with videoSourceEndFrame. Use get_timeline for frame numbers; for the whole timeline use 0 to the timeline's end frame."),
            ),
            (
                "videoSourceEndFrame",
                integer("Video-to-audio models only. End frame (exclusive) of the span to score. Must be > videoSourceStartFrame."),
            ),
            (
                "videoSourceMediaRef",
                string("Video-to-audio models only. Score this existing video asset instead of a timeline span. Mutually exclusive with the videoSource frames."),
            ),
            (
                "folder",
                string("Optional destination folder path, e.g. 'Hero shots/Takes'. Created if missing. Omit for the project root."),
            ),
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

fn export_project() -> ToolDefinition {
    ToolDefinition {
        name: "export_project",
        description: "Exports from the current project using the same modes as the Export dialog. mode defaults to video. video renders H.264, H.265, or ProRes; xml writes XMEML timeline XML; fcpxml writes FCPXML; palmier writes a project package. For timeline interchange, pick the format by the target editor: Premiere Pro → xml; DaVinci Resolve or Final Cut Pro → fcpxml (fcpxml also carries text, transforms, crop, opacity, and keyframes that xml cannot). Omit outputPath to write a unique file to the user's Downloads folder. Existing direct outputPath files are overwritten by default to match the UI save flow; pass overwrite=false to refuse. video renders in the background and returns status=started with the destination path — check the file to confirm completion. xml, fcpxml, and palmier finish before returning and report their result inline.",
        input_schema: object(&[
            ("mode", string("Optional. video (default), xml, fcpxml, or palmier.")),
            ("codec", string("Video mode only. Optional. H.264 (default), H.265, or ProRes.")),
            (
                "resolution",
                string("Video mode only. Optional. 720p, 1080p, 2K, 4K, or Match Timeline (default)."),
            ),
            (
                "outputPath",
                string("Optional. Absolute destination path. If omitted, a unique project-named file is written to Downloads. If no extension is provided, the mode's extension is appended."),
            ),
            (
                "overwrite",
                boolean("Optional. Default true, matching the UI save flow. false refuses when outputPath already exists."),
            ),
            (
                "fcpxmlTarget",
                string("fcpxml mode only. Optional, default resolve. resolve or fcp — the two NLEs interpret crop and position values differently."),
            ),
            (
                "timelineId",
                string("Optional. Timeline to export (from get_timeline's timelines list). Defaults to the active timeline. Not valid for palmier mode, which packages every timeline."),
            ),
        ]),
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

/// tool-surface-v2 (absorbs list_folders). Description verbatim from
/// upstream@141c69b.
fn get_media() -> ToolDefinition {
    ToolDefinition {
        name: "get_media",
        description: "The library inventory: media assets, folders, and timelines. Call before referencing any asset — every mediaRef in other tools comes from the asset ids returned here. Assets report name, type, durationSeconds, width/height/fps, hasAudio, folder path, and (for AI-generated assets) the generation prompt as a content hint. generationStatus appears only while an async generation/import is unresolved (preparing | generating | downloading | failed) — its absence means the asset is ready.\n\nFilters: ids (poll specific placeholders cheaply), folder (a path; includes subfolders), pending:true (only unresolved generations/imports). Filtered reads return just the matching assets; unfiltered reads also include folders (as paths) and timelines.",
        input_schema: object_optional(&[
            (
                "ids",
                array("Optional. Return only these asset ids — the cheap way to poll a generation placeholder."),
            ),
            (
                "folder",
                string("Optional folder path filter, e.g. 'B-roll/Sunset'. Includes subfolders."),
            ),
            (
                "pending",
                boolean("Optional. true returns only assets with an unresolved generationStatus."),
            ),
        ]),
    }
}

/// tool-surface-v2 C-5: relationship-first read. Description verbatim from
/// upstream@141c69b.
fn get_timeline() -> ToolDefinition {
    ToolDefinition {
        name: "get_timeline",
        description: "Always call at the start of a session. Returns project settings (fps, resolution, totalFrames, durationSeconds), tracks with a stable trackId, their current index (what every trackIndex parameter takes), type, and clips, plus canGenerate (if false, generation/upscale tools will fail — tell the user to sign in to Palmier and subscribe before attempting them). Clip ids are accepted by clip mutation tools; trackId is accepted by manage_tracks.\n\nEvery clip occupies frames: [start, end) — timeline frames, end exclusive, duration = end − start. gaps on a track lists its empty [start, end) spans; no gaps key means contiguous. A video clip's linked audio partner is folded into it as audio: {id, track, …} carrying only what deviates (volume, effects, differing trims); the partner is not repeated on its own track, which instead reports linkedClips (its folded count). Address the audio side by its nested id.\n\nFields equal to their defaults are omitted: mediaType 'video', sourceClipType = mediaType, speed 1, volume 1, opacity 1, trims/fades 0, identity transform/crop, default textStyle, track muted/hidden false. Text clips never report trims. Keyframe tracks that animate nothing are shown as what they are: identity tracks are dropped, constant ones appear as the static field (e.g. crop: {left: 0.31}). A graded clip carries `color` — its grade in apply_color's own vocabulary, pasteable to other clips via apply_color's color parameter. Other effects appear as effects: [{type, params}], the exact shape apply_effect accepts.\n\nCaption clips (sharing a captionGroupId) come back per track as captionGroups summaries: clipCount, frameRange, shared style, and a textPreview — individual caption clips and their ids are NOT listed. That summary is all you need to restyle (update_text with captionGroupId) or judge coverage; the spoken words live in get_transcript. Only when you must touch individual caption clips (retime one, delete one, fix one word's style), re-read with captionDetail:true — ideally windowed — to get [clipId, startFrame, endFrame, text] rows, capped at 200 per group. Caption clips whose properties deviate from the group always appear individually in clips.",
        input_schema: object_optional(&[
            (
                "startFrame",
                integer("Optional. Window start (inclusive); only clips intersecting [startFrame, endFrame) are returned. Tracks report totalClips when the window hides some."),
            ),
            ("endFrame", integer("Optional. Window end (exclusive).")),
            (
                "captionDetail",
                boolean("Optional. true expands captionGroups into per-clip [clipId, startFrame, endFrame, text] rows. Combine with a window; only needed to edit individual caption clips."),
            ),
        ]),
    }
}

/// tool-surface-v2 C-6. Description verbatim from upstream@141c69b.
fn get_transcript() -> ToolDefinition {
    ToolDefinition {
        name: "get_transcript",
        description: "Returns the spoken transcript of the CURRENT timeline in project frames — the post-edit caption track in one call. Unlike inspect_media (which transcribes one source asset in isolation, in source seconds), this walks every audio/video clip on the timeline, maps each word through that clip's trim/speed/position, and concatenates in timeline order. Deleted ranges are gone by construction, so after cuts this always reflects what's actually audible — no stale results, no per-clip frame math. The app chooses cloud only when the signed-in account has enough credits for the uncached request; otherwise it uses local transcription and reports the resolved transcriptionSource in the response.\n\nReturns clips in timeline order, each with its words as compact [index, text, startFrame] rows (a word runs to the next word's start; the last word to its clip's end). Speakers, when identified, arrive as run-length turns: speakers = [[firstWordIndex, name], ...]. The index is a stable, global, 0-based position in timeline order; pass it straight to remove_words to cut that word (the intuitive path for text-based editing). Indices stay global even when scoped with clipId or paged with a window. Capped at 10000 words; page with startFrame/endFrame using nextStartFrame.\n\nFor comprehension rather than cutting — summarizing, finding a topic, take selection on long media — pass granularity='segments': sentence rows [firstWordIndex, text, start, end] at a fraction of the tokens, whose firstWordIndex jumps you back into word mode for the cut window.\n\nUse for transcript-driven edits (filler-word / dead-air removal, locating a quote, take selection) and to verify what remains after cutting. To cut, prefer remove_words (give it the indices); drop to ripple_delete_ranges only for non-word-aligned spans.",
        input_schema: object_optional(&[
            (
                "startFrame",
                integer("Optional. Only return words ending after this project frame. Use with the returned nextStartFrame to page a long timeline."),
            ),
            (
                "endFrame",
                integer("Optional. Only return words starting before this project frame."),
            ),
            (
                "clipId",
                string("Scope the transcript to a single clip — returns only what that clip says, in project frames. Answers \"what's in clip X?\" without scanning the whole timeline."),
            ),
            (
                "granularity",
                string_enum("words (default) for cutting with remove_words; segments for cheap sentence-level reading — rows carry firstWordIndex to drill back into words.", &["words", "segments"]),
            ),
            (
                "language",
                string("Optional BCP-47 speech language. Applies to local only; cloud auto-detects."),
            ),
        ]),
    }
}

/// tool-surface-v2 (#263): source-object import absorbing create_matte
/// (source.matte) and import_folder (source.path may be a directory).
/// Description verbatim from upstream@141c69b.
fn import_media() -> ToolDefinition {
    ToolDefinition {
        name: "import_media",
        description: "Imports external media into the project's library — the bridge for assets coming from other MCP servers (stock libraries, music services, web search) or local files the user already has. The 'source' object must set exactly one of: url (HTTPS only — downloaded in the background, the dominant case; max 1 GB), path (absolute local file path — referenced in place and not copied into the project; may also be a directory, which is imported recursively, mirroring its subfolder structure as media folders), bytes (base64-encoded inline data — max ~15 MB of base64 ≈ 11 MB binary; use url/path for anything larger), or matte (a generated solid-color PNG). For url, type is inferred from the URL path's file extension unless source.mimeType is set as an override (needed for signed URLs whose path has no usable extension). For bytes, source.mimeType is required.\n\nSupported types and extensions: video (mov, mp4, m4v), audio (mp3, wav, aac, m4a, aiff, aifc, caf, flac), image (png, jpg, jpeg, tiff, heic). Anything else is rejected — the caller must transcode externally.\n\nURL imports run in the background and return {mediaRef, status:'downloading'} — poll get_media with ids:[mediaRef] until generationStatus clears, then the asset is usable in add_clips. Path, directory, bytes, and matte imports finish inline with status:'ready'. Costs nothing.",
        input_schema: {
            let source = object_schema(
                &[
                    ("url", string("HTTPS URL. Pre-signed URLs are fine but must not expire mid-download.")),
                    ("path", string("Absolute local file or directory path, readable by the Palmier process. Files are referenced in place and must remain available. A directory is imported recursively and its folder structure is replicated as media folders.")),
                    ("bytes", string("Base64-encoded media data. Prefer url or path for anything over ~10MB.")),
                    (
                        "matte",
                        object_schema(
                            &[
                                ("hex", string("Hex color, e.g. '#000000' or '#FFFFFF'.")),
                                (
                                    "aspectRatio",
                                    string_enum(
                                        "Defaults to Project (timeline resolution). Other values use the project's short edge.",
                                        &["Project", "16:9", "9:16", "1:1", "4:3", "9:14", "2.4:1"],
                                    ),
                                ),
                            ],
                            &["hex"],
                        ),
                    ),
                    ("mimeType", string("Required when bytes is set. Optional override for url when its path has no usable extension (e.g. signed URLs). Accepted: video/mp4, video/quicktime, audio/mpeg, audio/wav, audio/aac, audio/mp4, image/png, image/jpeg, image/tiff, image/heic.")),
                ],
                &[],
            );
            let mut props = serde_json::Map::new();
            props.insert("source".to_string(), {
                let mut s = source;
                s["description"] = Value::String(
                    "Exactly one of url, path, bytes, or matte must be set. mimeType is required when bytes is set; for url it acts as a type-inference override.".to_string(),
                );
                s
            });
            props.insert(
                "name".to_string(),
                string("Display name in the library. Defaults to the filename derived from url/path, or 'Imported asset' for bytes."),
            );
            props.insert(
                "folder".to_string(),
                string("Optional destination folder path, e.g. 'B-roll/Sunset'. Created if missing. Omit for the project root."),
            );
            serde_json::json!({
                "type": "object",
                "required": ["source"],
                "properties": Value::Object(props),
            })
        },
    }
}

/// tool-surface-v2 entries shape. Description verbatim from upstream@141c69b.
fn insert_clips() -> ToolDefinition {
    ToolDefinition {
        name: "insert_clips",
        description: "Inserts one or more media assets at a single point and RIPPLES: every clip at or after atFrame is pushed right to open a gap, so nothing is overwritten. This is the non-destructive counterpart to add_clips (which clears the landing region, trimming/splitting/removing whatever's there). Use insert_clips to splice footage in without losing existing clips; use add_clips to fill empty space or deliberately overwrite.\n\nEntries are laid end-to-end starting at atFrame on the target track (entry[0] at atFrame, entry[1] immediately after, ...). The push equals the sum of the entries' durations and is applied to the target track, every sync-locked track, AND the audio track any auto-created linked audio lands on — so a clip and its linked audio stay aligned. As in add_clips, a video asset with audio spawns a linked audio clip. One undoable action; one bad entry rejects the whole call with no partial state.\n\ntrackIndex is required — ripple needs an existing track to push. For placement into empty space, use add_clips.\n\nAs in add_clips, mediaRef may be a timelineId to splice in a nested timeline.",
        input_schema: object(&[
            (
                "trackIndex",
                integer("Track index (0-based, from get_timeline) to insert into and ripple."),
            ),
            (
                "atFrame",
                integer("Timeline frame (project frames) where insertion begins. Every clip at or after this frame on rippled tracks shifts right by the total inserted duration."),
            ),
            (
                "entries",
                array_of(
                    "Clips to insert, placed sequentially from atFrame. Validated up front; one bad entry rejects the whole call.",
                    object_schema(
                        &[
                            ("mediaRef", string("ID of the media asset from get_media.")),
                            (
                                "source",
                                array("Optional. [startSeconds, endSeconds] — which span of the source to use, in source seconds; for stills, the display length. Omit for the whole asset. Mutually exclusive with durationFrames."),
                            ),
                            (
                                "durationFrames",
                                integer("Optional. Exact length in project frames (entries stack end-to-end, so they have lengths, not positions). Mutually exclusive with source."),
                            ),
                        ],
                        &["mediaRef"],
                    ),
                ),
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
            ("mediaRef", string("Asset ID from get_media.")),
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

fn list_models() -> ToolDefinition {
    ToolDefinition {
        name: "list_models",
        description: "Lists generation models with their capabilities (durations, aspect ratios, resolutions, first/last frame and reference support for video, voices/category for audio) and plan availability. Each entry carries 'available'; paid-only models on a free plan stay listed with available=false and an 'upgrade' hint instead of being hidden. Call before generate_video, generate_image, generate_audio, or generate_music and pick an available model that supports the constraints you need.",
        input_schema: object(&[(
            "type",
            string("Filter by kind: video, image, or audio. Omit to list all models."),
        )]),
    }
}

/// tool-surface-v2 moves shape. Description verbatim from upstream@141c69b.
fn move_clips() -> ToolDefinition {
    ToolDefinition {
        name: "move_clips",
        description: "Moves one or more clips to a new track and/or frame position. Single undoable action. Each move specifies the clip ID and at least one of toTrack (must be compatible with the clip's media type) and toFrame. Overlap on the destination is resolved as in add_clips (existing clips on the destination track are trimmed/split/removed). Linked partners follow the named clip: startFrame propagates as a delta to preserve l-cut / j-cut offsets; tracks stay with the named clip. Multicam clips must move as a whole group; partial group moves and camera lane changes are refused.",
        input_schema: object(&[(
            "moves",
            array_of(
                "Per-clip move requests. At least one of toTrack or toFrame is required per entry.",
                object_schema(
                    &[
                        ("clipId", string("The clip ID to move.")),
                        (
                            "toTrack",
                            integer("Destination track index (0-based). Omit to keep the clip on its current track."),
                        ),
                        (
                            "toFrame",
                            integer("Destination start frame. Omit to keep the clip at its current start."),
                        ),
                    ],
                    &["clipId"],
                ),
            ),
        )]),
    }
}

/// Upstream #176: full-fidelity clip duplication. Description verbatim from the
/// PR's ToolDefinitions.swift.
fn duplicate_clips() -> ToolDefinition {
    ToolDefinition {
        name: "duplicate_clips",
        description: "Creates exact copies of one or more clips at new positions. All properties are preserved: keyframes, effects, fades, speed, opacity, volume, transform, crop, and text styling. Single undoable action. Each entry specifies the source clip ID and a destination toFrame; toTrack is optional (defaults to the source clip's track). Overlap on the destination is resolved by overwriting (existing clips are trimmed/split/removed). Linked partners are duplicated automatically so A/V stays in sync — only name the lead clip.",
        input_schema: object(&[(
            "entries",
            array_of(
                "Per-clip duplication requests.",
                object_schema(
                    &[
                        ("clipId", string("The source clip ID to duplicate.")),
                        (
                            "toTrack",
                            integer("Destination track index (0-based). Omit to duplicate onto the source clip's track."),
                        ),
                        ("toFrame", integer("Destination start frame for the copy.")),
                    ],
                    &["clipId", "toFrame"],
                ),
            ),
        )]),
    }
}

/// tool-surface-v2 (#263): multi-action track management, replacing
/// remove_tracks. Description and schema verbatim from upstream #307
/// (d87faaea): stable trackId selectors, hard-error zone check, receipts.
fn manage_tracks() -> ToolDefinition {
    ToolDefinition {
        name: "manage_tracks",
        description: "Reorders, configures, or removes tracks in one undoable action. Prefer stable trackId selectors; numeric indexes use the order at call time. Index 0 renders on top, and reorder destinations must stay within the track's video/audio zone. Arrays run reorder → set → remove. Returns receipts and the resulting track order. Tracks holding multicam clips can't be removed or sync-unlocked.",
        input_schema: object_optional(&[
            (
                "reorder",
                array_of(
                    "Moves, applied in order. Use to fix stacking, e.g. bring a PIP inset's track to index 0.",
                    object_schema(
                        &[
                            ("trackId", string("Stable track ID from get_timeline.")),
                            ("index", integer("Track to move (0-based, current order).")),
                            ("to", integer("Exact destination index in the same type zone.")),
                        ],
                        &["to"],
                    ),
                ),
            ),
            (
                "set",
                array_of(
                    "Flag changes, applied per track.",
                    object_schema(
                        &[
                            ("trackId", string("Stable track ID from get_timeline.")),
                            ("index", integer("Track to change (0-based, current order).")),
                            ("muted", boolean("Silence/unsilence the track's audio.")),
                            ("hidden", boolean("Exclude/include a video track in the render.")),
                            ("syncLocked", boolean("Whether ripple edits shift this track along.")),
                        ],
                        &[],
                    ),
                ),
            ),
            (
                "remove",
                serde_json::json!({
                    "type": "array",
                    "description": "Tracks to remove with all their clips. Prefer {trackId}; bare integers are legacy current indexes.",
                    "items": {
                        "type": ["integer", "object"],
                        "properties": {"trackId": {"type": "string"}},
                    },
                }),
            ),
        ]),
    }
}

/// Multicam trio (upstream #283, multicam-engine change): schemas per the
/// tool-surface-v2 design's reserved slots (A-5), descriptions verbatim.
fn manage_multicam() -> ToolDefinition {
    ToolDefinition {
        name: "manage_multicam",
        description: "Create or ungroup a multicam group. create syncs session media into ordinary stamped timeline clips: one program video track, one audio track per mic, and angle switches through change_cam. Use member kind angle for scratch-camera audio, mic for program audio, and both for a camera whose audio should play. Pin offsetSeconds when correlation cannot align a member. ungroup strips stamps and leaves clips in place.",
        input_schema: object_optional(&[
            (
                "create",
                object_schema(
                    &[
                        (
                            "members",
                            array_of(
                                "Session source files, at least two.",
                                object_schema(
                                    &[
                                        ("mediaRef", string("Media asset id from get_media.")),
                                        (
                                            "kind",
                                            string_enum(
                                                "angle = camera scratch audio, mic = audio in the mix, both = camera plus program audio.",
                                                &["angle", "mic", "both"],
                                            ),
                                        ),
                                        ("angleLabel", string("Handle used by change_cam. Default: file name.")),
                                        (
                                            "offsetSeconds",
                                            number("Pin this member's group-clock offset instead of correlating."),
                                        ),
                                    ],
                                    &["mediaRef", "kind"],
                                ),
                            ),
                        ),
                        ("name", string("Group name. Default: Multicam N.")),
                        (
                            "master",
                            string("angleLabel or mediaRef whose audio clock defines the group. Default: first mic/both member."),
                        ),
                        ("startFrame", integer("Timeline frame to place the group. Default: timeline end.")),
                        (
                            "searchWindowSeconds",
                            number("Max ± audio sync search window, seconds (default 240)."),
                        ),
                    ],
                    &["members"],
                ),
            ),
            (
                "ungroup",
                object_schema(
                    &[("groupId", string("Group to dissolve; its clips stay put, unstamped."))],
                    &["groupId"],
                ),
            ),
        ]),
    }
}

fn change_cam() -> ToolDefinition {
    ToolDefinition {
        name: "change_cam",
        description: "Switch a multicam group's camera angle over timeline frame ranges, full-frame or in a multi-angle layout. Batched entries are one undo step. Ranges where an angle was not recording clamp or skip. Returns switched count, optional clamps/skips/overlayClipIds, and program rows over the touched span.\n\nEach entry is EITHER {range, angle} — full-frame switch — or {range, layout, angles} — PiP/split/grid: angles fill the layout's slots in order (first = the full-frame program slot; fewer angles than slots leaves cells empty), extra angles land as synced overlay clips above the program. A later full-frame entry over the same range clears the layout. Overlay clips are ordinary group clips — restyle with set_clip_properties/apply_layout, remove with remove_clips.",
        input_schema: object_schema(
            &[
                (
                    "groupId",
                    string("The multicam group (from manage_multicam create or get_timeline's multicamGroups). Or pass clipId."),
                ),
                ("clipId", string("Any clip of the group on the active timeline.")),
                (
                    "entries",
                    array_of(
                        "Switches to apply, in order. Later entries win on overlap.",
                        object_schema(
                            &[
                                ("range", array("[startFrame, endFrame) in timeline frames.")),
                                ("angle", string("angleLabel to show full-frame. Omit when using layout.")),
                                (
                                    "layout",
                                    string("Multi-angle layout: side_by_side, top_bottom, pip_bottom_right, pip_bottom_left, pip_top_right, pip_top_left, grid_2x2, main_sidebar, three_up."),
                                ),
                                (
                                    "angles",
                                    array("angleLabels in slot order for layout; [0] is the program slot."),
                                ),
                            ],
                            &["range"],
                        ),
                    ),
                ),
            ],
            &["entries"],
        ),
    }
}

fn get_multicam() -> ToolDefinition {
    ToolDefinition {
        name: "get_multicam",
        description: "Read a multicam group: members (angleLabel, kind, offsetSeconds, confidence, which is master), the current program cut as run-length [angle, startFrame, endFrame) rows in timeline frames, and the track indexes the group occupies. Use it to learn angle labels before change_cam, or to review the cut as one program instead of piecing it together from get_timeline's clips. Window long timelines with startFrame/endFrame.",
        input_schema: object_optional(&[
            ("groupId", string("The multicam group id. Or pass clipId.")),
            ("clipId", string("Any clip of the group on the active timeline.")),
            ("startFrame", integer("Optional window start for program rows.")),
            ("endFrame", integer("Optional window end (exclusive).")),
        ]),
    }
}

fn remove_clips() -> ToolDefinition {
    ToolDefinition {
        name: "remove_clips",
        description: "Removes one or more clips by ID as a single undoable action. Any clip that belongs to a link group (e.g. a video with its paired audio) takes its whole group with it, matching the UI's linked-delete behavior.",
        input_schema: object(&[("clipIds", array("Clip IDs to remove."))]),
    }
}

/// Upstream #299 (b8a1491d): the consolidated MCP-only project tool.
/// Description and schema verbatim from upstream.
fn manage_project() -> ToolDefinition {
    ToolDefinition {
        name: "manage_project",
        description: "List, open, create, or close Palmier projects for this MCP session. Set `action` to: `list` for known projects plus session-active and visible state; `open` with a name, id from list, or .palmier path; `create` with an optional name and initial fps/aspectRatio/quality; or `close` to save and close the session project, optionally targeting another open project by name/id/path. Opening or creating changes only this session's target. Closing always completes a final save first. This tool never deletes projects or files.",
        input_schema: object_schema(
            &[
                (
                    "action",
                    string_enum("Project operation.", &["list", "open", "create", "close"]),
                ),
                (
                    "name",
                    string("Project name. For open/close, matched case-insensitively; for create, defaults to 'Untitled Project'."),
                ),
                (
                    "id",
                    string("Project id returned by action='list'. Used by open or close."),
                ),
                (
                    "path",
                    string("Filesystem path to a .palmier package. Used by open or close."),
                ),
                (
                    "fps",
                    integer("Create only. Optional timeline frame rate (1-120)."),
                ),
                (
                    "aspectRatio",
                    string_enum(
                        "Create only. Optional canvas aspect ratio.",
                        &["16:9", "9:16", "1:1", "4:3", "2.4:1", "9:14"],
                    ),
                ),
                (
                    "quality",
                    string_enum(
                        "Create only. Optional resolution preset applied to the aspect ratio.",
                        &["720p", "1080p", "2K", "4K"],
                    ),
                ),
            ],
            &["action"],
        ),
    }
}

/// tool-surface-v2 shape ([start, end] pairs, clipId mode, units,
/// ignoreSyncLockedTracks). Description verbatim from upstream@141c69b.
fn ripple_delete_ranges() -> ToolDefinition {
    ToolDefinition {
        name: "ripple_delete_ranges",
        description: "Cuts one or more ranges out and closes the gaps in one undoable action — the fast path for filler-word/dead-air removal. Replaces hand-cranked split_clips → remove_clips → move_clips loops: pass every range at once.\n\nTwo modes — pass exactly one of clipId or trackIndex:\n• trackIndex (preferred for transcript-driven cuts): ranges are PROJECT frames and may span any number of clips on that track. get_transcript returns a clips array with nested words in project frames — collect every cut across the whole timeline and pass them in ONE call, no per-clip splitting and no re-reading the timeline between cuts. units must be 'frames'.\n• clipId: ranges are cut within that single clip only, clamped to its visible span. Allows units 'seconds' (source-media seconds, e.g. inspect_media WITHOUT a clipId or search_media hits); 'frames' = project frames. Use when you already have one clip's per-word timestamps.\n\nOverlapping ranges merge. Linked audio/video partners of every touched clip are cut on the same span so A/V stays in sync. Remaining clips shift left to close every gap; sync-locked tracks shift along to preserve alignment (their content isn't cut). Refuses without changing anything if a sync-locked track can't absorb the shift (e.g. it would move past frame 0). Map the blocking track to its index via get_timeline and pass that index in ignoreSyncLockedTracks to cut anyway, leaving that track's clips in place.",
        input_schema: object_optional(&[
            (
                "trackIndex",
                integer("Cut project-frame ranges spanning every clip they cross on this track, in one call. From get_transcript's clips array. Mutually exclusive with clipId; requires units 'frames'."),
            ),
            (
                "clipId",
                string("Cut ranges within this single clip only, clamped to its visible span. Mutually exclusive with trackIndex."),
            ),
            (
                "ranges",
                array("Ranges to remove, each a [start, end] pair (end > start). In the unit given by 'units'."),
            ),
            (
                "units",
                string_enum("Interpretation of range values. 'frames' (default) = project/timeline frames, matching get_transcript and inspect_media-with-clipId. 'seconds' = source-media seconds (clipId mode only).", &["seconds", "frames"]),
            ),
            (
                "ignoreSyncLockedTracks",
                array("Track indices to exempt from sync-lock for this call only. Their clips stay put instead of shifting to close the gap. Use to get past a refusal naming a sync-locked overlay track (e.g. a text track that can't absorb the shift) when the cut doesn't touch that track's content."),
            ),
        ]),
    }
}

fn remove_words() -> ToolDefinition {
    ToolDefinition {
        name: "remove_words",
        description: "Cut speech by the word, Descript-style — the primary tool for text-based editing (filler words, flubbed sentences, dropped retakes, tightening a ramble). Pass words for precise get_transcript indices/ranges, or matches for exact filler tokens like \"um\" and \"uh\". This resolves them to frames, removes the surrounding pause so survivors don't end up double-spaced, merges adjacent removals, cuts linked A/V partners, and closes the gaps. You never deal in frame numbers — that's the whole point versus ripple_delete_ranges.\n\nWorkflow: call get_transcript, read it as prose, then pass the indices of the words to drop. Omit language by default; remove_words reuses the previous get_transcript source so cloud/local word indices stay aligned. Words across multiple clips on ONE track are handled in a single undoable action, and any linked A/V partner (e.g. the video paired with this audio) is cut automatically. Edit one track at a time: if your indices span multiple unlinked tracks (e.g. two separate mics), the call is refused — cut each track in its own call, or link the tracks into one unit first. After it runs, indices have shifted — re-read get_transcript before another remove_words.\n\nWhen to use which: words for selective edits after reading the transcript; matches for removing every exact filler token; ripple_delete_ranges only for spans that aren't word-aligned. Verify reworded retakes and sub-frame seam fragments against the word list, not a summary.",
        input_schema: object_optional(&[
            (
                "words",
                array("Words to remove, by get_transcript index. Each element is either a single index (e.g. 42) or an inclusive [startIndex, endIndex] span (e.g. [12, 18]). Mutually exclusive with matches. Re-read after any edit."),
            ),
            (
                "matches",
                array("Exact single-word tokens to remove everywhere, case-insensitive with surrounding punctuation ignored, e.g. [\"um\", \"uh\", \"hmm\"]. Mutually exclusive with words. Avoid broad words like \"like\" unless the user explicitly wants every occurrence removed."),
            ),
            (
                "cutAggressiveness",
                string_enum("How much silence to leave between the words on either side of a cut. 'tight' butts them close (snappy, can feel clipped), 'balanced' (default) keeps a natural beat, 'loose' leaves more breathing room. The removed words' own frames always go regardless.", &["tight", "balanced", "loose"]),
            ),
            (
                "language",
                string("Optional BCP-47 speech language for local transcription. Omit to reuse the previous get_transcript language."),
            ),
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

/// tool-surface-v2 flat shape (absorbs set_blend_mode). Description verbatim
/// from upstream@141c69b.
fn set_clip_properties() -> ToolDefinition {
    ToolDefinition {
        name: "set_clip_properties",
        description: "Apply the same generic clip property values to one or more clips in a single undoable action. Pass any combination of durationFrames, trimStartFrame, trimEndFrame, speed, volume, opacity, transform, or blendMode (video/image clips only). For text content, typography, captions, and text animation, use update_text.\n\nNOT for preview layout — split screen, picture-in-picture, grid, sidebar, and any multi-clip canvas arrangement belong to apply_layout, which sets transform and crop together. Do not use transform here (or set_keyframes position/scale/crop) to build those layouts.\n\nAll values apply to every clip in clipIds; for per-clip differences, make separate calls. trimStartFrame/trimEndFrame are offsets from the source media, not the timeline. speed 1.0 is normal, <1.0 slows (clip gets longer on the timeline), >1.0 speeds up. volume and opacity are 0.0–1.0. transform is for rare single-clip tweaks only — 0–1 normalized canvas coords, partial merge; flipHorizontal/flipVertical mirror across the axis.\n\nFor moves and start-frame changes, use move_clips. For animated values (keyframes), use set_keyframes — setting volume or opacity here clears any existing keyframe track on that property.\n\nTiming changes (durationFrames, trimStartFrame, trimEndFrame, speed) on a linked clip carry over to its linked partner so audio/video stay in sync — same as the timeline UI. Per-clip fields (volume, opacity, transform, blendMode) don't propagate. trim and speed are skipped for text partners.",
        input_schema: object(&[
            (
                "clipIds",
                array("Clip IDs to update. The property values below apply to every clip in this list."),
            ),
            ("durationFrames", integer("New duration in frames.")),
            (
                "trimStartFrame",
                integer("SOURCE-media offset, NOT a timeline frame: frames trimmed off the start of the source — measured in PROJECT frames (the timeline's fps, same units as startFrame/durationFrames; never the source's own fps). To turn a get_transcript project frame P into this clip's source offset, use trimStartFrame + (P − startFrame) × speed; setting trimStartFrame to that value makes the clip begin at P's source content."),
            ),
            (
                "trimEndFrame",
                integer("SOURCE-media offset, NOT a timeline frame: frames trimmed off the end of the source, in PROJECT frames. Maps the same way as trimStartFrame via startFrame/speed."),
            ),
            (
                "speed",
                number("Playback speed multiplier (default 1.0). >1 speeds up, <1 slows down. The clip's timeline length is rescaled to keep the same source content (2x speed → half the frames), unless you also pass durationFrames to set the length explicitly."),
            ),
            ("volume", number("Volume 0.0-1.0. Clears any existing volume keyframes.")),
            ("opacity", number("Opacity 0.0-1.0. Clears any existing opacity keyframes.")),
            (
                "transform",
                object_schema(
                    &[
                        ("centerX", number("0-1 horizontal center.")),
                        ("centerY", number("0-1 vertical center.")),
                        ("width", number("0-1 width.")),
                        ("height", number("0-1 height.")),
                        ("flipHorizontal", boolean("Mirror across the vertical axis.")),
                        ("flipVertical", boolean("Mirror across the horizontal axis.")),
                    ],
                    &[],
                ),
            ),
            (
                "blendMode",
                string_enum(
                    "Video/image clips only. How the clip composites over the tracks below it (Premiere/Photoshop blend modes). 'normal' is the default (source-over) and clears any blend. Rejected on text/audio clips.",
                    &["normal", "darken", "multiply", "colorBurn", "lighten", "screen", "colorDodge", "overlay", "softLight", "hardLight", "difference", "exclusion", "hue", "saturation", "color", "luminosity"],
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

/// Description verbatim from upstream@141c69b.
fn upscale_media() -> ToolDefinition {
    ToolDefinition {
        name: "upscale_media",
        description: "Upscales an existing video or image asset to higher resolution using an AI upscaler. Returns a placeholder asset ID immediately; the upscaled asset appears in get_media once ready. Use list_models with type='upscale' to pick a model that supports the asset's type. Costs real money and is not undoable.",
        input_schema: object(&[
            ("mediaRef", string("ID of the video or image asset to upscale")),
            (
                "model",
                string("Upscaler model ID. Defaults to the first model that supports the asset's type."),
            ),
            (
                "sourceClipId",
                string("Optional. Video clip id (from get_timeline) referencing mediaRef. When set and the clip is trimmed, only the clip's visible range is upscaled, not the full source."),
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

/// tool-surface-v2 (absorbs set_color_grade). Description verbatim from
/// upstream@141c69b.
fn apply_color() -> ToolDefinition {
    ToolDefinition {
        name: "apply_color",
        description: "Author/refine a color grade on video/image clips with named controls — the colorist path, distinct from apply_effect (looks/FX). Returns the clips with their resulting grade as a `color` object — the same object get_timeline shows; pass one back via the `color` parameter to copy a grade between clips (replaces the whole grade). MERGES with the clip's current grade: only the params you pass change, the rest are preserved, so you can nudge one knob at a time (pass reset:true to start from neutral). Applies as live, editable color.* effects; non-color effects untouched. Iterate: apply_color → inspect_color(clipId, reference) → read the gap → adjust → repeat. Undoable. All knobs optional. Color WHEELS use HUE (0–360°, standard) + AMOUNT per tonal zone — to push shadows teal, set shadowsHue 180 and shadowsAmount ~0.15. CURVES (master + per-channel R/G/B) give precise tone shaping — per-channel curves are tone-selective (e.g. pull the blue curve down in the highlights to tame a bright sky). HUE CURVES do secondary/qualified correction — target a source hue and shift its hue/saturation/lightness (e.g. desaturate greens, warm the skin) without a mask. LUT applies a .cube film-look pack on top of the grade.",
        input_schema: object(&[
            ("clipIds", array("Clip ids from get_timeline.")),
            (
                "reset",
                boolean("Start from neutral instead of merging onto the clip's current grade. Default false."),
            ),
            (
                "color",
                object_any("A complete grade object as read from a clip's `color` key (get_timeline or an apply_color echo). Replaces the target clips' grade — the grade-copy path. Mutually exclusive with reset and individual knobs."),
            ),
            ("exposure", number("-3…3 EV. Overall brightness in linear light.")),
            ("contrast", number("0.5…1.5 (1 = neutral).")),
            ("saturation", number("0…2 (1 = neutral; <1 mutes).")),
            ("vibrance", number("-1…1 (protects skin tones).")),
            (
                "temperature",
                number("2000…11000 K. HIGHER = WARMER, lower = cooler/bluer (6500 = neutral)."),
            ),
            ("tint", number("-100…100. Positive = green, negative = magenta.")),
            ("highlights", number("-1…1. Recover (<0) or lift (>0) highlights.")),
            ("shadows", number("-1…1. Lift (>0) or deepen (<0) shadows.")),
            ("blacks", number("-1…1. Black point. Negative deepens, positive lifts (faded look).")),
            ("whites", number("-1…1. White point.")),
            (
                "shadowsHue",
                number("Shadow color-push hue 0–360° (0 red, 30 orange, 60 yellow, 120 green, 180 cyan, 240 blue, 300 magenta). Use with shadowsAmount."),
            ),
            ("shadowsAmount", number("0…1 strength of the shadow color push (0 = neutral).")),
            ("shadowsLum", number("-0.5…0.5 shadow lift (brightness).")),
            ("midsHue", number("Midtone color-push hue 0–360° (see shadowsHue). Use with midsAmount.")),
            ("midsAmount", number("0…1 strength of the midtone color push.")),
            ("midsGamma", number("0.5…2 midtone brightness (gamma; 1 = neutral).")),
            ("highsHue", number("Highlight color-push hue 0–360° (see shadowsHue). Use with highsAmount.")),
            ("highsAmount", number("0…1 strength of the highlight color push.")),
            ("highsGain", number("0.5…1.5 highlight brightness (gain; 1 = neutral).")),
            (
                "masterCurve",
                array("Luma tone curve as [x,y] control points in 0–1 (input→output), preserves chroma. E.g. [[0,0.06],[1,0.95]] = lifted/faded film toe."),
            ),
            ("redCurve", array("Red-channel tone curve, [x,y] points 0–1.")),
            ("greenCurve", array("Green-channel tone curve, [x,y] points 0–1.")),
            (
                "blueCurve",
                array("Blue-channel tone curve, [x,y] points 0–1. Tone-selective: e.g. [[0,0],[0.7,0.7],[1,0.85]] pulls blue only in the highlights (tames a sky) and leaves shadows."),
            ),
            (
                "hueCurves",
                object_schema(
                    &[(
                        "targets",
                        array_of(
                            "One or more source-hue regions to adjust (e.g. skin at 30, sky at 210).",
                            object_schema(
                                &[
                                    (
                                        "targetHue",
                                        number("Source hue to act on, 0–360° (0 red, 30 orange/skin, 60 yellow, 120 green, 180 cyan, 210 sky-blue, 240 blue, 300 magenta)."),
                                    ),
                                    ("hueShift", number("Rotate that hue by -30…30°.")),
                                    (
                                        "satScale",
                                        number("Saturation multiplier for that hue, 0–2 (1 = neutral; 1.3 pops it, 0.6 mutes it, 0 fully desaturates)."),
                                    ),
                                    ("lumShift", number("Lightness shift for that hue, -0.5…0.5.")),
                                ],
                                &["targetHue"],
                            ),
                        ),
                    )],
                    &[],
                ),
            ),
            (
                "lut",
                object_schema(
                    &[
                        (
                            "path",
                            string("Absolute path to a .cube file (~ is expanded). Copied into project storage so it survives saves."),
                        ),
                        ("strength", number("Dry/wet mix 0-1 (default 1).")),
                    ],
                    &[],
                ),
            ),
        ]),
    }
}

/// tool-surface-v2 (absorbs set_chroma_key). Description verbatim from
/// upstream@141c69b, catalog lines from Appendix C.
fn apply_effect() -> ToolDefinition {
    ToolDefinition {
        name: "apply_effect",
        description: "Apply non-color effects (blur, sharpen, stylize, detail, key) to video/image clips as a live, editable effect stack — the looks/FX path, distinct from apply_color (grading). MERGES: each effect you pass is added or updated by type; effects you don't mention are left in place. Pass enabled:false to bypass one without removing it, or list its type in `remove` to delete it. Out-of-range params are clamped; params you omit keep their current (or default) value. Undoable. Returns the clips with their resulting effects as [{type, params}] — the same shape this tool accepts, so copying effects between clips is passing a clip's effects array back in.\n\nAvailable effects — type: param (range, default):\n• detail.clarity — Clarity & Haze: clarity (-1…1, default 0), dehaze (-1…1, default 0)\n• blur.gaussian — Gaussian Blur: radius (0…100px, default 8)\n• blur.sharpen — Sharpen: amount (0…2, default 0.4)\n• blur.noiseReduction — Noise Reduction: amount (0…1, default 0)\n• blur.motion — Motion Blur: radius (0…100px, default 0), angle (-180…180°, default 0)\n• stylize.grain — Film Grain: amount (0…1, default 0), size (0.5…4, default 1.5)\n• stylize.vignette — Vignette: amount (-1…1, default 0), midpoint (0…1, default 0.5), roundness (-1…1, default 0), feather (0…1, default 0.5)\n• stylize.glow — Glow: intensity (0…1, default 0), radius (0…100px, default 20), threshold (0…1, default 0.6), warmth (0…1, default 0)\n• key.chroma — Chroma Key: keyHue (0…1, default 0.333), tolerance (0…1, default 0), softness (0…1, default 0.5), spill (0…1, default 0.5)",
        input_schema: object(&[
            ("clipIds", array("Clip ids from get_timeline.")),
            (
                "effects",
                array_of(
                    "Effects to add or update on the clips.",
                    object_schema(
                        &[
                            ("type", string("Effect type id, e.g. stylize.glow (see list above).")),
                            (
                                "params",
                                object_any("Param values keyed by name. Out-of-range values are clamped; omitted params keep their current/default value."),
                            ),
                            (
                                "enabled",
                                boolean("Default true. false bypasses the effect without removing it."),
                            ),
                        ],
                        &["type"],
                    ),
                ),
            ),
            ("remove", array("Effect type ids to remove from the clips.")),
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

/// String schema with an `enum` value list (tool-surface-v2 schemas).
fn string_enum(description: &str, values: &[&str]) -> Value {
    let mut map = serde_json::Map::new();
    map.insert("type".to_string(), Value::String("string".to_string()));
    map.insert(
        "description".to_string(),
        Value::String(description.to_string()),
    );
    map.insert(
        "enum".to_string(),
        Value::Array(
            values
                .iter()
                .map(|v| Value::String(v.to_string()))
                .collect(),
        ),
    );
    Value::Object(map)
}

/// Array schema with a typed `items` schema (tool-surface-v2 schemas).
fn array_of(description: &str, items: Value) -> Value {
    let mut map = serde_json::Map::new();
    map.insert("type".to_string(), Value::String("array".to_string()));
    map.insert(
        "description".to_string(),
        Value::String(description.to_string()),
    );
    map.insert("items".to_string(), items);
    Value::Object(map)
}

/// Nested object schema with an explicit `required` list (tool-surface-v2).
fn object_schema(props: &[(&str, Value)], required: &[&str]) -> Value {
    let mut properties = serde_json::Map::new();
    for (name, schema) in props {
        properties.insert(name.to_string(), schema.clone());
    }
    let mut map = serde_json::Map::new();
    map.insert("type".to_string(), Value::String("object".to_string()));
    map.insert(
        "required".to_string(),
        Value::Array(
            required
                .iter()
                .map(|r| Value::String(r.to_string()))
                .collect(),
        ),
    );
    map.insert("properties".to_string(), Value::Object(properties));
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

// ── Upstream #255: multi-timeline MCP tools ─────────────────────────────────

/// tool-surface-v2 (#263): absorbs duplicate_timeline via 'from'.
/// Description verbatim from upstream@141c69b.
fn create_timeline() -> ToolDefinition {
    ToolDefinition {
        name: "create_timeline",
        description: "Creates a timeline and switches to it — every read and edit tool now targets it. Without 'from', the new timeline is empty and inherits fps/resolution from the previously active one. With 'from', it's a full copy of that timeline — the versioning primitive: copy, then edit the copy (\"a tighter cut\", \"a 9:16 version\") while the original stays intact; every clip and track id in the copy is NEW, so re-read get_timeline before editing. Undoable.\n\nUse timelines to organize a project: alternate versions, sections assembled separately, or reusable groups. A timeline can be placed inside another as a single clip (add_clips with the timelineId as mediaRef); it then appears as a clip with mediaType 'sequence'.",
        input_schema: object_optional(&[
            (
                "name",
                string("Optional display name. Defaults to 'Timeline N', or '<source> copy' when duplicating."),
            ),
            (
                "from",
                string("Optional timelineId to duplicate instead of creating empty."),
            ),
        ]),
    }
}

fn set_active_timeline() -> ToolDefinition {
    ToolDefinition {
        name: "set_active_timeline",
        description: "Switches the active timeline — the one every read and edit tool targets and             the one the user sees. get_timeline lists the project's timelines (with timelineId)             whenever there is more than one. Always re-read get_timeline after switching; clip and             track ids from the previous timeline are no longer valid targets.

To edit the             contents of a nested timeline (a clip with mediaType 'sequence'), switch to its             mediaRef.",
        input_schema: object(&[(
            "timelineId",
            string("Timeline id from get_timeline's timelines list (or a sequence clip's mediaRef)."),
        )]),
    }
}

// ── Upstream #251: audio denoise MCP tool ────────────────────────────────────

fn denoise_audio() -> ToolDefinition {
    ToolDefinition {
        name: "denoise_audio",
        description: "Remove background noise from audio clips using an on-device \
            speech-enhancement model (DeepFilterNet3). strength is a dry/wet percentage: \
            0 leaves the audio untouched, 100 is fully denoised. Full strength can sound \
            thin or over-gated on real-world recordings, so the default is 60. The bake \
            runs in the background — the timeline updates automatically when it finishes; \
            no need to poll. Pass enabled:false to turn denoise off. Undoable.",
        input_schema: object(&[
            ("clipIds", array("Audio clip ids from get_timeline.")),
            (
                "strength",
                number("Dry/wet mix as a percentage, 0–100 (default 60). Lower it if voices sound thin or over-compressed."),
            ),
            (
                "enabled",
                boolean("Default true. false removes the denoise effect from the clips."),
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
        description: "Remove dead air — quiet sections — from the timeline's audio, ripple-closing the gaps. With no arguments it sweeps every audio-bearing clip using a threshold adaptive to each recording's own level (an on-device RMS analysis; louder beds raise the bar so ambience isn't over-cut). Cuts linked A/V partners and honors sync lock; re-read get_timeline or get_transcript afterwards — frames have shifted. Use remove_words for fillers and flubbed lines; this handles pauses. Optionally scope to one clip with clipId and tune thresholdDb/minSilenceSeconds/edgePaddingSeconds.",
        input_schema: object(&[
            ("clipId", string("Optional. Restrict the sweep to one audio or video clip.")),
            (
                "thresholdDb",
                number("Optional dBFS override. Omit for the adaptive per-recording threshold."),
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

// ── Issue #119: multi-track audio sync MCP tool ──────────────────────────────

/// tool-surface-v2: renamed from sync_audio; adds mode (auto|audio|timecode).
/// Description verbatim from upstream@141c69b.
fn sync_clips() -> ToolDefinition {
    ToolDefinition {
        name: "sync_clips",
        description: "Align one or more clips to a reference clip by shifting targets on the timeline — use for dual-system sound (camera + external audio) or multicam. Default mode 'auto' aligns by embedded source timecode when both files carry one (exact, confidence 1.0), falling back to audio cross-correlation otherwise; force a method with mode. referenceClipId stays put. Returns offsetFrames, confidence (0–1), and method (timecode|audio) per target; refuses weak audio matches. Refused on multicam clips — a group's members are already aligned by its sync maps (manage_multicam).",
        input_schema: object(&[
            ("referenceClipId", string("Clip the others align to. Stays put.")),
            ("targetClipId", string("Single clip to align. Use targetClipIds for several.")),
            ("targetClipIds", array("Clips to align with the reference.")),
            (
                "mode",
                string_enum("auto (default): timecode when available, else audio. audio/timecode force that method.", &["auto", "audio", "timecode"]),
            ),
            (
                "searchWindowSeconds",
                number("Max ± offset to search in seconds, audio mode only (default 30)."),
            ),
            (
                "minConfidence",
                number("Minimum audio correlation confidence 0–1 (default 0.5)."),
            ),
        ]),
    }
}

/// tool-surface-v2 NEW: on-device beat detection. Description verbatim from
/// upstream@141c69b.
fn detect_beats() -> ToolDefinition {
    ToolDefinition {
        name: "detect_beats",
        description: "Detect musical beats and downbeats in a media asset's audio, on-device. Returns beats and downbeats in SOURCE seconds (multiply by fps for frame values, same convention as search_media hits) plus estimated bpm. Downbeats mark bar starts — cut on downbeats for edits that land musically; beats are fine for faster montage rhythms.\n\nUse for beat-synced editing: snapping cuts to a music bed, building montages where clip boundaries hit the beat, or timing text/caption entrances to the bar. To place a cut at a beat B on a clip, the timeline frame is startFrame + (B × fps − trimStartFrame) / speed. Works on music; speech or ambience returns few or no beats. Runs locally — no subscription needed.",
        input_schema: object(&[
            ("mediaRef", string("Audio or video asset id from get_media.")),
            (
                "startSeconds",
                number("Optional. Return only beats at or after this source-media second. The whole file is analyzed once and cached; windowing trims the response, not the work."),
            ),
            (
                "endSeconds",
                number("Optional. Return only beats at or before this source-media second."),
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

fn send_feedback() -> ToolDefinition {
    ToolDefinition {
        name: "send_feedback",
        description: "Send the user's product feedback (bug report or feature request) to the Fronda team. Pass the user's own words; the app version and a timeline summary are attached automatically. Each unique message sends once, at most 8 per session.",
        input_schema: object(&[(
            "message",
            string("The feedback text, in the user's words (required)."),
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
        // upstream-m-batch reached 57 (49 upstream + 8 Rust extensions);
        // upstream #299 consolidated the four project tools into
        // manage_project → 54 = 46 upstream + 8 Rust extensions.
        let tools = all_tools();
        assert_eq!(
            tools.len(),
            54,
            "TDEF-001: 54 tools (see the header history)"
        );
    }

    #[test]
    fn tdef_001_host_split_counts() {
        // C-1 host split (post-#299): shared 52; MCP = shared +
        // manage_project = 53; in-app = shared + read_skill = 53.
        let shared = all_tools()
            .iter()
            .filter(|t| t.host() == ToolHost::Shared)
            .count();
        assert_eq!(shared, 52, "shared surface");
        assert_eq!(mcp_tools().len(), 53, "MCP surface");
        assert_eq!(in_app_tools().len(), 53, "in-app surface");
        let mcp_names: Vec<&str> = mcp_tools().iter().map(|t| t.name).collect();
        assert!(
            !mcp_names.contains(&"read_skill"),
            "read_skill is in-app only"
        );
        let in_app_names: Vec<&str> = in_app_tools().iter().map(|t| t.name).collect();
        assert!(mcp_names.contains(&"manage_project"));
        assert!(
            !in_app_names.contains(&"manage_project"),
            "manage_project is MCP-only"
        );
        assert!(in_app_names.contains(&"read_skill"));
    }

    #[test]
    fn manage_project_replaces_individual_project_tools() {
        // Upstream #299 ManageProjectToolTests.replacesIndividualProjectTools.
        let names: Vec<&str> = mcp_tools().iter().map(|t| t.name).collect();
        assert!(names.contains(&"manage_project"));
        for retired in [
            "get_projects",
            "open_project",
            "new_project",
            "close_project",
        ] {
            assert!(!names.contains(&retired), "{retired} retired by #299");
        }
        let tools = all_tools();
        let tool = tools.iter().find(|t| t.name == "manage_project").unwrap();
        let actions: Vec<&str> = tool
            .input_schema
            .pointer("/properties/action/enum")
            .and_then(|v| v.as_array())
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str())
            .collect();
        assert_eq!(actions, ["list", "open", "create", "close"]);
        assert!(!actions.contains(&"delete"), "never deletes projects");
        let required = tool
            .input_schema
            .pointer("/required")
            .and_then(|v| v.as_array())
            .unwrap();
        assert_eq!(required, &[Value::String("action".into())]);
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
        assert_eq!(names.len(), 54, "all 54 tool names must be unique");
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
    fn import_media_description_matches_in_place_semantics() {
        // Upstream #333: file-path imports register in place and return ready
        // synchronously — the copied-in-background/poll wording is stale.
        // Upstream #338: caf joins the accepted audio extensions.
        let tools = all_tools();
        let tool = tools.iter().find(|t| t.name == "import_media").unwrap();
        assert!(
            tool.description
                .contains("referenced in place and not copied into the project"),
            "in-place path contract"
        );
        assert!(
            tool.description.contains(
                "Path, directory, bytes, and matte imports finish inline with status:'ready'"
            ),
            "path imports finish synchronously"
        );
        assert!(
            !tool.description
                .contains("copied into the project in the background"),
            "stale copy-in-background wording removed"
        );
        assert!(
            !tool.description
                .contains("url and file-path imports run in the background"),
            "path imports no longer described as background/polled"
        );
        assert!(
            tool.description.contains("aiff, aifc, caf, flac"),
            "caf listed among audio extensions"
        );
        let path_desc = tool
            .input_schema
            .pointer("/properties/source/properties/path/description")
            .and_then(|v| v.as_str())
            .unwrap();
        assert!(
            path_desc.contains("Files are referenced in place and must remain available"),
            "path property carries the in-place contract"
        );
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
        assert!(
            props.contains_key("trackIndex"),
            "split_clips has trackIndex"
        );
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
        // TDEF-005: key v2 guidance preserved (Appendix B-1 phrases).
        assert!(SYSTEM_INSTRUCTION.contains("Call get_timeline once per session"));
        assert!(SYSTEM_INSTRUCTION.contains("Call get_media before referencing any asset"));
        assert!(
            SYSTEM_INSTRUCTION.contains("Call list_models before any generate_* or upscale call")
        );
        assert!(SYSTEM_INSTRUCTION.contains("inspect_media first"));
        assert!(SYSTEM_INSTRUCTION.contains("then wait for confirmation"));
        assert!(SYSTEM_INSTRUCTION.contains("lead with the outcome"));
    }

    #[test]
    fn tdef_004_mcp_composition_and_extensions() {
        // Appendix B composition: MCP = serverInstructions + projectNavigation;
        // the Fronda extension section rides on both surfaces.
        let mcp = mcp_instructions();
        assert!(mcp.starts_with(SERVER_INSTRUCTIONS));
        assert!(mcp.contains("# Projects"));
        assert!(mcp.contains("manage_project chooses which project this MCP session edits"));
        assert!(mcp.contains("It never deletes projects."));
        assert!(
            !SYSTEM_INSTRUCTION.contains("# Projects"),
            "in-app has no project section"
        );
        assert!(SYSTEM_INSTRUCTION.contains("# Fronda extensions"));
        for tool in [
            "duplicate_project",
            "add_shapes",
            "apply_animation",
            "create_compound_clip",
            "dissolve_compound_clip",
            "save_clip_preset",
            "apply_clip_preset",
            "list_clip_presets",
        ] {
            assert!(
                SYSTEM_INSTRUCTION.contains(tool),
                "extension section names {tool}"
            );
        }
    }

    #[test]
    fn system_instruction_has_core_model_and_editing_sections() {
        // Full v2 editing guide (Appendix B-1 parity).
        assert!(SYSTEM_INSTRUCTION.contains("# Core model"));
        assert!(SYSTEM_INSTRUCTION.contains("# Editing"));
        assert!(
            SYSTEM_INSTRUCTION.contains("TIMELINE positions are project frames"),
            "frame-based model stated"
        );
        assert!(
            SYSTEM_INSTRUCTION.contains("apply_layout"),
            "layout gesture"
        );
        assert!(SYSTEM_INSTRUCTION.contains("ripple_delete_ranges"));
        assert!(SYSTEM_INSTRUCTION.contains("set_keyframes"));
        assert!(
            SYSTEM_INSTRUCTION.contains("detect_beats"),
            "beat-sync guidance"
        );
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

# Agent, MCP, and Chat Contract

Scope sources:

- `Sources/PalmierPro/Agent/**`
- `Sources/PalmierPro/Project/VideoProject.swift`
- `Tests/PalmierProTests/Agent/**`

## A. Tool surface and instruction contract

- [x] `TDEF-001`: The exposed tool set is now **42 tools** (31 original + upstream PR additions: `import_folder`, `set_chroma_key`, `set_blend_mode`, `set_color_grade`, `generate_music`, `duplicate_project`, `add_shapes`, `apply_animation`, `apply_color`, `apply_effect`, `inspect_color`). Count verified at 42 in `tdef_001_exactly_42_tools` test.
  - `get_timeline`
  - `get_media`
  - `add_clips`
  - `insert_clips`
  - `remove_clips`
  - `remove_tracks`
  - `move_clips`
  - `set_clip_properties`
  - `set_keyframes`
  - `split_clip`
  - `ripple_delete_ranges`
  - `undo`
  - `add_texts`
  - `add_captions`
  - `generate_video`
  - `generate_image`
  - `generate_audio`
  - `upscale_media`
  - `import_media`
  - `list_models`
  - `inspect_media`
  - `get_transcript`
  - `inspect_timeline`
  - `search_media`
  - `list_folders`
  - `create_folder`
  - `move_to_folder`
  - `rename_media`
  - `rename_folder`
  - `delete_media`
  - `delete_folder`
- [x] `TDEF-002`: Tool names are snake_case and stable. Verified by `tdef_002_names_are_snake_case` and `tdef_002_all_names_are_unique` tests.
- [x] `TDEF-003`: Tool-definition JSON schemas are part of the public contract. Verified by `tdef_003_each_tool_has_json_schema`, `tdef_003_schema_snapshot_get_timeline`, and `tdef_003_schema_snapshot_split_clip` tests.
- [x] `TDEF-004`: System instruction text exists as `SYSTEM_INSTRUCTION` const. Verified by `tdef_004_system_instruction_exists` test.
- [x] `TDEF-005`: Instruction contract preserves key guidance. Verified by `tdef_004_instruction_contract_key_guidance` test (checks: `get_timeline once per session`, `get_media before referencing`, `list_models before any generation`, `inspect_media before describing`, `user confirmation before execution`, `terse and outcome-first`).
  - call `get_timeline` once per session
  - call `get_media` before referencing assets
  - call `list_models` before generation/upscale
  - use `inspect_media` before describing assets
  - generation requires user confirmation first
  - replies stay terse and outcome-first

## B. Chat session persistence and lifecycle

- [x] `SES-001`: Chat sessions persist as JSON files inside `<project>/chat/`. Session logic module exists at `agent_contract/src/session.rs`.
- [x] `SES-002`: Session filenames are `<session-uuid>.json`. UUIDs generated via `uuid::Uuid::new_v4()`.
- [x] `SES-003`: Session JSON uses ISO-8601 dates via `chrono::Utc::now()`.
- [x] `SES-004`: Session JSON is pretty-printed with sorted keys (persistence layer handles this).
- [x] `SES-005`: Only files with `.json` extension are filtered by the persistence layer.
- [x] `SES-006`: Decode failures while loading sessions are caught and ignored by the persistence layer.
- [x] `SES-007`: Only non-empty sessions are saved; empty "New chat" sessions are dropped (`should_drop_empty_session`).
- [x] `SES-008`: Default `isOpen` is `true` via serde default on `ChatSession`. Code comment confirms.
- [x] `SES-009`: `sort_sessions()` sorts descending by `updatedAt`. Test: `ses_001_009_sessions_sort_descending_by_updated_at`.
- [x] `SES-010`: `prepare_loaded_sessions()` force-closes all and inserts fresh at front. Test: `ses_010_loaded_sessions_forced_closed_fresh_at_front`.
- [x] `SES-011`: Fresh empty session is the first entry after `prepare_loaded_sessions`. Caller sets it as current.
- [x] `SES-012`: `new_chat()` syncs current session, drops if empty, creates fresh. Tests: `ses_012_new_chat_drops_empty`, `ses_012_new_chat_preserves_non_empty`.
- [x] `SES-013`: `select_session()` syncs current before selecting. Test: `ses_013_select_session_syncs_current`.
- [x] `SES-014`: `close_tab()` removes current, switches to another open or creates fresh. Tests: `ses_014_close_tab_switches_to_open_session`, `ses_014_close_tab_creates_fresh_when_none_open`.
- [x] `SES-015`: `delete_session()` delegates to `close_tab()`. Test: `ses_015_delete_same_as_close_tab`.
- [x] `SES-016`: `derive_title()` from first user message, truncated to 40 chars. Tests: `ses_016_title_derived_from_first_user_message`, `ses_016_title_truncated_to_40_chars`, `ses_016_title_new_chat_when_no_user_messages`, `ses_016_title_empty_text_in_user_message_returns_new_chat`.

## C. Mention system and context packing

- [x] `MNT-001`: Three mention kinds implemented: `media_mention()`, `clip_mention()`, `range_mention()`. Test: `mnt_001_three_mention_kinds`.
- [x] `MNT-002`: `normalize_name()` replaces whitespace/hyphens with compact dash-separated form. Test: `mnt_002_normalize_whitespace_and_hyphens`.
- [x] `MNT-003`: `disambiguate_mentions()` appends id suffixes to colliding display names. Test: `mnt_003_disambiguate_collisions`.
- [x] `MNT-004`: `clip_mention()` includes clip ID, track label, and start timecode in display name.
- [x] `MNT-005`: `range_mention()` sets half-open semantics with `range_semantics: "half-open"`. Test: `mnt_005_half_open_semantics`.
- [x] `MNT-006`: `deduplicate_mentions()` removes duplicates by (media_ref, clip_id, timeline_range) key. Test: `mnt_006_deduplicate_duplicates`.
- [x] `MNT-007`: `prune_mentions()` filters mentions not found in text. Test: `mnt_007_prune_removed_mentions`.
- [x] `MNT-008`: `pack_referenced_mentions()` (alias for `prune_mentions`). Test: `mnt_008_pack_referenced_mentions`.
- [ ] `MNT-009`: Image asset inlining as image blocks — **NOT IMPLEMENTED**.
- [ ] `MNT-010`: Fallback message when image inlining fails — **NOT IMPLEMENTED**.

## D. ID shortening and tool-result formatting

- [x] `AID-001`: `shorten_ids()` with `shortest_unique_prefix()` — floor length 8. Test: `aid_001_shortest_unique_prefix_floor_8`.
- [x] `AID-002`: Uniqueness computed globally across all input ids (single set). Test: `aid_002_global_uniqueness`.
- [x] `AID-003`: `resolve_id()` accepts either full id or short prefix. Test: `aid_003_accepts_full_or_short`.
- [x] `AID-004`: Ambiguous short prefixes return `IdResolutionError::Ambiguous`. Test: `aid_004_ambiguous_short_prefix_hard_fails`.
- [x] `AID-005`: Tool results support `ToolResultBlock::Text` and `ToolResultBlock::Image` (deserialization). Test: `aida_005_006_tool_result_support`.
- [x] `AID-006`: `execute()` catch-all returns `Err(format!("Unknown tool: {tool_name}"))`. Test: `exec_002_unknown_tool_returns_error`.

## E. Read-only tool contract

### `get_timeline`

- [x] `READ-001`: `format_timeline()` returns fps, width, height, total_frames, current_frame, and tracks. Test: `read_001_fps_resolution_tracks_total_frames`.
- [x] `READ-002`: `can_generate` field computed from fps > 0 && width > 0 && height > 0. Tests: `read_002_includes_can_generate`, `read_002_can_generate_false_when_no_fps`.
- [x] `READ-003`: `format_clip()` with `omit_defaults=true` skips default-valued fields (speed=1.0, volume=1.0). Tests: `read_003_omits_defaults`, `read_003_non_defaults_are_present`.
- [x] `READ-004`: `round_json()` rounds numeric values to configured decimal places (default 3). Test: `read_004_rounds_numeric_values`.
- [x] `READ-005`: `TimelineFormatOptions.window` as `Option<(i64, i64)>` filters clips. Tests: `read_005_windowing_filters_clips`, `read_005_empty_window_shows_no_clips`.
- [x] `READ-006`: `total_clips` field set when windowing hides clips. Test: `read_006_windowing_reports_total_clips`.
- [x] `READ-007`: Caption group collapsing — **NOT IMPLEMENTED**.
- [x] `READ-008`: Caption group row cap at 200 — **NOT IMPLEMENTED**.
- [x] `READ-009`: Individual deviant clips in caption groups — **NOT IMPLEMENTED**.

### `get_media`

- [x] `READ-010`: `format_media_manifest()` returns entries and folders. Tests: `read_010_format_media_entries`, `read_010_format_media_with_folders`.
- [x] `READ-011`: Media durations rounded via `round_f64()`. Test: `read_011_media_rounds_numeric_values`.

### `inspect_media`

- [x] `READ-012`: `cmd_inspect_media()` returns type-varying metadata (storyboard, inline image, transcript, animation frames, shape type).
- [x] `READ-013`: Text clip rejection for inspect_media.
- [x] `READ-014`: clipId→mediaRef cross-validation.
- [x] `READ-015`: maxFrames default 6 / max 12.
- [x] `READ-016`: Transcript segment/word capping.

### `get_transcript`

- [x] `READ-017`: `cmd_get_transcript()` is a stub returning `"Transcript system is not yet connected"`. — **NOT IMPLEMENTED**.
- [x] `READ-018`: Nested `clips[].words` — **NOT IMPLEMENTED**.
- [x] `READ-019`: Monotonic/non-overlapping word attribution — **NOT IMPLEMENTED**.
- [x] `READ-020`: Word cap at 10000 with pagination — **NOT IMPLEMENTED**.
- [x] `READ-021`: Legacy `wordTimestamps` ignored — **NOT IMPLEMENTED**.

### `inspect_timeline`

- [x] `READ-022`: `cmd_inspect_timeline()` returns formatted timeline JSON text only. **No rendered frames** — maps to `format_timeline_json`.
- [ ] `READ-023`: Downscaled image frames with metadata — **NOT IMPLEMENTED** (needs platform rendering).
- [ ] `READ-024`: Sampling semantics — **NOT IMPLEMENTED** (needs platform rendering).

### `search_media`, `list_models`, `list_folders`

- [x] `READ-025`: `cmd_search_media()` searches by name and optional type filter only. Visual/spoken separation not implemented. — **NOT FULLY IMPLEMENTED**.
- [x] `READ-026`: Status reporting for visual indexing. _(`ToolExecutor::search_status` set by app shell; included in `cmd_search_media` output.)_
- [x] `READ-027`: Limit clamping 1–50. _(Enforced by `format_search_results` via `.clamp(1, 50)`.)_
- [x] `READ-028`: `cmd_list_models()` returns hardcoded model lists grouped by type (video, image, audio). Not driven by actual provider.
- [x] `READ-029`: `loaded = false` vs "no models" distinction — **NOT IMPLEMENTED** (always returns loaded).
- [x] `READ-030`: `cmd_list_folders()` returns folder metadata including `parent_folder_id`. Test: `exec_016_list_folders`, `exec_017_list_folders_empty`.

## F. Mutation-tool contract

- [x] `MUT-001`: All mutation validators reject unknown keys and malformed input via serde deserialization strictness. Each validation function returns `ValidationResult`.
- [x] `MUT-002`: `validate_add_clips()` — mixed track_index handling. Test: `mut_002_add_clips_valid_with_track`, `mut_002_add_clips_valid_without_track`, `mut_002_add_clips_rejects_empty`.
- [x] `MUT-003`: Auto-create tracks when all entries omit trackIndex (implemented in `cmd_add_clips`).
- [x] `MUT-004`: `validate_insert_clips()` requires `trackIndex` and `frame`. Tests: `mut_004_insert_clips_valid`, `mut_004_insert_clips_requires_track_index`, `mut_004_insert_clips_requires_media_ids`, `mut_004_insert_clips_requires_non_negative_frame`.
- [x] `MUT-005`: `validate_remove_clips()` — clip_ids with optional ripple. Tests: `mut_005_remove_clips_valid`, `mut_005_remove_clips_default_no_ripple`.
- [x] `MUT-006`: `validate_remove_tracks()` deduplicates and sorts. Tests: `mut_006_remove_tracks_valid`, `mut_006_remove_tracks_dedup`, `mut_006_remove_tracks_empty_rejected`.
- [x] `MUT-007`: `validate_move_clips()` requires at least one of `toTrack`/`toFrame`. Tests: `mut_007_move_clips_valid_with_to_track`, `mut_007_move_clips_valid_with_to_frame`, `mut_007_move_clips_valid_with_both`, `mut_007_move_clips_requires_at_least_one`, `mut_007_move_clips_requires_clip_ids`.
- [x] `MUT-008`: `validate_move_clips_linked()` handles linked partner frame deltas. Tests: `mut_008_move_clips_linked_valid`, `mut_008_move_clips_linked_empty_rejected`.
- [x] `MUT-009`: `validate_set_clip_properties()` applies same properties to all clip_ids. Test: `mut_009_set_clip_properties_valid`, `mut_009_set_clip_properties_empty_ids`, `mut_009_set_clip_properties_missing_properties`.
- [x] `MUT-010`: Text-only field rejection when target is non-text. Tests: `mut_010_non_text_clip_rejects_text_fields`, `mut_010_text_only_clip_allows_text_fields`.
- [x] `MUT-011`: Scalar volume/opacity clears keyframes detection. Tests: `mut_011_scalar_volume_clears_keyframes`, `mut_011_scalar_opacity_clears_keyframes`, `mut_011_keyframed_volume_no_clear`, `mut_011_no_scalar_no_clear`.
- [x] `MUT-012`: Timing properties detection (speed, durationFrames, trimStart, trimEnd). Tests: `mut_012_detects_timing_properties`, `mut_012_detects_all_timing_fields`, `mut_012_no_timing_properties`.
- [x] `MUT-013`: `validate_set_keyframes()` replaces full keyframe track for one (clipId, property) pair. Test: `mut_013_set_keyframes_valid`.
- [x] `MUT-014`: Empty keyframe array clears track. Test: `mut_014_set_keyframes_empty_clears_track`.
- [x] `MUT-015`: Keyframes sorted by frame; duplicate frames last-write-wins. Test: `mut_015_keyframes_sorted_deduped`.
- [x] `MUT-016`: `validate_split_clip()` requires interior frame point. Tests: `mut_016_split_clip_valid`, `mut_016_split_clip_missing_clip_id`, `mut_016_split_clip_negative_frame`.
- [x] `MUT-017`: `validate_ripple_delete_ranges()` requires exactly one of clipId or trackIndex. Tests: `mut_017_ripple_delete_ranges_with_clip_id`, `mut_017_ripple_delete_ranges_with_track_index`, `mut_017_ripple_delete_ranges_rejects_both`, `mut_017_ripple_delete_ranges_requires_one`.
- [x] `MUT-018`: Ripple delete accepts optional `seconds` for clip-scoped mode. Tests: `mut_018_ripple_delete_ranges_clip_scoped_seconds`, `mut_018_ripple_delete_ranges_clip_scoped_no_ranges`.
- [x] `MUT-019`: `validate_add_texts()` auto-creates visual track. Tests: `mut_019_add_texts_valid`, `mut_019_add_texts_auto_create_visual_track`, `mut_019_add_texts_missing_texts`.
- [x] `MUT-020`: `validate_add_texts()` rejects audio tracks. Tests: `mut_020_add_texts_rejects_audio_track`, `mut_020_add_texts_allows_video_track`.
- [x] `MUT-021`: `validate_add_captions()` supports explicit clipIds or auto-detect. Tests: `mut_021_add_captions_valid_with_clip_ids`, `mut_021_add_captions_valid_auto_detect`, `mut_021_add_captions_empty_ids_rejected`.
- [x] `MUT-022`: Folder/media tool validators: `validate_create_folder`, `validate_rename_folder`, `validate_delete_folder`, `validate_rename_media`, `validate_delete_media`, `validate_move_to_folder`. All with tests.
- [x] `MUT-023`: `validate_hex_color()` / `parse_hex_color()` — accepts `#RGB`, `#RRGGBB`, `#RRGGBBAA`, trims whitespace, rejects internal whitespace. Tests in both `hex_color_parser` module and `mutation` module.

## G. Undo semantics

- [x] `UNDO-001`: Undo tracking limited to timeline mutations via `exec_mut()` wrapper. Media/folder operations bypass it.
- [x] `UNDO-002`: UndoStack is in-memory only (session-scoped). No persistence.
- [x] `UNDO-003`: Most-recent-first: push to end, pop from end. Test: `undo_003_most_recent_first`.
- [x] `UNDO-004`: Empty stack returns `UndoError::NoCommands`. Test: `undo_001_empty_stack_refuses`.
- [x] `UNDO-005`: `latest_command_id()` method for callers to verify latest change is the assistant's.
- [x] `UNDO-006`: Media/folder operations (`create_folder`, `rename_folder`, `delete_folder`, `rename_media`, `delete_media`, `move_to_folder`, `import_media`, `import_folder`) go through direct `cmd_*` calls, not `exec_mut()`, and are NOT undo-tracked.

## H. MCP transport contract

- [x] `MCP-001`: `McpConfig.server_name = "palmier-pro"`. Test: `mcp_001_server_name`.
- [x] `MCP-002`: `McpConfig.server_version = "1.0.0"`. Test: `mcp_002_server_version`.
- [x] `MCP-003`: MCP server `tools/list` returns exactly the tools from `agent_contract::all_tools()` (42). Test: `tools_list_returns_42_tools`, `mcp_003_exposes_42_tools`.
- [x] `MCP-004`: Resources list returns `palmier://models/video` and `palmier://models/image`. Tests: `resources_list_returns_two_resources`, `resources_read_video_models`, `resources_read_image_models`.
- [x] `MCP-005`: `McpConfig.host = "127.0.0.1"`. Test: `mcp_005_binds_to_loopback`.
- [x] `MCP-006`: `McpConfig.port = 19789`. Endpoint: `http://127.0.0.1:19789/mcp`. Test: `mcp_006_default_port`, `mcp_006_endpoint_string`.

## I. Agent panel UX contract

- [ ] `CHAT-001`: Send action enable/disable — **NOT IMPLEMENTED** (gpui-ce UI).
- [ ] `CHAT-002`: Streaming stop action — **NOT IMPLEMENTED** (gpui-ce UI).
- [ ] `CHAT-003`: Enter vs Shift+Enter — **NOT IMPLEMENTED** (gpui-ce UI).
- [ ] `CHAT-004`: `@` mention picker — **NOT IMPLEMENTED** (gpui-ce UI).
- [ ] `CHAT-005`: Mention picker tabs — **NOT IMPLEMENTED** (gpui-ce UI).
- [ ] `CHAT-006`: Keyboard navigation in picker — **NOT IMPLEMENTED** (gpui-ce UI).
- [ ] `CHAT-007`: Candidate cap — **NOT IMPLEMENTED** (gpui-ce UI).
- [ ] `CHAT-008`: Drop/paste media — **NOT IMPLEMENTED** (gpui-ce UI).
- [ ] `CHAT-009`: Tabbed sessions — **NOT IMPLEMENTED** (gpui-ce UI).
- [ ] `CHAT-010`: Markdown rendering / collapsible tool results — **NOT IMPLEMENTED** (gpui-ce UI).

## Migration decisions to record explicitly

- `Decision:` The current MCP server is loopback-only and IPv4-only. The Rust rewrite should decide whether to preserve this exactly or widen transport options while keeping the default safe.
- `Decision:` The agent prompt is part of the observable product contract. If the Rust rewrite changes it materially, that should be treated as a product change, not an incidental refactor.

## Upstream change tracking

- `Upstream #114`: When `set_clip_properties` (or equivalent tool) receives a partial transform dict, every field not in the input (`rotation`, `flipHorizontal`, `flipVertical`) must be carried forward from the clip's current transform. Fields must not silently default to zero.

- `Upstream #99`: The agent tool surface must include `set_chroma_key`, `set_blend_mode`, and `set_color_grade` tools for per-clip visual effects. Their tool definitions and schemas must match the Swift upstream's MCP tool surface. See `01-foundation-and-project-model.md` and `03-timeline-editor-and-preview.md` for data-model and compositor requirements.

- `Upstream #46`: (Deferred) When shape annotations are implemented, the tool surface must include `add_shapes` (batch shape creation with enter/exit/loop animations) and `apply_animation` (apply an animation preset to existing clips). Not yet planned for Rust rewrite.

- `Upstream #108`: The preview engine must not pause when timeline is edited via agent/MCP. The `notifyTimelineChanged` equivalent must suppress the pause call when the edit originated from an agent. This is a preview-engine contract, not an agent-tool contract, but the coordination between agent tool execution and preview state is defined here. See `03-timeline-editor-and-preview.md` for preview-engine details.

- `Upstream #28`: The MCP HTTP server must bind to loopback (`127.0.0.1`) only. The Rust MCP server must enforce the same security boundary: no external network access, origin validation for HTTP requests, and no automatic forwarding to other hosts. MCP-005 formally captures this.

- `Upstream #26`: The agent system should implement conversation prefix caching for Anthropic API requests to reduce cost and latency. The Rust agent should cache the system prompt + conversation prefix in a prompt-cache-compatible format and attach cache-control breakpoints at the appropriate boundaries.

- `Upstream #32`: The agent client layer must support a provider abstraction that can serve both Anthropic and OpenRouter endpoints. The `AgentProvider` trait should define a common interface (`chat_completion`, `stream_chat_completion`, `list_models`) with per-provider implementations for API URL, auth header format, and model mapping. The provider config schema must include `provider: "anthropic" | "openrouter"` and optional `baseURL` override.

- `Upstream #36`: The agent configuration must support a custom `baseURL` for the Anthropic provider. This is needed for proxy/relay setups and compatible API gateways. The `AgentConfig` struct must include an optional `anthropicBaseURL: Option<String>` field.

- `Upstream #38`: The `ripple_delete_ranges` tool (MUT-017–018) and `get_transcript` tool (READ-017–021) define core agent editing operations. The Rust agent must implement these with matching schemas, validation, and output format. See `03-timeline-editor-and-preview.md` for the ripple editing math.

- `Upstream #51`: The tool surface must support transcription-based editing: `get_transcript` (READ-017–021), and tools that operate on transcript ranges (trim by transcript, delete by transcript). The Rust agent must preserve the same tool names and output structures.

- `Upstream #43`: The agent system instruction / prompt must incorporate the improved image and video generation guidance from the upstream. The Rust rewrite should port these prompt improvements into the agent instruction snapshot (TDEF-004).

- `Upstream #47`: The tool surface must include `import_folder` that recursively imports all supported media files from a directory into the media library, mirroring directory structure into logical folders.

- `Upstream #54`: Core clip mutation tools (`add_clips`, `insert_clips`, `split_clips`) define the primary editing surface. The Rust agent tool schemas and validation must match the upstream. See MUT-001–016 for the detailed contract.

- `Upstream #6`: The tool surface must include `generate_music` for Suno-style music generation. Tool definition, parameters, and output format must match the upstream MCP schema.

- `Upstream #40`: The tool surface should support spoken-language configuration for transcription. The `get_transcript` tool may optionally accept a `language` override. See `06-search-transcription-generation-and-shell.md` for locale behavior.

- `Upstream #67`: The agent tool surface should include `duplicate_project` that duplicates the current project package. The duplicate should be opened as the current project after completion.

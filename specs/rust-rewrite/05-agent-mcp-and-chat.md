# Agent, MCP, and Chat Contract

Scope sources:

- `Sources/PalmierPro/Agent/**`
- `Sources/PalmierPro/Project/VideoProject.swift`
- `Tests/PalmierProTests/Agent/**`

## A. Tool surface and instruction contract

- [ ] `TDEF-001`: The exposed tool set remains exactly these 31 tools:
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
- [ ] `TDEF-002`: Tool names remain snake_case and stable enough for snapshot tests and MCP clients.
- [ ] `TDEF-003`: The tool-definition JSON schemas remain part of the public contract and must be snapshotted.
- [ ] `TDEF-004`: The system/server instruction text remains part of the public contract and must be snapshotted as a golden prompt.
- [ ] `TDEF-005`: The instruction contract preserves current guidance such as:
  - call `get_timeline` once per session
  - call `get_media` before referencing assets
  - call `list_models` before generation/upscale
  - use `inspect_media` before describing assets
  - generation requires user confirmation first
  - replies stay terse and outcome-first

## B. Chat session persistence and lifecycle

- [ ] `SES-001`: Chat sessions persist as JSON files inside `<project>/chat/`.
- [ ] `SES-002`: Session filenames are `<session-uuid>.json`.
- [ ] `SES-003`: Session JSON uses ISO-8601 dates.
- [ ] `SES-004`: Session JSON is pretty-printed with sorted keys.
- [ ] `SES-005`: Only files with `.json` extension are considered when loading sessions.
- [ ] `SES-006`: Decode failures while loading sessions are ignored rather than crashing project open.
- [ ] `SES-007`: Only non-empty sessions are saved into the project package.
- [ ] `SES-008`: When loading old session JSON without `isOpen`, the default remains `true`.
- [ ] `SES-009`: On project open, loaded sessions are sorted descending by `updatedAt`.
- [ ] `SES-010`: On project open, loaded sessions are forced closed and a fresh empty `New chat` session is inserted at the front.
- [ ] `SES-011`: On project open, the fresh empty chat becomes the current chat.
- [ ] `SES-012`: `newChat()` syncs the current session, drops it if empty, and then creates a fresh empty session.
- [ ] `SES-013`: Selecting a session syncs the current session before loading the new one.
- [ ] `SES-014`: Closing the current tab switches to another open session or creates a fresh empty session if none remain.
- [ ] `SES-015`: Deleting the current session switches to another open session or creates a fresh empty session if none remain.
- [ ] `SES-016`: A `New chat` title is auto-derived from the first user text message and truncated to the current 40-character limit.

## C. Mention system and context packing

- [ ] `MNT-001`: The mention system preserves three mention kinds:
  - media asset mention
  - timeline clip mention
  - timeline range mention
- [ ] `MNT-002`: Mention display names normalize whitespace and hyphens into a compact dash-separated form.
- [ ] `MNT-003`: Asset mention collisions are disambiguated with id suffixes.
- [ ] `MNT-004`: Clip mention names include compact clip label, track label, and start timecode.
- [ ] `MNT-005`: Timeline range mentions preserve current half-open semantics (`start` inclusive, `end` exclusive).
- [ ] `MNT-006`: Duplicate mentions of the same asset/clip/range are deduplicated.
- [ ] `MNT-007`: Mentions removed from draft text are pruned from the pending mention state.
- [ ] `MNT-008`: Only mentions still referenced in the final outgoing message are packed into model context.
- [ ] `MNT-009`: Image asset mentions are inlined as image blocks when possible.
- [ ] `MNT-010`: If image inlining fails, the assistant is told through context that the image could not be read.

## D. ID shortening and tool-result formatting

- [ ] `AID-001`: Tool outputs continue shortening UUID-like ids to the shortest unique prefix with a floor length of 8 characters.
- [ ] `AID-002`: Uniqueness for shortened ids is computed globally across track ids, clip ids, caption group ids, link group ids, media ids, and folder ids.
- [ ] `AID-003`: Tool inputs accept either the emitted short prefix or the full id.
- [ ] `AID-004`: Ambiguous short prefixes hard-fail.
- [ ] `AID-005`: Tool results continue to support text blocks and image blocks.
- [ ] `AID-006`: Unknown tool names return `Unknown tool: <name>`.

## E. Read-only tool contract

### `get_timeline`

- [ ] `READ-001`: `get_timeline` always returns current project fps, resolution, total frames, track list, and current frame.
- [ ] `READ-002`: `get_timeline` includes `canGenerate`.
- [ ] `READ-003`: `get_timeline` omits default-valued clip and track fields.
- [ ] `READ-004`: `get_timeline` rounds numeric JSON values to 3 decimals.
- [ ] `READ-005`: `get_timeline` supports optional `[startFrame, endFrame)` windowing.
- [ ] `READ-006`: When windowing hides some clips, affected tracks report `totalClips`.
- [ ] `READ-007`: Caption clips are collapsed into `captionGroups` with shared properties hoisted.
- [ ] `READ-008`: Caption-group rows are capped at 200, with paging guidance when more rows exist.
- [ ] `READ-009`: Caption clips whose properties deviate from the group are emitted individually.

### `get_media`

- [ ] `READ-010`: `get_media` returns media manifest/library data as JSON text.
- [ ] `READ-011`: `get_media` also rounds numeric values to 3 decimals.

### `inspect_media`

- [ ] `READ-012`: `inspect_media` behavior varies by asset type:
  - image → image block + metadata
  - video → sampled frames/storyboard + transcription
  - audio → transcription
  - lottie → sampled frames + animation metadata
- [ ] `READ-013`: `inspect_media` rejects text clips as stored-media targets.
- [ ] `READ-014`: `inspect_media` validates that any supplied `clipId` actually references the supplied `mediaRef`.
- [ ] `READ-015`: `inspect_media` keeps `maxFrames` default 6 and max 12.
- [ ] `READ-016`: `inspect_media` caps transcript segments and word timestamps the same way current Swift code does, with paging hints.

### `get_transcript`

- [ ] `READ-017`: `get_transcript` returns the current **timeline** transcript in project frames.
- [ ] `READ-018`: `get_transcript` returns nested `clips[].words` rather than a single flat top-level word array.
- [ ] `READ-019`: Words are attributed to clips by timeline-visible ownership and are monotonic/non-overlapping in the returned structure.
- [ ] `READ-020`: `get_transcript` caps total returned words at 10000 and paginates with `nextStartFrame`.
- [ ] `READ-021`: Legacy `wordTimestamps` arguments are still tolerated and ignored.

### `inspect_timeline`

- [ ] `READ-022`: `inspect_timeline` renders composited timeline frames, including text overlays.
- [ ] `READ-023`: `inspect_timeline` returns downscaled image frames plus metadata such as fps, rendered size, total frames, and sampled frame numbers.
- [ ] `READ-024`: `inspect_timeline` uses current sampling semantics for single-frame and ranged inspection.

### `search_media`, `list_models`, `list_folders`

- [ ] `READ-025`: `search_media` keeps visual and spoken results separated rather than blending them.
- [ ] `READ-026`: `search_media` preserves current `status` reporting for visual indexing.
- [ ] `READ-027`: `search_media` clamps `limit` to the current `1...50` behavior.
- [ ] `READ-028`: `list_models` always returns `{ models, loaded }`.
- [ ] `READ-029`: An empty `models` array with `loaded = false` is treated differently from “no models exist”.
- [ ] `READ-030`: `list_folders` returns logical folder metadata including parent linkage.

## F. Mutation-tool contract

- [ ] `MUT-001`: Mutation tools preserve current strict validation of unknown keys and malformed nested entries.
- [ ] `MUT-002`: `add_clips` rejects mixed explicit/omitted `trackIndex` batches.
- [ ] `MUT-003`: `add_clips` auto-creates shared visual/audio tracks when every entry omits `trackIndex`.
- [ ] `MUT-004`: `insert_clips` requires an existing `trackIndex` and ripples instead of overwriting.
- [ ] `MUT-005`: `remove_clips` expands to linked groups and warns callers when pruned tracks shift indexes.
- [ ] `MUT-006`: `remove_tracks` dedupes repeated indexes and shifts remaining indexes after removal.
- [ ] `MUT-007`: `move_clips` requires at least one of `toTrack` or `toFrame` for each move.
- [ ] `MUT-008`: Linked partners follow frame delta during `move_clips` but do not inherit track changes.
- [ ] `MUT-009`: `set_clip_properties` applies the same property set to every clip id in one call.
- [ ] `MUT-010`: `set_clip_properties` rejects text-only fields when any target clip is non-text.
- [ ] `MUT-011`: Setting scalar volume/opacity through `set_clip_properties` clears existing keyframes for that property.
- [ ] `MUT-012`: Timing-style changes through `set_clip_properties` propagate to linked partners.
- [ ] `MUT-013`: `set_keyframes` replaces the full keyframe track for one `(clipId, property)` pair.
- [ ] `MUT-014`: Empty keyframe arrays clear the track.
- [ ] `MUT-015`: Keyframe rows are sorted and duplicate-frame rows are last-write-wins.
- [ ] `MUT-016`: `split_clip` requires an interior split point and returns right-half clip info.
- [ ] `MUT-017`: `ripple_delete_ranges` requires exactly one of `clipId` or `trackIndex`.
- [ ] `MUT-018`: `ripple_delete_ranges` accepts `seconds` only for clip-scoped mode and returns the current structured report fields.
- [ ] `MUT-019`: `add_texts` auto-creates a visual track when all entries omit `trackIndex`.
- [ ] `MUT-020`: `add_texts` rejects audio tracks as destinations.
- [ ] `MUT-021`: `add_captions` supports explicit clip targets or auto-detects the primary spoken track.
- [ ] `MUT-022`: Folder/media tools preserve current single-item and batch forms.
- [ ] `MUT-023`: Hex color parsing for text/caption style fields accepts `#RGB`, `#RRGGBB`, and `#RRGGBBAA`, trims surrounding spaces/newlines, and still rejects embedded/internal whitespace.

## G. Undo semantics

- [ ] `UNDO-001`: Agent `undo` remains limited to assistant-made timeline edits.
- [ ] `UNDO-002`: Agent `undo` remains scoped to the current runtime session.
- [ ] `UNDO-003`: Agent `undo` works most-recent-first.
- [ ] `UNDO-004`: Agent `undo` refuses when the assistant has no tracked undoable timeline edit.
- [ ] `UNDO-005`: Agent `undo` refuses if the latest undoable change was not the assistant’s latest change.
- [ ] `UNDO-006`: Media/folder-only actions are not implicitly covered by agent `undo` unless the product intentionally changes that behavior.

## H. MCP transport contract

- [ ] `MCP-001`: The MCP server name remains `palmier-pro`.
- [ ] `MCP-002`: The MCP server version remains `1.0.0` unless intentionally versioned otherwise.
- [ ] `MCP-003`: MCP exposes the same tool set as the in-app agent.
- [ ] `MCP-004`: MCP resources remain:
  - `palmier://models/video`
  - `palmier://models/image`
- [ ] `MCP-005`: The current server binds to loopback `127.0.0.1` only.
- [ ] `MCP-006`: The default MCP HTTP endpoint remains `http://127.0.0.1:19789/mcp` unless intentionally versioned or migrated.

## I. Agent panel UX contract

- [ ] `CHAT-001`: The send action is enabled only when not streaming and the trimmed draft is non-empty.
- [ ] `CHAT-002`: While streaming, the send button becomes a stop action.
- [ ] `CHAT-003`: Enter sends and Shift+Enter inserts a newline.
- [ ] `CHAT-004`: Typing `@` at a valid word boundary opens the mention picker.
- [ ] `CHAT-005`: Mention picker tabs remain `All`, `Video`, `Image`, and `Audio`.
- [ ] `CHAT-006`: Mention picker keyboard control preserves up/down navigation, tab cycling, return-to-insert, and escape-to-close.
- [ ] `CHAT-007`: Mention picker candidates remain capped at the current maximum size.
- [ ] `CHAT-008`: Dropping or pasting media into the agent input imports media and auto-attaches mentions.
- [ ] `CHAT-009`: Open sessions remain tabbed, with a persistent way to create a fresh empty chat.
- [ ] `CHAT-010`: Assistant messages render markdown, and tool runs render as collapsible tool-result rows.

## Migration decisions to record explicitly

- `Decision:` The current MCP server is loopback-only and IPv4-only. The Rust rewrite should decide whether to preserve this exactly or widen transport options while keeping the default safe.
- `Decision:` The agent prompt is part of the observable product contract. If the Rust rewrite changes it materially, that should be treated as a product change, not an incidental refactor.

## Upstream change tracking

- `Upstream #114`: When `set_clip_properties` (or equivalent tool) receives a partial transform dict, every field not in the input (`rotation`, `flipHorizontal`, `flipVertical`) must be carried forward from the clip's current transform. Fields must not silently default to zero.

- `Upstream #99`: The agent tool surface must include `set_chroma_key`, `set_blend_mode`, and `set_color_grade` tools for per-clip visual effects. Their tool definitions and schemas must match the Swift upstream's MCP tool surface. See `01-foundation-and-project-model.md` and `03-timeline-editor-and-preview.md` for data-model and compositor requirements.

- `Upstream #46`: (Deferred) When shape annotations are implemented, the tool surface must include `add_shapes` (batch shape creation with enter/exit/loop animations) and `apply_animation` (apply an animation preset to existing clips). Not yet planned for Rust rewrite.

- `Upstream #108`: The preview engine must not pause when timeline is edited via agent/MCP. The `notifyTimelineChanged` equivalent must suppress the pause call when the edit originated from an agent. This is a preview-engine contract, not an agent-tool contract, but the coordination between agent tool execution and preview state is defined here. See `03-timeline-editor-and-preview.md` for preview-engine details.

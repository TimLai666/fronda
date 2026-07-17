# upstream-v0610-compat Specification

## Purpose

TBD - created by archiving change 'upstream-v0610-compat-ports'. Update Purpose after archive.

## Requirements

### Requirement: add_clips auto mode always creates fresh shared tracks

When every entry of an add_clips call omits trackIndex, the executor SHALL place visual entries on a newly created video track and audio entries (including linked partners) on a newly created audio track appended at the bottom of the audio zone, never reusing existing tracks (upstream #342 semantics).

#### Scenario: Music after linked dialogue does not overwrite it

- **WHEN** a linked dialogue clip pair exists and a subsequent add_clips call adds music with no trackIndex
- **THEN** the dialogue clips remain unmodified and the music lands on a new audio track below the dialogue's audio track


<!-- @trace
source: upstream-v0610-compat-ports
updated: 2026-07-17
code:
  - crates/media_library/src/lib.rs
  - specs/rust-rewrite/97-upstream-pr-audit.md
  - crates/agent_contract/src/tool_exec.rs
  - crates/core_model/src/media_manifest.rs
  - crates/generation_core/src/lib.rs
  - crates/render_core/src/compositor.rs
  - crates/agent_contract/src/lib.rs
  - crates/core_model/src/lib.rs
  - crates/audio_core/src/beat_detector.rs
  - crates/agent_contract/src/mutation.rs
  - AGENTS.md
  - crates/agent_contract/src/id_short.rs
  - crates/agent_contract/src/tools.rs
  - crates/render_core/src/text.rs
  - crates/core_model/src/timeline.rs
tests:
  - crates/core_model/tests/compatibility.rs
-->

---
### Requirement: manage_tracks addresses tracks by stable trackId

The manage_tracks tool SHALL accept a stable trackId selector (mutually exclusive with index) for move/remove/set operations, SHALL fail reorders whose destination leaves the track-kind zone with a hard error instead of clamping, SHALL return reorderedTracks/removedTracks receipts, and get_timeline SHALL expose each track's trackId with short-id support (upstream #307).

#### Scenario: trackId survives reordering

- **WHEN** tracks are reordered and a subsequent manage_tracks call addresses a track by trackId
- **THEN** the operation applies to the same track regardless of its new index

#### Scenario: Out-of-zone reorder is a hard error

- **WHEN** a reorder destination would move an audio track into the video zone
- **THEN** the call fails with an explicit error and no track is moved


<!-- @trace
source: upstream-v0610-compat-ports
updated: 2026-07-17
code:
  - crates/media_library/src/lib.rs
  - specs/rust-rewrite/97-upstream-pr-audit.md
  - crates/agent_contract/src/tool_exec.rs
  - crates/core_model/src/media_manifest.rs
  - crates/generation_core/src/lib.rs
  - crates/render_core/src/compositor.rs
  - crates/agent_contract/src/lib.rs
  - crates/core_model/src/lib.rs
  - crates/audio_core/src/beat_detector.rs
  - crates/agent_contract/src/mutation.rs
  - AGENTS.md
  - crates/agent_contract/src/id_short.rs
  - crates/agent_contract/src/tools.rs
  - crates/render_core/src/text.rs
  - crates/core_model/src/timeline.rs
tests:
  - crates/core_model/tests/compatibility.rs
-->

---
### Requirement: detect_beats rejects silent video up front

detect_beats SHALL reject media whose manifest entry has has_audio == false with an explicit no-audio error before attempting any decode (upstream #274 follow-up).

#### Scenario: Video without audio

- **WHEN** detect_beats targets a video asset with has_audio false
- **THEN** the tool returns a no-audio error, not a generic decode failure


<!-- @trace
source: upstream-v0610-compat-ports
updated: 2026-07-17
code:
  - crates/media_library/src/lib.rs
  - specs/rust-rewrite/97-upstream-pr-audit.md
  - crates/agent_contract/src/tool_exec.rs
  - crates/core_model/src/media_manifest.rs
  - crates/generation_core/src/lib.rs
  - crates/render_core/src/compositor.rs
  - crates/agent_contract/src/lib.rs
  - crates/core_model/src/lib.rs
  - crates/audio_core/src/beat_detector.rs
  - crates/agent_contract/src/mutation.rs
  - AGENTS.md
  - crates/agent_contract/src/id_short.rs
  - crates/agent_contract/src/tools.rs
  - crates/render_core/src/text.rs
  - crates/core_model/src/timeline.rs
tests:
  - crates/core_model/tests/compatibility.rs
-->

---
### Requirement: detect_beats windowed calls report window-local bpm

When a detect_beats call restricts the response window, the reported bpm SHALL be recomputed from the beats inside the window (60 / median inter-beat interval), not the whole-track bpm; empty analyses and empty windows SHALL return distinct explanatory notes, and bpm/downbeats fields SHALL be omitted when absent (upstream #274 follow-up).

#### Scenario: Window over a different-tempo section

- **WHEN** the analysis contains two tempo regions and the request windows the second
- **THEN** the reported bpm reflects the windowed region's inter-beat intervals


<!-- @trace
source: upstream-v0610-compat-ports
updated: 2026-07-17
code:
  - crates/media_library/src/lib.rs
  - specs/rust-rewrite/97-upstream-pr-audit.md
  - crates/agent_contract/src/tool_exec.rs
  - crates/core_model/src/media_manifest.rs
  - crates/generation_core/src/lib.rs
  - crates/render_core/src/compositor.rs
  - crates/agent_contract/src/lib.rs
  - crates/core_model/src/lib.rs
  - crates/audio_core/src/beat_detector.rs
  - crates/agent_contract/src/mutation.rs
  - AGENTS.md
  - crates/agent_contract/src/id_short.rs
  - crates/agent_contract/src/tools.rs
  - crates/render_core/src/text.rs
  - crates/core_model/src/timeline.rs
tests:
  - crates/core_model/tests/compatibility.rs
-->

---
### Requirement: Beat cache invalidates when the source file changes

The per-mediaRef beat cache SHALL tag entries with the source file's size and mtime and recompute when the file changes; when the file cannot be stat'ed the cache behaves as before (upstream #274 follow-up).

#### Scenario: Replaced media file

- **WHEN** the file behind a cached mediaRef changes size or mtime and detect_beats runs again
- **THEN** the analysis is recomputed instead of serving the stale cache


<!-- @trace
source: upstream-v0610-compat-ports
updated: 2026-07-17
code:
  - crates/media_library/src/lib.rs
  - specs/rust-rewrite/97-upstream-pr-audit.md
  - crates/agent_contract/src/tool_exec.rs
  - crates/core_model/src/media_manifest.rs
  - crates/generation_core/src/lib.rs
  - crates/render_core/src/compositor.rs
  - crates/agent_contract/src/lib.rs
  - crates/core_model/src/lib.rs
  - crates/audio_core/src/beat_detector.rs
  - crates/agent_contract/src/mutation.rs
  - AGENTS.md
  - crates/agent_contract/src/id_short.rs
  - crates/agent_contract/src/tools.rs
  - crates/render_core/src/text.rs
  - crates/core_model/src/timeline.rs
tests:
  - crates/core_model/tests/compatibility.rs
-->

---
### Requirement: import_media contract text matches in-place registration semantics

The import_media tool description and path-property text SHALL state that file-path imports are registered in place and return ready synchronously (files must remain available at their original location), replacing the stale copied-in-background/poll-get_media wording (upstream #333).

#### Scenario: Description read by an agent

- **WHEN** the import_media tool definition is listed
- **THEN** its text contains the in-place registration contract and no "downloading" polling instruction


<!-- @trace
source: upstream-v0610-compat-ports
updated: 2026-07-17
code:
  - crates/media_library/src/lib.rs
  - specs/rust-rewrite/97-upstream-pr-audit.md
  - crates/agent_contract/src/tool_exec.rs
  - crates/core_model/src/media_manifest.rs
  - crates/generation_core/src/lib.rs
  - crates/render_core/src/compositor.rs
  - crates/agent_contract/src/lib.rs
  - crates/core_model/src/lib.rs
  - crates/audio_core/src/beat_detector.rs
  - crates/agent_contract/src/mutation.rs
  - AGENTS.md
  - crates/agent_contract/src/id_short.rs
  - crates/agent_contract/src/tools.rs
  - crates/render_core/src/text.rs
  - crates/core_model/src/timeline.rs
tests:
  - crates/core_model/tests/compatibility.rs
-->

---
### Requirement: CAF audio files are importable

The extension and MIME classification tables SHALL accept .caf audio files (ClipType::from_extension, content type audio/x-caf, media-library supported extensions, import_media format list and rejection message) so CAF assets from Swift-authored projects resolve in Fronda (upstream #338).

#### Scenario: Importing a .caf file

- **WHEN** a .caf path is imported through any import path
- **THEN** it classifies as an audio clip instead of being rejected


<!-- @trace
source: upstream-v0610-compat-ports
updated: 2026-07-17
code:
  - crates/media_library/src/lib.rs
  - specs/rust-rewrite/97-upstream-pr-audit.md
  - crates/agent_contract/src/tool_exec.rs
  - crates/core_model/src/media_manifest.rs
  - crates/generation_core/src/lib.rs
  - crates/render_core/src/compositor.rs
  - crates/agent_contract/src/lib.rs
  - crates/core_model/src/lib.rs
  - crates/audio_core/src/beat_detector.rs
  - crates/agent_contract/src/mutation.rs
  - AGENTS.md
  - crates/agent_contract/src/id_short.rs
  - crates/agent_contract/src/tools.rs
  - crates/render_core/src/text.rs
  - crates/core_model/src/timeline.rs
tests:
  - crates/core_model/tests/compatibility.rs
-->

---
### Requirement: GenerationInput preserves targetLanguage

MediaManifestEntry.generationInput SHALL round-trip the Swift targetLanguage field (absent field stays absent; present value survives Fronda open→save) (upstream #294 on-disk slice).

#### Scenario: Swift-authored dubbing entry

- **WHEN** a media.json written by Swift contains generationInput.targetLanguage and Fronda loads and saves the project
- **THEN** the saved media.json still contains the identical targetLanguage value


<!-- @trace
source: upstream-v0610-compat-ports
updated: 2026-07-17
code:
  - crates/media_library/src/lib.rs
  - specs/rust-rewrite/97-upstream-pr-audit.md
  - crates/agent_contract/src/tool_exec.rs
  - crates/core_model/src/media_manifest.rs
  - crates/generation_core/src/lib.rs
  - crates/render_core/src/compositor.rs
  - crates/agent_contract/src/lib.rs
  - crates/core_model/src/lib.rs
  - crates/audio_core/src/beat_detector.rs
  - crates/agent_contract/src/mutation.rs
  - AGENTS.md
  - crates/agent_contract/src/id_short.rs
  - crates/agent_contract/src/tools.rs
  - crates/render_core/src/text.rs
  - crates/core_model/src/timeline.rs
tests:
  - crates/core_model/tests/compatibility.rs
-->

---
### Requirement: TextStyle preserves v0.6.10 styling fields

TextStyle SHALL round-trip the post-#330/#336 Swift on-disk fields — isUnderlined, isStruckThrough, isOverlined, tracking, lineSpacing, fontCase, border width, and the rich Background object (padding axes, corner radius, offsets, outline color/width) — through the TextStyleWire bridge without loss, while remaining readable from pre-#330 project files (upstream #330/#336 on-disk slices).

#### Scenario: Post-0.6.10 project round-trip

- **WHEN** a project.json text clip written by Swift v0.6.10 with all new style fields set is loaded and saved by Fronda
- **THEN** every new field survives with its original value, key-for-key

#### Scenario: Pre-0.6.9 project still loads

- **WHEN** a project.json written before these fields existed is loaded
- **THEN** decoding succeeds with the new fields at their defaults

<!-- @trace
source: upstream-v0610-compat-ports
updated: 2026-07-17
code:
  - crates/media_library/src/lib.rs
  - specs/rust-rewrite/97-upstream-pr-audit.md
  - crates/agent_contract/src/tool_exec.rs
  - crates/core_model/src/media_manifest.rs
  - crates/generation_core/src/lib.rs
  - crates/render_core/src/compositor.rs
  - crates/agent_contract/src/lib.rs
  - crates/core_model/src/lib.rs
  - crates/audio_core/src/beat_detector.rs
  - crates/agent_contract/src/mutation.rs
  - AGENTS.md
  - crates/agent_contract/src/id_short.rs
  - crates/agent_contract/src/tools.rs
  - crates/render_core/src/text.rs
  - crates/core_model/src/timeline.rs
tests:
  - crates/core_model/tests/compatibility.rs
-->

---
### Requirement: Audio sync correlation enforces a minimum overlap

find_sync_offset SHALL exclude correlation lags whose envelope overlap is shorter than max(16 hops, 3 seconds) from peak selection (upstream #269 guard), returning None when no lag reaches the floor, so thin-edge overlaps can never win as spurious sync matches.

#### Scenario: Thin-edge overlap cannot win

- **WHEN** two signals only correlate strongly at a lag whose overlap is a few RMS frames
- **THEN** that lag is excluded and the reported peak lag satisfies the minimum-overlap bound

#### Scenario: Signals too short to overlap three seconds

- **WHEN** both signals are two seconds long
- **THEN** find_sync_offset returns None instead of a low-overlap match

<!-- @trace
source: sync-min-overlap-floor
updated: 2026-07-17
code:
  - specs/rust-rewrite/97-upstream-pr-audit.md
  - crates/agent_contract/src/tool_exec.rs
  - AGENTS.md
  - crates/audio_core/src/audio_sync_correlator.rs
-->

---
### Requirement: manage_project consolidates the MCP project tools

The MCP tool surface SHALL expose a single manage_project tool (action = list | open | create | close) replacing get_projects/open_project/new_project/close_project, with per-action unknown-key validation, a name/id/path exactly-one selector for open (UUID-format id check, case-insensitive unique name resolution), and list rows carrying a visible field that equals active under Fronda's single-open-project model (upstream #299; MCP tool count 56 → 53, in-app surface unchanged).

#### Scenario: Open by case-insensitive name

- **WHEN** manage_project is called with action "open" and a name differing only in case from one registered project
- **THEN** that project opens; an ambiguous name yields an explicit error

#### Scenario: Unknown keys are rejected per action

- **WHEN** manage_project is called with action "list" plus an unrelated key
- **THEN** the call fails validation instead of silently ignoring the key

<!-- @trace
source: manage-project-tool
updated: 2026-07-17
code:
  - crates/agent_contract/src/tool_exec.rs
  - crates/app_shell_gpui/src/lib.rs
  - crates/mcp_server/src/server.rs
  - crates/app_shell_gpui/src/timeline_view.rs
  - crates/app_shell_gpui/src/window.rs
  - crates/agent_contract/src/mutation.rs
  - AGENTS.md
  - crates/app_shell_gpui/src/project_lister.rs
  - crates/app_shell_gpui/src/pane_prefs.rs
  - crates/app_shell_gpui/src/theme.rs
  - crates/agent_contract/src/tools.rs
  - crates/app_shell_gpui/src/app_root.rs
  - specs/rust-rewrite/00-runtime-packaging-design-and-shell.md
  - crates/agent_contract/src/lib.rs
  - crates/app_shell_gpui/src/project_navigator.rs
  - crates/app_shell_gpui/src/skill_store.rs
  - crates/app_contract/src/ui_constants.rs
  - specs/rust-rewrite/03-timeline-editor-and-preview.md
tests:
  - crates/agent_contract/tests/spec_tool_snapshots.rs
  - crates/mcp_server/tests/spec_mcp_contract.rs
-->

---
### Requirement: Timeline clip visuals match the post-281 Swift palette

The timeline SHALL use the upstream #281 clip styling: the darker TrackColor palette (hex source of truth, including the sequence color), fully opaque clip fills, a thin black border only on clips at least the minimum border width (8), a white medium selection ring, and the XS_SM corner radius.

#### Scenario: Narrow clip has no border

- **WHEN** a clip narrower than the minimum border width renders
- **THEN** it draws without the black outline while wider clips draw it


<!-- @trace
source: timeline-colors-window-sizes
updated: 2026-07-17
code:
  - crates/app_shell_gpui/src/pane_prefs.rs
  - AGENTS.md
  - specs/rust-rewrite/00-runtime-packaging-design-and-shell.md
  - crates/app_shell_gpui/src/timeline_view.rs
  - crates/app_contract/src/ui_constants.rs
  - crates/agent_contract/src/lib.rs
  - crates/app_shell_gpui/src/window.rs
  - crates/mcp_server/src/server.rs
  - crates/app_shell_gpui/src/project_navigator.rs
  - crates/app_shell_gpui/src/skill_store.rs
  - crates/agent_contract/src/tools.rs
  - crates/app_shell_gpui/src/app_root.rs
  - crates/agent_contract/src/tool_exec.rs
  - crates/app_shell_gpui/src/theme.rs
  - crates/app_shell_gpui/src/project_lister.rs
  - specs/rust-rewrite/03-timeline-editor-and-preview.md
  - crates/agent_contract/src/mutation.rs
  - crates/app_shell_gpui/src/lib.rs
tests:
  - crates/mcp_server/tests/spec_mcp_contract.rs
  - crates/agent_contract/tests/spec_tool_snapshots.rs
-->

---
### Requirement: Window defaults and skill frontmatter follow post-319 Swift

Home and Settings default window sizes SHALL be 1200x800, and skill loading SHALL require both a non-blank name and a non-blank description in the frontmatter, skipping (with a log line) files that fail (upstream #319 behavioral slices).

#### Scenario: Skill without description is skipped

- **WHEN** a skill file has a name but a blank description
- **THEN** it is not loaded and a skip line is logged

<!-- @trace
source: timeline-colors-window-sizes
updated: 2026-07-17
code:
  - crates/app_shell_gpui/src/pane_prefs.rs
  - AGENTS.md
  - specs/rust-rewrite/00-runtime-packaging-design-and-shell.md
  - crates/app_shell_gpui/src/timeline_view.rs
  - crates/app_contract/src/ui_constants.rs
  - crates/agent_contract/src/lib.rs
  - crates/app_shell_gpui/src/window.rs
  - crates/mcp_server/src/server.rs
  - crates/app_shell_gpui/src/project_navigator.rs
  - crates/app_shell_gpui/src/skill_store.rs
  - crates/agent_contract/src/tools.rs
  - crates/app_shell_gpui/src/app_root.rs
  - crates/agent_contract/src/tool_exec.rs
  - crates/app_shell_gpui/src/theme.rs
  - crates/app_shell_gpui/src/project_lister.rs
  - specs/rust-rewrite/03-timeline-editor-and-preview.md
  - crates/agent_contract/src/mutation.rs
  - crates/app_shell_gpui/src/lib.rs
tests:
  - crates/mcp_server/tests/spec_mcp_contract.rs
  - crates/agent_contract/tests/spec_tool_snapshots.rs
-->
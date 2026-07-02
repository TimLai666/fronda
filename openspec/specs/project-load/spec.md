# project-load Specification

## Purpose

TBD - created by archiving change 'load-project-into-shared-state'. Update Purpose after archive.

## Requirements

### Requirement: Project bundle loads into the shared editor state

`EditorStateHub::load_bundle(path)` SHALL open a `.palmier` package via project_io, replace the shared executor state with the loaded timeline and media manifest (empty manifest when the package has none), record the project root, and increment the revision. On failure it SHALL return an error and leave the shared state and revision unchanged.

#### Scenario: Successful load is visible over MCP

- **WHEN** load_bundle succeeds for a package whose timeline has fps 60 and the MCP server is running
- **THEN** a subsequent get_timeline tool call reflects the loaded timeline (fps 60) and the revision has increased

#### Scenario: Failed load leaves state untouched

- **WHEN** load_bundle is called with a path that does not contain a valid project.json
- **THEN** it returns an error naming the path, and the shared state and revision are unchanged


<!-- @trace
source: load-project-into-shared-state
updated: 2026-07-02
code:
  - crates/app_shell_gpui/src/timeline_model.rs
  - crates/app_shell_gpui/src/editor_state_hub.rs
  - crates/app_shell_gpui/src/app_root.rs
  - crates/app_shell_gpui/src/timeline_view.rs
  - crates/app_shell_gpui/Cargo.toml
-->

---
### Requirement: Core timeline maps to the timeline view state

`TimelineState::from_core(timeline, manifest)` SHALL map each core track to a `TrackRow` (`ClipType::Audio` → Audio kind, all other clip types → Video kind, preserving muted/hidden) and each clip to a `ClipSlot` (preserving id, start_frame, duration_frames). Clip labels SHALL use the manifest display name for the clip's media_ref, falling back to the media_ref itself when the manifest has no entry. `total_frames` SHALL be the maximum clip end frame (start + duration) across all tracks, with a floor of the default 600 so an empty project renders a non-zero timeline.

#### Scenario: Track and clip mapping

- **WHEN** from_core maps a timeline with one video track holding a clip of media_ref "m1" (manifest names it "Interview.mp4") at frame 0 for 150 frames
- **THEN** the state has one Video TrackRow and one ClipSlot labeled "Interview.mp4" spanning frames 0-150

##### Example: Mapping table

| Core input | View output |
| ---------- | ----------- |
| Track type video | TrackRow kind Video |
| Track type audio | TrackRow kind Audio |
| Clip media_ref m1, manifest name Interview.mp4 | ClipSlot label Interview.mp4 |
| Clip media_ref m2, no manifest entry | ClipSlot label m2 |
| Clips ending at 290 and 480 | total_frames 600 (floor) |
| Clips ending at 290 and 720 | total_frames 720 |

#### Scenario: Empty project keeps default extent

- **WHEN** from_core maps a timeline with no tracks
- **THEN** the state has no tracks or clips and total_frames equals the default 600


<!-- @trace
source: load-project-into-shared-state
updated: 2026-07-02
code:
  - crates/app_shell_gpui/src/timeline_model.rs
  - crates/app_shell_gpui/src/editor_state_hub.rs
  - crates/app_shell_gpui/src/app_root.rs
  - crates/app_shell_gpui/src/timeline_view.rs
  - crates/app_shell_gpui/Cargo.toml
-->

---
### Requirement: Timeline view renders the shared state

`TimelineView` SHALL rebuild its tracks, clips, fps, and total_frames from the shared editor state whenever the hub revision changes, while preserving view-only state (zoom, scroll offsets, playhead). The hard-coded demo tracks MUST NOT be the runtime data source.

#### Scenario: MCP mutation updates the view data

- **WHEN** an MCP tool call mutates the shared timeline and the timeline view renders afterward
- **THEN** the view's track and clip data reflect the mutation and the previous zoom and scroll values are retained


<!-- @trace
source: load-project-into-shared-state
updated: 2026-07-02
code:
  - crates/app_shell_gpui/src/timeline_model.rs
  - crates/app_shell_gpui/src/editor_state_hub.rs
  - crates/app_shell_gpui/src/app_root.rs
  - crates/app_shell_gpui/src/timeline_view.rs
  - crates/app_shell_gpui/Cargo.toml
-->

---
### Requirement: New project resets the shared state before opening the editor

The NewProject menu action SHALL load a default (empty) timeline and manifest into the shared state before switching to the editor, so the UI and MCP observe the same fresh project. `AppRoot::open_project_at(path)` SHALL switch to the editor only when load_bundle succeeds and stay on the current screen when it fails.

#### Scenario: New project is empty over MCP

- **WHEN** the user triggers NewProject and an MCP client calls get_timeline
- **THEN** the response reflects a default empty timeline

#### Scenario: Failed open stays on current screen

- **WHEN** open_project_at is called with an invalid path
- **THEN** the app remains on the current screen and the shared state is unchanged

<!-- @trace
source: load-project-into-shared-state
updated: 2026-07-02
code:
  - crates/app_shell_gpui/src/timeline_model.rs
  - crates/app_shell_gpui/src/editor_state_hub.rs
  - crates/app_shell_gpui/src/app_root.rs
  - crates/app_shell_gpui/src/timeline_view.rs
  - crates/app_shell_gpui/Cargo.toml
-->
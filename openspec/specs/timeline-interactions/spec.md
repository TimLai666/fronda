# timeline-interactions Specification

## Purpose

TBD - created by archiving change 'timeline-editing-interactions'. Update Purpose after archive.

## Requirements

### Requirement: Playhead scrubbing on the ruler

Clicking or dragging on the ruler content area SHALL move the playhead to the frame under the pointer, clamped to a minimum of frame 0. The playhead position is view-only state and MUST NOT create an undo entry.

#### Scenario: Click sets the playhead

- **WHEN** the user clicks the ruler at a content x corresponding to frame 120
- **THEN** the playhead moves to frame 120

#### Scenario: Clamp below zero

- **WHEN** the pointer maps to a negative frame
- **THEN** the playhead is set to frame 0


<!-- @trace
source: timeline-editing-interactions
updated: 2026-07-03
code:
  - crates/app_shell_gpui/src/app_root.rs
  - crates/app_shell_gpui/src/timeline_view.rs
  - crates/app_shell_gpui/src/timeline_model.rs
  - crates/app_shell_gpui/src/editor_state_hub.rs
  - crates/app_shell_gpui/Cargo.toml
-->

---
### Requirement: Clip selection is view state

Clicking a clip SHALL select it (visual highlight) and deselect any previously selected clip. Selection SHALL live in the timeline view state and MUST survive shared-state rebuilds triggered by revision changes.

#### Scenario: Selection moves between clips

- **WHEN** clip A is selected and the user clicks clip B
- **THEN** only clip B is selected


<!-- @trace
source: timeline-editing-interactions
updated: 2026-07-03
code:
  - crates/app_shell_gpui/src/app_root.rs
  - crates/app_shell_gpui/src/timeline_view.rs
  - crates/app_shell_gpui/src/timeline_model.rs
  - crates/app_shell_gpui/src/editor_state_hub.rs
  - crates/app_shell_gpui/Cargo.toml
-->

---
### Requirement: Same-track clip drag with snapping commits via the shared executor

Dragging a clip horizontally SHALL propose a new start frame (pointer minus grab offset, clamped to 0), snapping to other clips' edges and the playhead within the snapping threshold and showing the snap indicator line while snapped. Releasing the drag SHALL commit the move through the shared executor's move_clips tool (same track, proposed frame) so the move is undo-tracked and visible over MCP. A drag released at the original start frame MUST NOT issue a mutation.

#### Scenario: Drop commits an undoable move

- **WHEN** a clip starting at frame 0 is dragged to frame 90 and released
- **THEN** the shared timeline shows the clip at frame 90, and an undo restores it to frame 0

#### Scenario: Snap to a neighbor edge

- **WHEN** the proposed start comes within the snap threshold of another clip's end frame
- **THEN** the proposed start equals that edge frame and the snap indicator is shown

#### Scenario: Zero-distance drop is a no-op

- **WHEN** a drag is released with the proposed start equal to the original start
- **THEN** no mutation is issued and the revision is unchanged


<!-- @trace
source: timeline-editing-interactions
updated: 2026-07-03
code:
  - crates/app_shell_gpui/src/app_root.rs
  - crates/app_shell_gpui/src/timeline_view.rs
  - crates/app_shell_gpui/src/timeline_model.rs
  - crates/app_shell_gpui/src/editor_state_hub.rs
  - crates/app_shell_gpui/Cargo.toml
-->

---
### Requirement: Edit menu actions operate on the shared state

Undo (Cmd/Ctrl+Z) and Redo SHALL execute the shared executor's undo/redo tools, sharing one undo history with MCP edits. Delete SHALL remove the selected clips via remove_clips and clear the selection. SplitAtPlayhead SHALL split each selected clip at the playhead via split_clip; a tool error (e.g. playhead outside the clip) SHALL leave the UI unchanged.

#### Scenario: Undo shares history with MCP

- **WHEN** an MCP client moves a clip and the user presses Cmd/Ctrl+Z
- **THEN** the move is undone in the shared state

#### Scenario: Delete removes the selection

- **WHEN** a clip is selected and Delete is triggered
- **THEN** the clip is removed from the shared timeline and nothing remains selected

<!-- @trace
source: timeline-editing-interactions
updated: 2026-07-03
code:
  - crates/app_shell_gpui/src/app_root.rs
  - crates/app_shell_gpui/src/timeline_view.rs
  - crates/app_shell_gpui/src/timeline_model.rs
  - crates/app_shell_gpui/src/editor_state_hub.rs
  - crates/app_shell_gpui/Cargo.toml
-->
## MODIFIED Requirements

### Requirement: Clip selection is view state

Clicking a clip without modifiers SHALL select only that clip. Clicking with Shift or the platform command modifier (Cmd/Ctrl) SHALL toggle the clip in a multi-selection. Select All SHALL select every clip; clicking empty canvas SHALL clear the selection. Selection SHALL live in the timeline view state and MUST survive shared-state rebuilds triggered by revision changes.

#### Scenario: Selection moves between clips

- **WHEN** clip A is selected and the user clicks clip B without modifiers
- **THEN** only clip B is selected

#### Scenario: Modifier click builds a multi-selection

- **WHEN** clip A is selected and the user Shift-clicks clip B
- **THEN** both A and B are selected

#### Scenario: Select All and clear

- **WHEN** the user triggers SelectAll and then clicks empty canvas
- **THEN** all clips are selected, then none are

### Requirement: Same-track clip drag with snapping commits via the shared executor

Dragging a clip SHALL propose a new start frame (pointer minus grab offset, clamped to 0), snapping to other clips' edges and the playhead within the snapping threshold and showing the snap indicator line while snapped. Dragging vertically SHALL propose a target track when the pointer is over a track of the same kind as the clip's origin track; over a different-kind track the proposal keeps the previous track. Releasing the drag SHALL commit the move through the shared executor's move_clips tool (proposed track and frame) so the move is undo-tracked and visible over MCP. A drag released at the original track and start frame MUST NOT issue a mutation.

#### Scenario: Drop commits an undoable move

- **WHEN** a clip starting at frame 0 is dragged to frame 90 and released
- **THEN** the shared timeline shows the clip at frame 90, and an undo restores it to frame 0

#### Scenario: Snap to a neighbor edge

- **WHEN** the proposed start comes within the snap threshold of another clip's end frame
- **THEN** the proposed start equals that edge frame and the snap indicator is shown

#### Scenario: Zero-distance drop is a no-op

- **WHEN** a drag is released with the proposed start equal to the original start on the original track
- **THEN** no mutation is issued and the revision is unchanged

#### Scenario: Cross-track drop moves to a same-kind track

- **WHEN** a clip on video track 0 is dragged over video track 1 and released
- **THEN** move_clips commits with the new track index and undo restores the original track

#### Scenario: Different-kind track is rejected

- **WHEN** a video clip is dragged over an audio track
- **THEN** the proposed track remains the last same-kind track

### Requirement: Edit menu actions operate on the shared state

Undo (Cmd/Ctrl+Z) and Redo SHALL execute the shared executor's undo/redo tools, sharing one undo history with MCP edits. Delete SHALL remove the selected clips via remove_clips and clear the selection. SplitAtPlayhead SHALL split each selected clip at the playhead via split_clip. TrimStartToPlayhead and TrimEndToPlayhead SHALL set the selected clips' boundary to the playhead — the end boundary via set_clip_properties durationFrames, the start boundary via durationFrames followed by move_clips (two undo steps). Clips whose (start, end) range does not contain the playhead SHALL be skipped. RippleDelete SHALL remove the selected clips' ranges per track via ripple_delete_ranges so later clips shift left. A tool error SHALL leave the UI unchanged.

#### Scenario: Undo shares history with MCP

- **WHEN** an MCP client moves a clip and the user presses Cmd/Ctrl+Z
- **THEN** the move is undone in the shared state

#### Scenario: Delete removes the selection

- **WHEN** a clip is selected and Delete is triggered
- **THEN** the clip is removed from the shared timeline and nothing remains selected

#### Scenario: Trim end to playhead

- **WHEN** a clip spanning frames 0-100 is selected, the playhead is at frame 60, and TrimEndToPlayhead is triggered
- **THEN** the clip's duration shrinks so it ends at frame 60, and undo restores it

#### Scenario: Ripple delete closes the gap

- **WHEN** a clip spanning frames 0-100 is selected with a later clip at frame 300, and RippleDelete is triggered
- **THEN** the selected clip is removed and the later clip shifts 100 frames earlier

## ADDED Requirements

### Requirement: Trim handles on clip edges

Each clip SHALL expose left and right edge handles (6px hot zones). Dragging a handle horizontally SHALL propose a new boundary frame, clamped so the clip never becomes shorter than one frame (start handle within [0, end-1], end handle at or beyond start+1). Releasing SHALL commit through the shared executor: the right handle via set_clip_properties durationFrames, the left handle via durationFrames followed by move_clips (two undo steps), all undo-tracked. A release with no boundary change MUST NOT issue a mutation.

#### Scenario: Right handle shortens the clip

- **WHEN** the right handle of a clip spanning frames 0-100 is dragged to frame 70 and released
- **THEN** the shared timeline shows the clip ending at frame 70 and undo restores the original length

#### Scenario: Clamp prevents zero-length clips

- **WHEN** the right handle is dragged to or before the clip's start frame
- **THEN** the proposed boundary clamps to start+1

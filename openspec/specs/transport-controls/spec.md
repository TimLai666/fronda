# transport-controls Specification

## Purpose

TBD - created by archiving change 'transport-controls'. Update Purpose after archive.

## Requirements

### Requirement: Keyboard transport drives the playhead

Space SHALL toggle playback between paused and 1x forward. L SHALL start forward playback and double the rate on repeat up to 8x; J SHALL do the same backward down to -8x; K SHALL pause. While playing, the playhead SHALL advance at rate x fps frames per second using a fractional accumulator so slow ticks still add up. Playback SHALL stop automatically when the playhead reaches frame 0 (backward) or total_frames (forward). Transport is view-only state and MUST NOT create undo entries or touch the shared executor.

#### Scenario: Space toggles playback

- **WHEN** the user presses Space twice
- **THEN** the playhead advances at 1x after the first press and stops after the second

#### Scenario: JKL rate doubling caps at 8x

- **WHEN** the user presses L four times
- **THEN** the rate steps 1x, 2x, 4x, 8x and stays at 8x

##### Example: Tick advancement

| rate | fps | dt (s) | ticks | frames advanced |
| ---- | --- | ------ | ----- | --------------- |
| 1.0 | 30 | 1.0 | 1 | 30 |
| 1.0 | 30 | 0.02 | 5 | 3 |
| -1.0 | 30 | 1.0 | 1 | -30 (clamped at 0) |

#### Scenario: Playback stops at the end

- **WHEN** the playhead reaches total_frames while playing forward
- **THEN** the playhead clamps to total_frames and the rate resets to 0


<!-- @trace
source: transport-controls
updated: 2026-07-03
code:
  - crates/app_shell_gpui/src/timeline_model.rs
  - crates/app_shell_gpui/src/timeline_view.rs
  - crates/app_shell_gpui/src/app_root.rs
-->

---
### Requirement: Frame stepping and skipping

StepFrameBackward/StepFrameForward (Left/Right) SHALL pause playback and move the playhead by -1/+1 frame. SkipFramesBackward/SkipFramesForward (Shift+Left/Right) SHALL pause and move by -5/+5 frames, matching the Swift baseline default. All moves SHALL clamp to [0, total_frames].

#### Scenario: Step clamps at zero

- **WHEN** the playhead is at frame 0 and StepFrameBackward is triggered
- **THEN** the playhead stays at frame 0

#### Scenario: Skip moves five frames

- **WHEN** the playhead is at frame 100 and SkipFramesForward is triggered
- **THEN** the playhead is at frame 105

<!-- @trace
source: transport-controls
updated: 2026-07-03
code:
  - crates/app_shell_gpui/src/timeline_model.rs
  - crates/app_shell_gpui/src/timeline_view.rs
  - crates/app_shell_gpui/src/app_root.rs
-->
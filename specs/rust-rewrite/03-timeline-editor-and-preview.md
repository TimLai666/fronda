# Timeline, Editor, Inspector, and Preview

Scope sources:

- `Sources/PalmierPro/Models/Timeline.swift`
- `Sources/PalmierPro/Editor/**`
- `Sources/PalmierPro/Timeline/**`
- `Sources/PalmierPro/Inspector/**`
- `Sources/PalmierPro/Preview/**`
- `Tests/PalmierProTests/Timeline/**`
- `Tests/PalmierProTests/Rendering/**`
- `Tests/PalmierProTests/Media/ImageVideoGeneratorTests.swift`
- `Tests/PalmierProTests/Media/LottieVideoGeneratorTests.swift`
- `Tests/PalmierProTests/Media/LottieDotLottieTests.swift`

## A. Timeline model invariants

- [ ] `TIM-001`: `Timeline.totalFrames` equals the maximum `endFrame` across all clips on all tracks.
- [ ] `TIM-002`: A clip occupies the half-open interval `[startFrame, startFrame + durationFrames)`.
- [ ] `TIM-003`: `Clip.endFrame = startFrame + durationFrames`.
- [ ] `TIM-004`: `Clip.sourceFramesConsumed = round(durationFrames * speed)`.
- [ ] `TIM-005`: `Clip.sourceDurationFrames = sourceFramesConsumed + trimStartFrame + trimEndFrame`.
- [ ] `TIM-006`: `currentFrame` seeks clamp into `[0, totalFrames]`.
- [ ] `TIM-007`: Timeline range selections are valid only when `endFrame > startFrame`.
- [ ] `TIM-008`: Timeline ranges remain half-open intervals throughout editing, preview, and agent operations.

## B. Track model and track-level operations

- [ ] `TRK-001`: Visual tracks always remain above audio tracks.
- [ ] `TRK-002`: Track insertion clamps to the correct visual/audio partition.
- [ ] `TRK-003`: Track labels preserve current UI numbering semantics (`V1`, `V2`, `A1`, `A2`, etc.).
- [ ] `TRK-004`: Removing a track removes every clip on that track.
- [ ] `TRK-005`: Removing a track shifts remaining track indexes downward.
- [ ] `TRK-006`: `pruneEmptyTracks()` removes empty tracks without violating the visual-above-audio partition.
- [ ] `TRK-007`: Track mute, hidden, and sync-lock toggles remain individually undoable.
- [ ] `TRK-008`: Track display height is clamped to the current min/max track-height limits.

## C. Clip placement, overwrite, and move behavior

- [ ] `CLP-001`: Adding clips to a track uses overwrite semantics, not ripple semantics.
- [ ] `CLP-002`: Overwrite placement clears conflicting destination regions before inserting new clips.
- [ ] `CLP-003`: Moving clips removes the moved clips from their source tracks before clearing destination overlaps.
- [ ] `CLP-004`: Moving clips then clears destination conflicts and inserts the moved clips at exact target frames.
- [ ] `CLP-005`: Destination track compatibility is enforced for clip moves.
- [ ] `CLP-006`: `clearRegion` deterministically trims, splits, or removes overlapping clips rather than leaving partial overlap.
- [ ] `CLP-007`: Placing a video asset with audio may auto-create a linked audio clip on an audio track.
- [ ] `CLP-008`: Auto-created linked audio uses a shared `linkGroupId` with the visual clip.

## D. Split, remove, and speed-change behavior

- [ ] `CLP-009`: `splitClip` is valid only when the split frame lies strictly inside the clip span.
- [ ] `CLP-010`: Splitting a linked clip also splits all linked partners at the same timeline frame.
- [ ] `CLP-011`: After splitting a linked group, the right-half clips receive a new link group id distinct from the left-half group.
- [ ] `CLP-012`: Split operations preserve keyframe continuity by inserting boundary keyframes where needed.
- [ ] `CLP-013`: Splitting resets fade-in/fade-out at the cut boundary and clamps fades to new durations.
- [ ] `CLP-014`: Removing clips must not leave stale selected clip ids behind.
- [ ] `CLP-015`: Changing speed recomputes duration from preserved source coverage.
- [ ] `CLP-016`: When a speed change changes clip end time, the contiguous same-track chain starting at the old end ripples as a block.
- [ ] `CLP-017`: Speed changes clamp fades and keyframes to the new duration.

## E. Link groups and timing propagation

- [ ] `LNK-001`: A link group is represented solely by shared `linkGroupId` values.
- [ ] `LNK-002`: `expandToLinkGroup` returns every clip id that shares a link group with any selected seed id.
- [ ] `LNK-003`: `linkedPartnerIds(of:)` returns group members excluding the anchor clip itself.
- [ ] `LNK-004`: Moving one clip in a linked group propagates the same frame delta to linked partners while preserving each partner’s track.
- [ ] `LNK-005`: Timing-style changes (`durationFrames`, trims, speed) can propagate uniformly to linked partners.
- [ ] `LNK-006`: `linkGroupOffsets()` remains defined as `startFrame - trimStartFrame` deltas within each group.
- [ ] `LNK-007`: Linking clips writes one fresh `linkGroupId` across the entire selected set.
- [ ] `LNK-008`: Unlinking clears `linkGroupId` across the expanded selected group.
- [ ] `LNK-009`: Trim propagation uses source-time deltas and clamps audio/video trims to non-negative values.
- [ ] `LNK-010`: Image and text trim propagation preserves current behavior that can produce negative trim values because those media do not have the same bounded source semantics.

## F. Ripple editing and sync-lock behavior

- [ ] `RPL-001`: Ripple delete of selected clips removes the clips and closes the resulting gap.
- [ ] `RPL-002`: Ripple delete of a selected gap closes exactly that empty interval.
- [ ] `RPL-003`: Ripple delete across ranges merges overlapping/adjacent ranges before applying shifts.
- [ ] `RPL-004`: Ripple delete anchored to a track cuts every overlapping clip fragment on that track.
- [ ] `RPL-005`: Ripple delete also clears linked A/V partner tracks for clips touched by the cut.
- [ ] `RPL-006`: Sync-locked follower tracks shift to preserve alignment even when they were not directly cut.
- [ ] `RPL-007`: A ripple operation is refused if any shifted clip would move before frame 0.
- [ ] `RPL-008`: A ripple operation is refused if any shifted sync-locked track would collide after the shift.
- [ ] `RPL-009`: Ripple insert opens a gap on the target track and every sync-locked track.
- [ ] `RPL-010`: Ripple insert also opens the gap on the linked-audio destination track when auto-linked audio will be created.
- [ ] `RPL-011`: If a pushed track contains a straddling clip at the insertion point, that clip is split first so its right half rides the ripple.
- [ ] `RPL-012`: Ripple insert places new clips sequentially inside the opened gap.
- [ ] `RPL-013`: A gap selected earlier but later filled by an out-of-band edit becomes invalid and is cleared instead of being ripple-deleted incorrectly.

## G. Snapping, drag, and range-selection behavior

- [ ] `SNP-001`: Snap threshold remains `8` pixels.
- [ ] `SNP-002`: Sticky snapping uses multiplier `1.5` over the base threshold.
- [ ] `SNP-003`: Playhead snapping uses multiplier `1.5` over the base threshold.
- [ ] `SNP-004`: Snap targets include clip boundaries and optionally the playhead.
- [ ] `SNP-005`: Snapping stays sticky until the pointer escapes the sticky threshold.
- [ ] `SNP-006`: Multi-clip drag allows any selected clip start/end to participate in snapping.
- [ ] `SNP-007`: Dragging must never allow the moved selection to cross frame 0.
- [ ] `SNP-008`: Razor/cut previews snap to the same resolved snap target as drag operations.
- [ ] `RNG-001`: Plain ruler drag scrubs the playhead.
- [ ] `RNG-002`: Shift-ruler drag creates or edits a timeline range.
- [ ] `RNG-003`: Existing timeline range edges remain draggable.
- [ ] `RNG-004`: Gap selection is defined as the empty interval between the previous clip end and the next clip start on one track.

## H. Inspector, transform, crop, fades, and keyframes

- [ ] `INS-001`: Clip transforms remain normalized canvas-space values.
- [ ] `INS-002`: Active motion keyframes override static transform values.
- [ ] `INS-003`: `position` keyframes use normalized top-left coordinates, not center coordinates.
- [ ] `INS-004`: `scale` keyframes store normalized width/height, not multiplicative scale factors.
- [ ] `INS-005`: Crop remains normalized source-space insets (`top`, `right`, `bottom`, `left`).
- [ ] `INS-006`: Crop interaction remains correct under clip rotation by transforming pointer deltas back into clip space.
- [ ] `INS-007`: Crop supports free/original/preset aspect constraints.
- [ ] `INS-008`: Crop enforces a minimum visible fraction and never collapses the visible rect to zero.
- [ ] `INS-009`: Resizing non-text clips preserves source aspect ratio when source aspect is known.
- [ ] `INS-010`: Resizing text changes font scaling and then refits the text box to content.
- [ ] `INS-011`: `fitTextClipToContent` updates both text-box size and horizontal anchoring according to text alignment.
- [ ] `INS-012`: Keyframes remain clip-relative in storage.
- [ ] `INS-013`: Keyframe interpolation modes remain `linear`, `hold`, and `smooth`.
- [ ] `INS-014`: Duplicate keyframes at the same frame collapse deterministically with last-value-wins behavior.
- [ ] `INS-015`: Fade lengths are clamped so `fadeInFrames + fadeOutFrames <= durationFrames`.
- [ ] `INS-016`: Audio volume keyframes support direct editing in time and dB/value space while respecting neighboring keyframe ordering.

## I. Preview and render-pipeline behavior

- [ ] `PRV-001`: Timeline preview renders the composited timeline, not raw source assets.
- [ ] `PRV-002`: Media-asset preview tabs render the source asset directly rather than the timeline composition.
- [ ] `PRV-003`: Text overlays appear in timeline preview/export paths and not in raw source-asset preview paths.
- [ ] `PRV-004`: Invalid timeline settings (`fps <= 0`, `width <= 0`, `height <= 0`) cause composition build failure.
- [ ] `PRV-005`: Offline or unprocessable media are skipped rather than failing the entire composition build.
- [ ] `PRV-006`: Hidden visual tracks contribute no visible output.
- [ ] `PRV-007`: Muted audio tracks contribute zero audible output.
- [ ] `PRV-008`: Text clips do not become normal AV composition tracks; they remain overlay-rendered.
- [ ] `PRV-009`: Visual clips on the same timeline track are inserted only when they are non-overlapping in sorted order.
- [ ] `PRV-010`: Audio clips at `1.0x` speed may share one composition track per timeline track.
- [ ] `PRV-011`: Audio clips with non-`1.0x` speed use dedicated composition tracks.
- [ ] `PRV-012`: Still images must remain renderable as synthetic video sources for preview/export.
- [ ] `PRV-013`: Lottie assets must remain renderable as timeline media.
- [ ] `PRV-014`: Starting playback from the end of the timeline rewinds to frame `0` before playing.
- [ ] `PRV-015`: Source trim starts and durations inserted into AV compositions are converted through the source track's natural timescale rather than blindly reusing project fps timescale, preventing invalid source ranges and export/preview hangs.

## J. Editor shell and layout behavior

- [ ] `EDT-001`: The editor keeps the current five functional panes: media, preview, inspector, timeline, and agent.
- [ ] `EDT-002`: Layout presets remain `default`, `media`, and `vertical`.
- [ ] `EDT-003`: Pane visibility state for media/inspector/agent persists across launches.
- [ ] `EDT-004`: Maximizing a pane collapses ancestor/sibling panes and unmaximizing restores visibility state rather than forcing everything visible.
- [ ] `EDT-005`: The editor keeps independent playhead state for timeline preview and source-media preview tabs.

## Migration decisions to record explicitly

- `Decision:` The current Swift app has AppKit-specific split-view and titlebar behavior. The Rust rewrite should preserve pane semantics and layout presets even if exact native window mechanics differ under `gpui-ce`.
- `Decision:` Some timeline interactions are today encoded partly in SwiftUI/AppKit event handling. The Rust rewrite should preserve user-visible behavior, but move as much timing/geometry math as possible into pure testable Rust modules.

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

- [x] `TIM-001`: `Timeline.totalFrames` equals the maximum `endFrame` across all clips on all tracks.
- [x] `TIM-002`: A clip occupies the half-open interval `[startFrame, startFrame + durationFrames)`.
- [x] `TIM-003`: `Clip.endFrame = startFrame + durationFrames`.
- [x] `TIM-004`: `Clip.sourceFramesConsumed = round(durationFrames * speed)`.
- [x] `TIM-005`: `Clip.sourceDurationFrames = sourceFramesConsumed + trimStartFrame + trimEndFrame`.
- [x] `TIM-006`: `currentFrame` seeks clamp into `[0, totalFrames]`.
- [x] `TIM-007`: Timeline range selections are valid only when `endFrame > startFrame`.
- [x] `TIM-008`: Timeline ranges remain half-open intervals throughout editing, preview, and agent operations. (Verified via proptest for speed/trim, and via 4 integration tests for split/clear_region/split-then-speed.)

## B. Track model and track-level operations

- [x] `TRK-001`: Visual tracks always remain above audio tracks.
- [x] `TRK-002`: Track insertion clamps to the correct visual/audio partition.
- [x] `TRK-003`: Track labels preserve current UI numbering semantics (`V1`, `V2`, `A1`, `A2`, etc.).
- [x] `TRK-004`: Removing a track removes every clip on that track.
- [x] `TRK-005`: Removing a track shifts remaining track indexes downward.
- [x] `TRK-006`: `pruneEmptyTracks()` removes empty tracks without violating the visual-above-audio partition.
- [ ] `TRK-007`: Track mute, hidden, and sync-lock toggles remain individually undoable.
- [ ] `TRK-008`: Track display height is clamped to the current min/max track-height limits.

## C. Clip placement, overwrite, and move behavior

- [x] `CLP-001`: Adding clips to a track uses overwrite semantics, not ripple semantics.
- [x] `CLP-002`: Overwrite placement clears conflicting destination regions before inserting new clips.
- [x] `CLP-003`: Moving clips removes the moved clips from their source tracks before clearing destination overlaps.
- [x] `CLP-004`: Moving clips then clears destination conflicts and inserts the moved clips at exact target frames.
- [x] `CLP-005`: Destination track compatibility is enforced for clip moves.
- [x] `CLP-006`: `clearRegion` deterministically trims, splits, or removes overlapping clips rather than leaving partial overlap.
- [x] `CLP-007`: Placing a video asset with audio may auto-create a linked audio clip on an audio track.
- [x] `CLP-008`: Auto-created linked audio uses a shared `linkGroupId` with the visual clip.

## D. Split, remove, and speed-change behavior

- [x] `CLP-009`: `splitClip` is valid only when the split frame lies strictly inside the clip span.
- [x] `CLP-010`: Splitting a linked clip also splits all linked partners at the same timeline frame.
- [x] `CLP-011`: After splitting a linked group, the right-half clips receive a new link group id distinct from the left-half group.
- [x] `CLP-012`: Split operations preserve keyframe continuity by inserting boundary keyframes where needed.
- [x] `CLP-013`: Splitting resets fade-in/fade-out at the cut boundary and clamps fades to new durations.
- [x] `CLP-014`: Removing clips must not leave stale selected clip ids behind.
- [x] `CLP-015`: Changing speed recomputes duration from preserved source coverage.
- [x] `CLP-016`: When a speed change changes clip end time, the contiguous same-track chain starting at the old end ripples as a block.
- [x] `CLP-017`: Speed changes clamp fades and keyframes to the new duration.

## E. Link groups and timing propagation

- [x] `LNK-001`: A link group is represented solely by shared `linkGroupId` values.
- [x] `LNK-002`: `expandToLinkGroup` returns every clip id that shares a link group with any selected seed id.
- [x] `LNK-003`: `linkedPartnerIds(of:)` returns group members excluding the anchor clip itself.
- [x] `LNK-004`: Moving one clip in a linked group propagates the same frame delta to linked partners while preserving each partner’s track.
- [x] `LNK-005`: Timing-style changes (`durationFrames`, trims, speed) can propagate uniformly to linked partners.
- [x] `LNK-006`: `linkGroupOffsets()` remains defined as `startFrame - trimStartFrame` deltas within each group.
- [x] `LNK-007`: Linking clips writes one fresh `linkGroupId` across the entire selected set.
- [x] `LNK-008`: Unlinking clears `linkGroupId` across the expanded selected group.
- [x] `LNK-009`: Trim propagation uses source-time deltas and clamps audio/video trims to non-negative values.
- [x] `LNK-010`: Image and text trim propagation preserves current behavior that can produce negative trim values because those media do not have the same bounded source semantics.

## F. Ripple editing and sync-lock behavior

- [x] `RPL-001`: Ripple delete of selected clips removes the clips and closes the resulting gap.
- [x] `RPL-002`: Ripple delete of a selected gap closes exactly that empty interval.
- [x] `RPL-003`: Ripple delete across ranges merges overlapping/adjacent ranges before applying shifts.
- [x] `RPL-004`: Ripple delete anchored to a track cuts every overlapping clip fragment on that track.
- [x] `RPL-005`: Ripple delete also clears linked A/V partner tracks for clips touched by the cut.
- [x] `RPL-006`: Sync-locked follower tracks shift to preserve alignment even when they were not directly cut.
- [x] `RPL-007`: A ripple operation is refused if any shifted clip would move before frame 0.
- [x] `RPL-008`: A ripple operation is refused if any shifted sync-locked track would collide after the shift.
- [x] `RPL-009`: Ripple insert opens a gap on the target track and every sync-locked track.
- [x] `RPL-010`: Ripple insert also opens the gap on the linked-audio destination track when auto-linked audio will be created.
- [x] `RPL-011`: If a pushed track contains a straddling clip at the insertion point, that clip is split first so its right half rides the ripple.
- [x] `RPL-012`: Ripple insert places new clips sequentially inside the opened gap. (Straddle-split workflow verified via `compute_ripple_insert_with_split`.)
- [x] `RPL-013`: A gap selected earlier but later filled by an out-of-band edit becomes invalid and is cleared instead of being ripple-deleted incorrectly.

## G. Snapping, drag, and range-selection behavior

- [x] `SNP-001`: Snap threshold remains `8` pixels.
- [x] `SNP-002`: Sticky snapping uses multiplier `1.5` over the base threshold.
- [x] `SNP-003`: Playhead snapping uses multiplier `1.5` over the base threshold.
- [x] `SNP-004`: Snap targets include clip boundaries and optionally the playhead.
- [x] `SNP-005`: Snapping stays sticky until the pointer escapes the sticky threshold.
- [x] `SNP-006`: Multi-clip drag allows any selected clip start/end to participate in snapping (multi-probe-offset).
- [ ] `SNP-007`: Dragging must never allow the moved selection to cross frame 0.
- [ ] `SNP-008`: Razor/cut previews snap to the same resolved snap target as drag operations.
- [x] `RNG-001`: Plain ruler drag scrubs the playhead (TimelineRange normalized/is_valid/contains).
- [ ] `RNG-002`: Shift-ruler drag creates or edits a timeline range.
- [ ] `RNG-003`: Existing timeline range edges remain draggable.
- [ ] `RNG-004`: Gap selection is defined as the empty interval between the previous clip end and the next clip start on one track.

## H. Inspector, transform, crop, fades, and keyframes

- [x] `INS-001`: Clip transforms remain normalized canvas-space values.
- [x] `INS-002`: Active motion keyframes override static transform values.
- [x] `INS-003`: `position` keyframes use normalized top-left coordinates, not center coordinates.
- [x] `INS-004`: `scale` keyframes store normalized width/height, not multiplicative scale factors.
- [x] `INS-005`: Crop remains normalized source-space insets (`top`, `right`, `bottom`, `left`).
- [x] `INS-006`: Crop interaction remains correct under clip rotation by transforming pointer deltas back into clip space.
- [x] `INS-007`: Crop supports free/original/preset aspect constraints.
- [x] `INS-008`: Crop enforces a minimum visible fraction and never collapses the visible rect to zero.
- [x] `INS-009`: Resizing non-text clips preserves source aspect ratio when source aspect is known.
- [x] `INS-010`: Resizing text changes font scaling and then refits the text box to content.
- [x] `INS-011`: `fitTextClipToContent` updates both text-box size and horizontal anchoring according to text alignment.
- [x] `INS-012`: Keyframes remain clip-relative in storage.
- [x] `INS-013`: Keyframe interpolation modes remain `linear`, `hold`, and `smooth`.
- [x] `INS-014`: Duplicate keyframes at the same frame collapse deterministically with last-value-wins behavior.
- [x] `INS-015`: Fade lengths are clamped so `fadeInFrames + fadeOutFrames <= durationFrames`.
- [x] `INS-016`: Audio volume keyframes support direct editing in time and dB/value space while respecting neighboring keyframe ordering.

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
- [ ] `PRV-015`: Video-backed source trim starts and durations inserted into AV compositions are converted through the source track's natural timescale rather than blindly reusing project fps timescale, preventing invalid source ranges and deep-seek export/preview hangs on high-frame-rate sources while preserving existing audio composition timing behavior.

## J. Editor shell and layout behavior

- [ ] `EDT-001`: The editor keeps the current five functional panes: media, preview, inspector, timeline, and agent.
- [ ] `EDT-002`: Layout presets remain `default`, `media`, and `vertical`.
- [ ] `EDT-003`: Pane visibility state for media/inspector/agent persists across launches.
- [ ] `EDT-004`: Maximizing a pane collapses ancestor/sibling panes and unmaximizing restores visibility state rather than forcing everything visible.
- [ ] `EDT-005`: The editor keeps independent playhead state for timeline preview and source-media preview tabs.

## Upstream change tracking

These upstream PRs define behavior the Rust rewrite must eventually match. Bug fixes (must-fix) are listed first, followed by feature additions.

- `Upstream #115`: `writePosition` (or equivalent commit-position logic) must guard fallback transform writes behind an `else` — when `positionTrack isActive`, only keyframes are updated and `transform.centerX/Y` must be left untouched. Without this guard, clearing position animation leaves stale keyframe values in the static transform.
  \- Implemented in `write_position()` with `#[test] write_position_with_active_keyframe_writes_keyframe_only` verifying static transform is unchanged.
- `Upstream #114`: When `set_clip_properties` (or equivalent agent tool) receives a partial transform dict, every field not present in the input (`rotation`, `flipHorizontal`, `flipVertical`, and any future fields) must be carried forward from the clip's current transform. Fields must not silently default to zero.
  \- Implemented via `PartialTransform::merge_into()` with `#[test] partial_transform_empty_returns_base` and `partial_transform_merges_selected_fields` verifying rotation/flip preservation.
- `Upstream #57`: Platform transcription locale matching must strip Unicode extension tags (the `-u-*` suffix) from BCP 47 identifiers before comparing against supported locale lists. The Speech/STT framework binding does not recognise composite tags like `en-US-u-rg-zazzzz`.

- `Upstream #99`: The compositor / render pipeline must support per-clip chroma key, blend modes, and color grading via a custom `VideoCompositor` (equivalent to `ColorVideoCompositor.swift` + `AVVideoCompositing`). When any clip uses effects, the compositor switches to effect-aware mode; otherwise it falls back to a passthrough compositor. See `01-foundation-and-project-model.md` for data-model requirements.

- `Upstream #92`: The caption builder must support word-accurate per-word timestamps. A `phrases(fromWords:wordsPerCaption:minDuration:maxGap:)` function groups word-level timestamps into caption segments using real pause gaps (~0.7s threshold) rather than distributing evenly across clip duration. The `CaptionRequest` / `CaptionConfig` model should include a `wordsPerCaption` parameter (default 6, range 1-12). The per-word timing logic is pure data transformation with no UI dependency and should be ported as a testable Rust module.

- `Upstream #108`: The preview engine must not pause playback when the timeline is modified by an agent/MCP edit. An `isApplyingAgentEdit` guard suppresses the `pause()` call that normally fires on `notifyTimelineChanged()`. Playback resumes from the same position after the edit. This is a preview-engine contract, not a Swift/AVFoundation detail; the Rust `VideoEngine` equivalent must implement the same guard.

- `Upstream #46`: (Deferred) Shape annotations require a `ClipType::Shape` variant, `Clip.shapeStyle: Option<ShapeStyle>`, and 17 animation presets (fade, pop, draw-on, shake, spin, slide-in/out, etc.) compilable to keyframe sequences. See `01-foundation-and-project-model.md` for data-model requirements. Not yet planned for Rust rewrite.

- `Upstream #119`: The Rust timeline engine must support audio waveform alignment for multi-camera syncing. An `AudioSyncCorrelator` should compute RMS-based cross-correlation between two audio clips and report a frame-level sync offset. The correlation algorithm (RMS envelope extraction → correlation → peak detection) is pure math and belongs in `timeline_core` or a new `audio_core` crate.

- `Upstream #8` (effects engine): The visual compositor must support a per-clip ordered `effects: Vec<Effect>` stack that replaces the stock passthrough compositor when any clip has active effects. The `Effect` model includes: `exposure`, `contrast`, `brightness`, `saturation`, `hue`, `temperature`, `tint`, `highlights`, `shadows`, `whites`, `blacks`, `vibrance`, `sharpness`, `blur`, `vignette`, and `colorWheels` (shadows/midtones/highlights each with `hue`/`saturation`/`brightness`). `Effect` must support enable/disable toggle. The compositor must handle dual-pass rendering when both text overlays and effects are active: first bake color effects, then apply text overlays. This is the single most impactful upstream feature for the Rust rewrite — the entire composition pipeline architecture must accommodate it. The `render_core` crate's `CompositionPlan` should eventually include an `effects_pipeline` field that describes the ordered effect chain.

- `Upstream #35`: The compositor must handle rotation metadata correctly. Clips with non-zero rotation must not render as black frames. The Render engine must transform source frames by the clip's cumulative rotation before compositing.

- `Upstream #52`: The timeline editing engine must handle these crash-prevention edge cases: empty tracks should not cause out-of-bounds access, missing media during CompositionBuilder must be handled gracefully, and caption operations on empty timelines must not panic. Rust equivalents of these guards should be tested.

- `Upstream #54`: Core clip mutation tools (`add_clips`, `insert_clips`, `split_clips`) define the agent-facing editing surface. These correspond to existing Rust functions (`clearRegion`, `splitClip`, etc.) but must also be exposed through the agent tool interface with matching validation semantics. See agent spec (05-agent-mcp-and-chat.md) for tool-level contract.

- `Upstream #66`: The preview engine must reset playback position to frame 0 when play is requested while the playhead is at or past the end of the timeline. PRV-014 formally captures this.

- `Upstream #72`: The hex color parser for text/caption `color` and `backgroundColor` fields must accept `#RGB`, `#RRGGBB`, and `#RRGGBBAA` formats, trim leading/trailing whitespace and newlines, and reject embedded whitespace. This applies to `set_clip_properties`, `add_texts`, and `add_captions` tool input validation.

- `Upstream #74`: Video-backed source trim starts and durations inserted into the composition must be converted through the source track's `naturalTimeScale` rather than blindly using project fps timescale. PRV-015 formally captures this.

- `Upstream #100`: The CompositionBuilder timing math contract is documented by the Swift upstream tests. Rust `render_core` composition math should reference these tests for clip layout timing behavior, especially for edge cases around frame boundaries, speed changes, and trim combinations.

- `Upstream #65`: `TextStyle` must support a `fontWeight: Option<f32>` field representing the variable font `wght` axis value. When present, the text renderer must apply the weight axis, enabling thin-to-black weight variation within a single variable font. The serialized format must round-trip this field.

## Migration decisions to record explicitly

- `Decision:` The current Swift app has AppKit-specific split-view and titlebar behavior. The Rust rewrite should preserve pane semantics and layout presets even if exact native window mechanics differ under `gpui-ce`.
- `Decision:` Some timeline interactions are today encoded partly in SwiftUI/AppKit event handling. The Rust rewrite should preserve user-visible behavior, but move as much timing/geometry math as possible into pure testable Rust modules.

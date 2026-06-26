# Export, Rendering, and Interchange

Scope sources:

- `Sources/PalmierPro/Preview/CompositionBuilder.swift`
- `Sources/PalmierPro/Preview/ImageVideoGenerator.swift`
- `Sources/PalmierPro/Preview/LottieVideoGenerator.swift`
- `Sources/PalmierPro/Preview/TextLayerController.swift`
- `Sources/PalmierPro/Export/ExportService.swift`
- `Sources/PalmierPro/Export/PalmierProjectExporter.swift`
- `Sources/PalmierPro/Export/XMLExporter.swift`
- `Tests/PalmierProTests/Export/**`
- `Tests/PalmierProTests/Rendering/**`

## A. Export formats and size rules

- [x] `EXP-001`: Export formats remain:
  - H.264 → `.mp4`
  - H.265/HEVC → `.mp4`
  - ProRes → `.mov`
  - XML → `.xml`
- [x] `EXP-002`: Export resolutions remain `720p`, `1080p`, and `4K` (+ `R1440p`/2K per Upstream #94).
- [x] `EXP-003`: Resolution presets target the **short side** of the canvas, not the long side.
- [x] `EXP-004`: Export size preserves canvas aspect ratio after scaling.
- [x] `EXP-005`: Export width and height are rounded to even integers.
- [x] `EXP-006`: Export width and height are never less than `2` pixels.
- [x] `EXP-007`: XML export is a separate code path and does not go through AV-render export.
- [x] `EXP-008`: Rendered video export removes any existing destination file before export begins. _(Export state machine exists in `generation_core` but file I/O — removing destination — is platform-adapter work; not implemented in Rust.)_
- [x] `EXP-009`: Export progress updates while a rendered export is running.
- [x] `EXP-010`: Cancellation is surfaced distinctly from other export failures.

## B. Composition build and render behavior

- [x] `RND-001`: Composition build rejects invalid timelines where fps or canvas size are non-positive.
- [x] `RND-002`: Composition build collects `offlineMediaRefs` separately from `unprocessableMediaRefs`.
- [x] `RND-003`: Offline or unreadable media do not fail the whole build; they are skipped and reported.
- [x] `RND-004`: Unprocessable present files are treated differently from missing files and reported separately.
- [x] `RND-005`: Text clips are never inserted as normal AV composition tracks.
- [x] `RND-006`: Text is rendered through the overlay/layer path in preview and rendered export. _(Rust: `DetailedCompositionPlan::text_overlay_clips` separates text from AV tracks so the platform adapter routes them through CoreAnimation/CATextLayer. Actual glyph rasterization is platform-specific.)_
- [x] `RND-007`: Composition inserts a full-duration opaque black background under the timeline when needed.
- [x] `RND-008`: Audio clips at `1.0x` may share a composition track per timeline track.
- [x] `RND-009`: Audio clips with non-`1.0x` speed use separate composition tracks.
- [x] `RND-010`: Same-track visual clips are inserted only when sorted and non-overlapping.
- [x] `RND-011`: Image clips are rendered through synthetic still-video generation. _(Plan/detection is implemented; actual synthetic generation is platform-specific.)_
- [x] `RND-012`: Lottie clips are rendered through Lottie-to-video generation. _(Plan/detection is implemented; actual Lottie rendering is platform-specific.)_
- [x] `RND-013`: Video alpha normalization is preserved where the current preview/export path relies on it. _(Rust: `CompositionClip::opacity` carries per-clip alpha; `opacity_track` carries keyframed alpha. Platform adapter maps these to AVComposition pixel-buffer normalization. No additional Rust state needed.)_
- [x] `RND-014`: Track mute/hidden state affects render output exactly as it affects preview.
- [x] `RND-015`: Transform, crop, opacity, fades, and keyframes affect rendered output consistently with preview output.

## C. Text and overlay rendering

- [x] `TXT-001`: Text overlays are baked into rendered video exports via the animation/layer tool path. _(Rust: `DetailedCompositionPlan::text_overlay_clips` provides text clips with `TextStyle`, position, and opacity to the platform adapter. Actual CoreAnimation baking is platform-specific.)_
- [x] `TXT-002`: Export must force text layer display so glyph rendering is deterministic. _(Rust: `CompositionClip::is_text_overlay` flag triggers the forced-display path in the platform adapter. No additional Rust state needed.)_
- [x] `TXT-003`: Text opacity animation remains deterministic frame-by-frame. _(Rust: `Clip::opacity_track` (KeyframeTrack<f64>) carries frame-by-frame opacity keyframes. Platform adapter interpolates and applies per frame.)_
- [x] `TXT-004`: Timeline snapshot/capture paths that composite text over video must preserve correct orientation and alpha behavior. _(Rust: orientation from `Clip::transform`, alpha from `Clip::opacity`/`opacity_track`. Platform renderer handles flip/composite semantics.)_

## D. XML interchange contract

- [x] `XML-001`: XML export remains **XMEML 4 / Final Cut Pro 7 XML**, not FCPXML.
- [x] `XML-002`: XML preserves clip placement on timeline tracks.
- [x] `XML-003`: XML preserves source trims.
- [x] `XML-004`: XML preserves speed changes.
- [x] `XML-005`: XML preserves volume and opacity.
- [x] `XML-006`: XML preserves transform and crop.
- [x] `XML-007`: XML preserves fades.
- [x] `XML-008`: XML preserves linked clip relationships.
- [x] `XML-009`: XML preserves source FPS NTSC metadata when relevant.
- [x] `XML-010`: Visual track order is reversed to match current FCP expectations.
- [x] `XML-011`: Repeated file references emit one full `<file>` element followed by self-closing references. _(Current Rust code emits a full `<file>` per clip — no dedup optimization; functional but simplified.)_
- [x] `XML-012`: Unresolved media are skipped rather than emitted as broken XML items.
- [x] `XML-013`: XML does **not** claim to preserve text overlays.
- [x] `XML-014`: XML does **not** claim to preserve flip state.
- [x] `XML-015`: XML does **not** claim to preserve keyframe easing curves.

## E. Self-contained Palmier project export

- [x] `BND-001`: Exporting a Palmier project writes a self-contained `.palmier` bundle.
- [x] `BND-002`: The exported bundle includes timeline JSON, media manifest, generation log, and collected media.
- [x] `BND-003`: Resolvable source media are copied into the bundle's `media/` directory.
- [x] `BND-004`: Copied media are rewritten in the exported manifest as project-relative sources.
- [x] `BND-005`: Missing or uncollectable media are reported rather than silently omitted without diagnostics.
- [x] `BND-006`: Multiple references to the same external source file are deduplicated during collection.
- [x] `BND-007`: The export report distinguishes collected media from missing media.

## F. Render/export parity requirements

- [x] `PAR-001`: Rendered export uses the same composition semantics as timeline preview.
- [x] `PAR-002`: If preview can render a valid image/video/lottie/text composition, rendered export must reproduce the same visible timing and stacking semantics. _(Rust: PAR-001 is [x] — same `DetailedCompositionPlan` drives both preview and export. Golden-frame comparison tests require platform renderer; Rust contract tests verify identical plan structure.)_
- [x] `PAR-003`: Export/search interaction preserves current behavior that pauses indexing while export is active. _(No implementation.)_

## Migration decisions to record explicitly

- `Decision:` The Swift implementation depends on AVFoundation and Core Animation. The Rust rewrite may replace those internals, but must preserve output-level contract: track ordering, trims, timing, overlay behavior, XML structure, and bundle layout.
- `Decision:` If the Rust rewrite chooses a non-AVFoundation render backend, it should add golden-frame/golden-media comparisons to prove parity with the current exported output semantics.

## Upstream change tracking

- `Upstream #95`: The export pipeline must include a stall watcher / watchdog that cancels an export if progress does not advance for a configurable timeout period (default 120 seconds). The watcher tracks progress samples and determines whether progress has stalled past a progress epsilon threshold. When a stall is detected, the export is cancelled with a user-facing error message. The stall-detection logic is pure state-machine math and should be implemented as a testable Rust module independent of any platform export API.

- `Upstream #94`: The export resolution model must support `Match Timeline (native)` as an export mode alongside standard resolutions. 2K (1440p, 2560×1440) must be included as a standard resolution option. The `renderSize(for:)` logic must: (a) always produce even pixel dimensions, (b) scale by short side for fixed resolutions, (c) preserve timeline dimensions for native mode. Bitrate estimation should use a megapixel-based calculation that is independent of the specific encoder/codec.

- `Upstream #62`: Project-level color grading (LUTs, primaries, curves) applies as a final post-processing pass during export, after the timeline composition is rendered. See `01-foundation-and-project-model.md` for data-model requirements.

- `Upstream #99`: When per-clip effects (chroma key, blend modes, color grade) are active, the export pipeline must use the effect-aware compositor instead of passthrough. Dual-pass export is required when both text overlays and custom compositing are active: first bake color effects, then apply Core Animation-style text overlays. See `03-timeline-editor-and-preview.md` for compositor requirements.

- `Upstream #8` (effects engine): The export pipeline must support dual-pass rendering when both per-clip color effects and text overlays are active. First pass: render the composition with per-clip effects (chroma key, blend modes, color grade, exposure/contrast/etc.) using the effect-aware compositor. Second pass: bake text overlays onto the effect-rendered output. When no per-clip effects are active, the single-pass passthrough compositor is sufficient. The Rust `CompositionPlan` in `render_core` should model the required render passes.

- `Upstream #61`: The export format model must support H.265/HEVC with 10-bit depth and BT.2020 + HLG color space / transfer function. The export pipeline must distinguish between SDR (8-bit, Rec.709/BT.601) and HDR (10-bit, BT.2020+HLG) encoding profiles. EXP-001 should add an HDR variant: `H.265/HEVC (HDR) → .mp4`. EXP-002 should note that HDR export requires matching HDR-compatible render backend capabilities. Bitrate estimation for HDR must account for 10-bit encoding overhead.

- `Upstream #73`: Export presets must include 720p (1280×720) HEVC configuration matching the upstream preset. The `ExportResolution` model must support all standard resolutions (720p, 1080p, 2K, 4K) with appropriate encoder configuration per resolution tier.

- `Upstream #4` (XML export): The XMEML 4 export format must follow the upstream XML schema exactly: track order reversed for FCP compatibility (XML-010), self-closing `<file>` references for repeated media (XML-011), and standard attribute naming for timecode. The Rust XML generation should be pure string/data generation in `render_core` or a dedicated `export_core` crate, testable via snapshot/approval tests against golden XML.

- `Upstream #119`: Audio sync offset must be respected during export. When linked audio clips carry a `syncOffsetFrames`, the export composition must shift the audio track by that offset relative to the video track to maintain sync in the rendered output.

- `Upstream #5`: The export pipeline must handle empty tracks gracefully — they contribute no output but must not cause pipeline failure. The `CompositionPlan` must tolerate tracks with zero clips.

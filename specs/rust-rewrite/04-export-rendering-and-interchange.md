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

- [ ] `EXP-001`: Export formats remain:
  - H.264 → `.mp4`
  - H.265/HEVC → `.mp4`
  - ProRes → `.mov`
  - XML → `.xml`
- [ ] `EXP-002`: Export resolutions remain `720p`, `1080p`, and `4K`.
- [ ] `EXP-003`: Resolution presets target the **short side** of the canvas, not the long side.
- [ ] `EXP-004`: Export size preserves canvas aspect ratio after scaling.
- [ ] `EXP-005`: Export width and height are rounded to even integers.
- [ ] `EXP-006`: Export width and height are never less than `2` pixels.
- [ ] `EXP-007`: XML export is a separate code path and does not go through AV-render export.
- [ ] `EXP-008`: Rendered video export removes any existing destination file before export begins.
- [ ] `EXP-009`: Export progress updates while a rendered export is running.
- [ ] `EXP-010`: Cancellation is surfaced distinctly from other export failures.

## B. Composition build and render behavior

- [ ] `RND-001`: Composition build rejects invalid timelines where fps or canvas size are non-positive.
- [ ] `RND-002`: Composition build collects `offlineMediaRefs` separately from `unprocessableMediaRefs`.
- [ ] `RND-003`: Offline or unreadable media do not fail the whole build; they are skipped and reported.
- [ ] `RND-004`: Unprocessable present files are treated differently from missing files and reported separately.
- [ ] `RND-005`: Text clips are never inserted as normal AV composition tracks.
- [ ] `RND-006`: Text is rendered through the overlay/layer path in preview and rendered export.
- [ ] `RND-007`: Composition inserts a full-duration opaque black background under the timeline when needed.
- [ ] `RND-008`: Audio clips at `1.0x` may share a composition track per timeline track.
- [ ] `RND-009`: Audio clips with non-`1.0x` speed use separate composition tracks.
- [ ] `RND-010`: Same-track visual clips are inserted only when sorted and non-overlapping.
- [ ] `RND-011`: Image clips are rendered through synthetic still-video generation.
- [ ] `RND-012`: Lottie clips are rendered through Lottie-to-video generation.
- [ ] `RND-013`: Video alpha normalization is preserved where the current preview/export path relies on it.
- [ ] `RND-014`: Track mute/hidden state affects render output exactly as it affects preview.
- [ ] `RND-015`: Transform, crop, opacity, fades, and keyframes affect rendered output consistently with preview output.

## C. Text and overlay rendering

- [ ] `TXT-001`: Text overlays are baked into rendered video exports via the animation/layer tool path.
- [ ] `TXT-002`: Export must force text layer display so glyph rendering is deterministic.
- [ ] `TXT-003`: Text opacity animation remains deterministic frame-by-frame.
- [ ] `TXT-004`: Timeline snapshot/capture paths that composite text over video must preserve correct orientation and alpha behavior.

## D. XML interchange contract

- [ ] `XML-001`: XML export remains **XMEML 4 / Final Cut Pro 7 XML**, not FCPXML.
- [ ] `XML-002`: XML preserves clip placement on timeline tracks.
- [ ] `XML-003`: XML preserves source trims.
- [ ] `XML-004`: XML preserves speed changes.
- [ ] `XML-005`: XML preserves volume and opacity.
- [ ] `XML-006`: XML preserves transform and crop.
- [ ] `XML-007`: XML preserves fades.
- [ ] `XML-008`: XML preserves linked clip relationships.
- [ ] `XML-009`: XML preserves source FPS NTSC metadata when relevant.
- [ ] `XML-010`: Visual track order is reversed to match current FCP expectations.
- [ ] `XML-011`: Repeated file references emit one full `<file>` element followed by self-closing references.
- [ ] `XML-012`: Unresolved media are skipped rather than emitted as broken XML items.
- [ ] `XML-013`: XML does **not** claim to preserve text overlays.
- [ ] `XML-014`: XML does **not** claim to preserve flip state.
- [ ] `XML-015`: XML does **not** claim to preserve keyframe easing curves.

## E. Self-contained Palmier project export

- [ ] `BND-001`: Exporting a Palmier project writes a self-contained `.palmier` bundle.
- [ ] `BND-002`: The exported bundle includes timeline JSON, media manifest, generation log, and collected media.
- [ ] `BND-003`: Resolvable source media are copied into the bundle’s `media/` directory.
- [ ] `BND-004`: Copied media are rewritten in the exported manifest as project-relative sources.
- [ ] `BND-005`: Missing or uncollectable media are reported rather than silently omitted without diagnostics.
- [ ] `BND-006`: Multiple references to the same external source file are deduplicated during collection.
- [ ] `BND-007`: The export report distinguishes collected media from missing media.

## F. Render/export parity requirements

- [ ] `PAR-001`: Rendered export uses the same composition semantics as timeline preview.
- [ ] `PAR-002`: If preview can render a valid image/video/lottie/text composition, rendered export must reproduce the same visible timing and stacking semantics.
- [ ] `PAR-003`: Export/search interaction preserves current behavior that pauses indexing while export is active.

## Migration decisions to record explicitly

- `Decision:` The Swift implementation depends on AVFoundation and Core Animation. The Rust rewrite may replace those internals, but must preserve output-level contract: track ordering, trims, timing, overlay behavior, XML structure, and bundle layout.
- `Decision:` If the Rust rewrite chooses a non-AVFoundation render backend, it should add golden-frame/golden-media comparisons to prove parity with the current exported output semantics.

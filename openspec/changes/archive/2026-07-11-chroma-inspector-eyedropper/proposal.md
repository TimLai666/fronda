## Why

Rust had no chroma-key editing UI at all — chroma was settable only via the AI agent tool, so the upstream #291 eyedropper had no home. This builds the missing Chroma Key inspector section and the preview eyedropper on top of the already-shipped soft chroma renderer, so a user can enable a key, pick its colour (presets or by sampling the preview), and tune tolerance/softness/spill.

## What Changes

- New `chroma_controls` pure module: `rgb_to_hue`/`hue_to_rgb`, `ChromaControls` (read a clip's key, build `apply_effect key.chroma` args), and `frame_uv_from_click` (aspect-fit letterbox mapping). Fully unit-tested.
- New `chroma_sampling` cross-view hand-off: the inspector arms the eyedropper with a clip id; the preview consumes it on the next click.
- Inspector **Chroma Key** section (visual clips): On/Off toggle, key-colour swatch + Green/Blue presets + eyedropper button, and Tolerance/Softness/Spill scrub rows. Chroma params are threaded through the existing scrub machinery (`default/derive_scrub_values`, `scrub_commit_args` → `apply_effect`, `fmt_scrub`).
- Preview eyedropper: a bounds-capturing `canvas()` + click handler maps the click to a frame pixel (Fit/zoom=1), samples the composited PNG colour, and applies `key.chroma` with that hue (Swift-parity defaults: tolerance 0.15, softness 0.1).

## Non-Goals

- Full HSV colour wheel (presets + eyedropper cover the common cases).
- Correct sampling at `canvas_zoom ≠ 1.0` (assumes Fit; documented).
- Interaction verification — this repo cannot run gpui, so click-accuracy and feel are verified by a human running the app (per the user's explicit choice). Compile + pure-logic tests are green.

## Capabilities

### New Capabilities

- `chroma-inspector-eyedropper`: a Chroma Key inspector panel and a preview eyedropper that drive the existing soft chroma renderer.

## Impact

- Affected code:
  - New: `crates/app_shell_gpui/src/{chroma_controls.rs,chroma_sampling.rs}`
  - Modified: `crates/app_shell_gpui/src/{inspector_view.rs,preview_view.rs,lib.rs}`
- No on-disk or tool-surface change (drives the existing `apply_effect key.chroma`).

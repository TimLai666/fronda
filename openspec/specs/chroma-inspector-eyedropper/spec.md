# chroma-inspector-eyedropper Specification

## Purpose

TBD - created by archiving change 'chroma-inspector-eyedropper'. Update Purpose after archive.

## Requirements

### Requirement: Chroma control conversions and args

`chroma_controls` SHALL convert RGB↔hue (matching the compositor's key-hue
convention), read a clip's `ChromaKey` into editable controls, and build valid
`apply_effect key.chroma` arguments (keyHue/tolerance/softness/spill) for a set
of clip ids.

#### Scenario: primaries and round-trip

- **WHEN** converting RGB primaries to hue
- **THEN** green → 1/3, blue → 2/3, and `hue_to_rgb` followed by `rgb_to_hue` returns the original hue

#### Scenario: args carry the sampled hue

- **WHEN** building apply args from controls whose colour was set to a hue
- **THEN** the `effects[0].params.keyHue` equals that hue and `type` is `key.chroma`


<!-- @trace
source: chroma-inspector-eyedropper
updated: 2026-07-11
code:
  - crates/app_shell_gpui/src/lib.rs
  - crates/app_shell_gpui/src/chroma_sampling.rs
  - crates/app_shell_gpui/src/inspector_view.rs
  - crates/app_shell_gpui/src/preview_view.rs
-->

---
### Requirement: Eyedropper click maps to a frame pixel

`frame_uv_from_click` SHALL map a canvas-relative click onto normalized frame
coordinates for an aspect-fit frame, returning `None` when the click lands in
the letterbox / pillarbox bars.

#### Scenario: centre and letterbox

- **WHEN** a 16:9 frame is fit in a square canvas
- **THEN** a centre click maps to (0.5, 0.5) and a click in the top letterbox bar returns `None`


<!-- @trace
source: chroma-inspector-eyedropper
updated: 2026-07-11
code:
  - crates/app_shell_gpui/src/lib.rs
  - crates/app_shell_gpui/src/chroma_sampling.rs
  - crates/app_shell_gpui/src/inspector_view.rs
  - crates/app_shell_gpui/src/preview_view.rs
-->

---
### Requirement: Chroma Key inspector section

The inspector SHALL show a Chroma Key section for visual clips with an On/Off
toggle, a key-colour swatch, Green/Blue hue presets, an eyedropper button, and
Tolerance/Softness/Spill controls. Editing SHALL write the `key.chroma` effect
through the shared executor (so undo and MCP observe it), preserving the other
chroma parameters.

#### Scenario: presets and sliders apply through the tool

- **WHEN** the user picks a preset or drags a chroma slider on a visual clip
- **THEN** an `apply_effect key.chroma` runs on the shared executor with the updated parameters, and the other parameters are preserved


<!-- @trace
source: chroma-inspector-eyedropper
updated: 2026-07-11
code:
  - crates/app_shell_gpui/src/lib.rs
  - crates/app_shell_gpui/src/chroma_sampling.rs
  - crates/app_shell_gpui/src/inspector_view.rs
  - crates/app_shell_gpui/src/preview_view.rs
-->

---
### Requirement: Preview eyedropper samples and applies

Arming the eyedropper SHALL make the next preview click sample the composited
frame's colour at that point and apply `key.chroma` with the sampled hue
(enabled) to the armed clip, then disarm. A click outside the frame SHALL
disarm without changing the key.

#### Scenario: sampling a pixel sets the key hue

- **WHEN** the eyedropper is armed and the user clicks a coloured point in the preview
- **THEN** `key.chroma` is applied to the armed clip with the sampled pixel's hue and sampling is disarmed

<!-- @trace
source: chroma-inspector-eyedropper
updated: 2026-07-11
code:
  - crates/app_shell_gpui/src/lib.rs
  - crates/app_shell_gpui/src/chroma_sampling.rs
  - crates/app_shell_gpui/src/inspector_view.rs
  - crates/app_shell_gpui/src/preview_view.rs
-->
# inspector-binding Specification

## Purpose

TBD - created by archiving change 'inspector-completion'. Update Purpose after archive.

## Requirements

### Requirement: Rows bind to the selected clip

Inspector numeric rows (transform, volume, speed, opacity) SHALL display the selected clip's current values (keyframe-resolved at the playhead where applicable) and scrubbing SHALL write back through the standard clip-property tools; each section SHALL offer a reset to defaults.

#### Scenario: Selection drives values

- **WHEN** the user selects a clip whose scale is 0.5
- **THEN** the Scale row shows 0.5 (not a default), and scrubbing it to 0.7 updates the clip


<!-- @trace
source: inspector-completion
updated: 2026-07-10
code:
  - crates/app_shell_gpui/src/inspector_view.rs
  - crates/timeline_core/src/drag_payload.rs
  - crates/timeline_core/src/lib.rs
  - crates/app_shell_gpui/src/app_root.rs
  - crates/app_shell_gpui/src/theme.rs
  - specs/rust-rewrite/98-ui-parity-audit.md
  - crates/generation_core/src/model_catalog.rs
  - crates/app_shell_gpui/src/field_components.rs
  - crates/agent_contract/src/tool_exec.rs
  - crates/app_shell_gpui/src/context_menu.rs
  - crates/app_shell_gpui/src/text_area.rs
  - crates/app_shell_gpui/src/generation_view.rs
  - crates/app_shell_gpui/src/media_panel_view.rs
  - crates/app_shell_gpui/src/preview_view.rs
  - crates/app_shell_gpui/src/lib.rs
  - crates/app_shell_gpui/src/toolbar_view.rs
  - crates/app_shell_gpui/src/tour_overlay_view.rs
  - crates/app_shell_gpui/src/timeline_view.rs
-->

---
### Requirement: Crop and Flip controls

The Crop row SHALL provide an enable toggle and aspect menu bound to the clip's crop, and the Flip row SHALL provide H/V toggles bound to the clip's flip flags.

#### Scenario: Flip toggles

- **WHEN** the user toggles Flip H on a selected clip
- **THEN** the clip's flip_horizontal flag flips and the preview mirrors accordingly


<!-- @trace
source: inspector-completion
updated: 2026-07-10
code:
  - crates/app_shell_gpui/src/inspector_view.rs
  - crates/timeline_core/src/drag_payload.rs
  - crates/timeline_core/src/lib.rs
  - crates/app_shell_gpui/src/app_root.rs
  - crates/app_shell_gpui/src/theme.rs
  - specs/rust-rewrite/98-ui-parity-audit.md
  - crates/generation_core/src/model_catalog.rs
  - crates/app_shell_gpui/src/field_components.rs
  - crates/agent_contract/src/tool_exec.rs
  - crates/app_shell_gpui/src/context_menu.rs
  - crates/app_shell_gpui/src/text_area.rs
  - crates/app_shell_gpui/src/generation_view.rs
  - crates/app_shell_gpui/src/media_panel_view.rs
  - crates/app_shell_gpui/src/preview_view.rs
  - crates/app_shell_gpui/src/lib.rs
  - crates/app_shell_gpui/src/toolbar_view.rs
  - crates/app_shell_gpui/src/tour_overlay_view.rs
  - crates/app_shell_gpui/src/timeline_view.rs
-->

---
### Requirement: Real source metadata

The Source section SHALL show the asset's real file data (dimensions, size, path), an AI badge for generated assets, the Generated parameters from generation_input, and the prompt with a copy button.

#### Scenario: Generated asset

- **WHEN** an AI-generated asset is selected
- **THEN** the AI badge, its generation model/parameters, and its prompt (copyable) are shown

<!-- @trace
source: inspector-completion
updated: 2026-07-10
code:
  - crates/app_shell_gpui/src/inspector_view.rs
  - crates/timeline_core/src/drag_payload.rs
  - crates/timeline_core/src/lib.rs
  - crates/app_shell_gpui/src/app_root.rs
  - crates/app_shell_gpui/src/theme.rs
  - specs/rust-rewrite/98-ui-parity-audit.md
  - crates/generation_core/src/model_catalog.rs
  - crates/app_shell_gpui/src/field_components.rs
  - crates/agent_contract/src/tool_exec.rs
  - crates/app_shell_gpui/src/context_menu.rs
  - crates/app_shell_gpui/src/text_area.rs
  - crates/app_shell_gpui/src/generation_view.rs
  - crates/app_shell_gpui/src/media_panel_view.rs
  - crates/app_shell_gpui/src/preview_view.rs
  - crates/app_shell_gpui/src/lib.rs
  - crates/app_shell_gpui/src/toolbar_view.rs
  - crates/app_shell_gpui/src/tour_overlay_view.rs
  - crates/app_shell_gpui/src/timeline_view.rs
-->
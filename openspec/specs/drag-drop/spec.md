# drag-drop Specification

## Purpose

TBD - created by archiving change 'drag-drop-system'. Update Purpose after archive.

## Requirements

### Requirement: External file import by drop

The media panel SHALL accept files dragged from the OS file manager, highlight while a compatible drag hovers, and import the dropped files through the existing import flow.

#### Scenario: Drop a video file

- **WHEN** the user drops an .mp4 from the file manager onto the media grid
- **THEN** the asset appears in the library exactly as if imported via the menu


<!-- @trace
source: drag-drop-system
updated: 2026-07-10
code:
  - crates/app_shell_gpui/src/toolbar_view.rs
  - crates/app_shell_gpui/src/inspector_view.rs
  - crates/app_shell_gpui/src/app_root.rs
  - crates/agent_contract/src/tool_exec.rs
  - crates/app_shell_gpui/src/lib.rs
  - crates/app_shell_gpui/src/text_area.rs
  - crates/app_shell_gpui/src/timeline_view.rs
  - crates/app_shell_gpui/src/field_components.rs
  - crates/timeline_core/src/drag_payload.rs
  - crates/app_shell_gpui/src/generation_view.rs
  - crates/generation_core/src/model_catalog.rs
  - crates/app_shell_gpui/src/theme.rs
  - crates/app_shell_gpui/src/context_menu.rs
  - crates/timeline_core/src/lib.rs
  - crates/app_shell_gpui/src/media_panel_view.rs
  - specs/rust-rewrite/98-ui-parity-audit.md
  - crates/app_shell_gpui/src/tour_overlay_view.rs
  - crates/app_shell_gpui/src/preview_view.rs
-->

---
### Requirement: Asset to timeline

A media tile SHALL be draggable onto a timeline track; dropping places the asset at the pointer's frame via the standard placement path (linked A/V, fps warnings) with an insertion indicator during hover.

#### Scenario: Drop places a clip

- **WHEN** the user drags an asset over a video track and releases at frame ~120
- **THEN** a clip for that asset is placed at the drop frame on that track with normal linked-audio behavior


<!-- @trace
source: drag-drop-system
updated: 2026-07-10
code:
  - crates/app_shell_gpui/src/toolbar_view.rs
  - crates/app_shell_gpui/src/inspector_view.rs
  - crates/app_shell_gpui/src/app_root.rs
  - crates/agent_contract/src/tool_exec.rs
  - crates/app_shell_gpui/src/lib.rs
  - crates/app_shell_gpui/src/text_area.rs
  - crates/app_shell_gpui/src/timeline_view.rs
  - crates/app_shell_gpui/src/field_components.rs
  - crates/timeline_core/src/drag_payload.rs
  - crates/app_shell_gpui/src/generation_view.rs
  - crates/generation_core/src/model_catalog.rs
  - crates/app_shell_gpui/src/theme.rs
  - crates/app_shell_gpui/src/context_menu.rs
  - crates/timeline_core/src/lib.rs
  - crates/app_shell_gpui/src/media_panel_view.rs
  - specs/rust-rewrite/98-ui-parity-audit.md
  - crates/app_shell_gpui/src/tour_overlay_view.rs
  - crates/app_shell_gpui/src/preview_view.rs
-->

---
### Requirement: Asset to generation reference

Generation reference tiles SHALL accept a dragged media asset, subject to the same type and cap rules as click-to-pick assignment.

#### Scenario: Drop into a reference slot

- **WHEN** the user drags an image asset onto an empty reference tile
- **THEN** the tile shows that asset's thumbnail and the generation state records the reference

<!-- @trace
source: drag-drop-system
updated: 2026-07-10
code:
  - crates/app_shell_gpui/src/toolbar_view.rs
  - crates/app_shell_gpui/src/inspector_view.rs
  - crates/app_shell_gpui/src/app_root.rs
  - crates/agent_contract/src/tool_exec.rs
  - crates/app_shell_gpui/src/lib.rs
  - crates/app_shell_gpui/src/text_area.rs
  - crates/app_shell_gpui/src/timeline_view.rs
  - crates/app_shell_gpui/src/field_components.rs
  - crates/timeline_core/src/drag_payload.rs
  - crates/app_shell_gpui/src/generation_view.rs
  - crates/generation_core/src/model_catalog.rs
  - crates/app_shell_gpui/src/theme.rs
  - crates/app_shell_gpui/src/context_menu.rs
  - crates/timeline_core/src/lib.rs
  - crates/app_shell_gpui/src/media_panel_view.rs
  - specs/rust-rewrite/98-ui-parity-audit.md
  - crates/app_shell_gpui/src/tour_overlay_view.rs
  - crates/app_shell_gpui/src/preview_view.rs
-->
# home-preview-polish Specification

## Purpose

TBD - created by archiving change 'home-preview-polish'. Update Purpose after archive.

## Requirements

### Requirement: Project card interactions

Recent-project cards SHALL show hover feedback, a hover-revealed delete button with a confirmation step, and a file-missing overlay when the project path no longer exists.

#### Scenario: Missing project file

- **WHEN** a recent project's directory has been deleted on disk
- **THEN** its card dims with a file-missing indicator and opening it is prevented with an explanation


<!-- @trace
source: home-preview-polish
updated: 2026-07-10
code:
  - specs/rust-rewrite/98-ui-parity-audit.md
  - crates/app_shell_gpui/src/preview_view.rs
  - crates/app_shell_gpui/src/lib.rs
  - crates/generation_core/src/model_catalog.rs
  - crates/app_shell_gpui/src/generation_view.rs
  - crates/app_shell_gpui/src/text_area.rs
  - crates/app_shell_gpui/src/tour_overlay_view.rs
  - crates/app_shell_gpui/src/app_root.rs
  - crates/app_shell_gpui/src/context_menu.rs
  - crates/app_shell_gpui/src/toolbar_view.rs
  - crates/timeline_core/src/lib.rs
  - crates/app_shell_gpui/src/field_components.rs
  - crates/agent_contract/src/tool_exec.rs
  - crates/app_shell_gpui/src/timeline_view.rs
  - crates/app_shell_gpui/src/media_panel_view.rs
  - crates/app_shell_gpui/src/theme.rs
  - crates/timeline_core/src/drag_payload.rs
  - crates/app_shell_gpui/src/inspector_view.rs
-->

---
### Requirement: Open Project file panel

The sidebar Open Project action SHALL present the platform folder picker and open the chosen .palmier project.

#### Scenario: Pick a project

- **WHEN** the user picks a valid project directory in the panel
- **THEN** the editor opens that project exactly like a recents click


<!-- @trace
source: home-preview-polish
updated: 2026-07-10
code:
  - specs/rust-rewrite/98-ui-parity-audit.md
  - crates/app_shell_gpui/src/preview_view.rs
  - crates/app_shell_gpui/src/lib.rs
  - crates/generation_core/src/model_catalog.rs
  - crates/app_shell_gpui/src/generation_view.rs
  - crates/app_shell_gpui/src/text_area.rs
  - crates/app_shell_gpui/src/tour_overlay_view.rs
  - crates/app_shell_gpui/src/app_root.rs
  - crates/app_shell_gpui/src/context_menu.rs
  - crates/app_shell_gpui/src/toolbar_view.rs
  - crates/timeline_core/src/lib.rs
  - crates/app_shell_gpui/src/field_components.rs
  - crates/agent_contract/src/tool_exec.rs
  - crates/app_shell_gpui/src/timeline_view.rs
  - crates/app_shell_gpui/src/media_panel_view.rs
  - crates/app_shell_gpui/src/theme.rs
  - crates/timeline_core/src/drag_payload.rs
  - crates/app_shell_gpui/src/inspector_view.rs
-->

---
### Requirement: Preview settings menus

The preview header SHALL offer Aspect-Ratio, Frame-Rate, Resolution/Quality, and Zoom menus fed by the project presets, applying selections through the standard settings path.

#### Scenario: Change fps

- **WHEN** the user picks a different frame rate
- **THEN** the timeline settings update via set_project_settings semantics (rescale prompts included)


<!-- @trace
source: home-preview-polish
updated: 2026-07-10
code:
  - specs/rust-rewrite/98-ui-parity-audit.md
  - crates/app_shell_gpui/src/preview_view.rs
  - crates/app_shell_gpui/src/lib.rs
  - crates/generation_core/src/model_catalog.rs
  - crates/app_shell_gpui/src/generation_view.rs
  - crates/app_shell_gpui/src/text_area.rs
  - crates/app_shell_gpui/src/tour_overlay_view.rs
  - crates/app_shell_gpui/src/app_root.rs
  - crates/app_shell_gpui/src/context_menu.rs
  - crates/app_shell_gpui/src/toolbar_view.rs
  - crates/timeline_core/src/lib.rs
  - crates/app_shell_gpui/src/field_components.rs
  - crates/agent_contract/src/tool_exec.rs
  - crates/app_shell_gpui/src/timeline_view.rs
  - crates/app_shell_gpui/src/media_panel_view.rs
  - crates/app_shell_gpui/src/theme.rs
  - crates/timeline_core/src/drag_payload.rs
  - crates/app_shell_gpui/src/inspector_view.rs
-->

---
### Requirement: Capture frame

A Capture Frame button SHALL composite the current paused frame and add it to the media library as an image asset.

#### Scenario: Capture

- **WHEN** the user hits Capture Frame while paused
- **THEN** a PNG of the composited frame lands in the project media and appears in the library


<!-- @trace
source: home-preview-polish
updated: 2026-07-10
code:
  - specs/rust-rewrite/98-ui-parity-audit.md
  - crates/app_shell_gpui/src/preview_view.rs
  - crates/app_shell_gpui/src/lib.rs
  - crates/generation_core/src/model_catalog.rs
  - crates/app_shell_gpui/src/generation_view.rs
  - crates/app_shell_gpui/src/text_area.rs
  - crates/app_shell_gpui/src/tour_overlay_view.rs
  - crates/app_shell_gpui/src/app_root.rs
  - crates/app_shell_gpui/src/context_menu.rs
  - crates/app_shell_gpui/src/toolbar_view.rs
  - crates/timeline_core/src/lib.rs
  - crates/app_shell_gpui/src/field_components.rs
  - crates/agent_contract/src/tool_exec.rs
  - crates/app_shell_gpui/src/timeline_view.rs
  - crates/app_shell_gpui/src/media_panel_view.rs
  - crates/app_shell_gpui/src/theme.rs
  - crates/timeline_core/src/drag_payload.rs
  - crates/app_shell_gpui/src/inspector_view.rs
-->

---
### Requirement: Tour spotlight and Add-Text

The tour overlay SHALL visually spotlight the current step's anchor region (dimming everything else), and the toolbar SHALL include the Add-Text button inserting a default text clip at the playhead.

#### Scenario: Add text

- **WHEN** the user clicks the toolbar "T" button
- **THEN** a text clip appears at the playhead on the appropriate track

<!-- @trace
source: home-preview-polish
updated: 2026-07-10
code:
  - specs/rust-rewrite/98-ui-parity-audit.md
  - crates/app_shell_gpui/src/preview_view.rs
  - crates/app_shell_gpui/src/lib.rs
  - crates/generation_core/src/model_catalog.rs
  - crates/app_shell_gpui/src/generation_view.rs
  - crates/app_shell_gpui/src/text_area.rs
  - crates/app_shell_gpui/src/tour_overlay_view.rs
  - crates/app_shell_gpui/src/app_root.rs
  - crates/app_shell_gpui/src/context_menu.rs
  - crates/app_shell_gpui/src/toolbar_view.rs
  - crates/timeline_core/src/lib.rs
  - crates/app_shell_gpui/src/field_components.rs
  - crates/agent_contract/src/tool_exec.rs
  - crates/app_shell_gpui/src/timeline_view.rs
  - crates/app_shell_gpui/src/media_panel_view.rs
  - crates/app_shell_gpui/src/theme.rs
  - crates/timeline_core/src/drag_payload.rs
  - crates/app_shell_gpui/src/inspector_view.rs
-->
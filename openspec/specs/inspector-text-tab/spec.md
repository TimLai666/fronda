# inspector-text-tab Specification

## Purpose

TBD - created by archiving change 'inspector-completion'. Update Purpose after archive.

## Requirements

### Requirement: Text tab edits the selected text clip

With a text clip selected, the Text tab SHALL show and edit its TextStyle: content (multiline), font family, size, opacity, color, alignment, background (color + toggle), shadow, stroke, and position, writing changes back through the standard text-update path so undo and persistence behave like any other edit.

#### Scenario: Editing content updates the clip

- **WHEN** the user edits the content field and commits
- **THEN** the selected clip's text updates on the timeline/preview and undo reverts it

#### Scenario: Style round-trip

- **WHEN** the user picks a font, size, and color
- **THEN** the clip's TextStyle carries those values after save/reload

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
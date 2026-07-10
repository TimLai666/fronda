# context-menus Specification

## Purpose

TBD - created by archiving change 'context-menu-system'. Update Purpose after archive.

## Requirements

### Requirement: Context menu component

A reusable context menu SHALL open at the pointer on right-click, close on outside click or Escape, and render items with hover highlight, separators, and a destructive style for dangerous actions.

#### Scenario: Open and dismiss

- **WHEN** the user right-clicks a surface with a menu and then clicks elsewhere
- **THEN** the menu appears at the pointer and disappears without triggering any item


<!-- @trace
source: context-menu-system
updated: 2026-07-10
code:
  - crates/generation_core/src/model_catalog.rs
  - crates/app_shell_gpui/src/inspector_view.rs
  - crates/app_shell_gpui/src/field_components.rs
  - crates/timeline_core/src/drag_payload.rs
  - crates/app_shell_gpui/src/text_area.rs
  - crates/app_shell_gpui/src/app_root.rs
  - crates/app_shell_gpui/src/chat_view.rs
  - crates/timeline_core/src/lib.rs
  - crates/app_shell_gpui/src/lib.rs
  - crates/app_shell_gpui/src/media_panel_view.rs
  - crates/app_shell_gpui/src/context_menu.rs
  - crates/app_shell_gpui/src/preview_view.rs
  - specs/rust-rewrite/98-ui-parity-audit.md
  - crates/agent_contract/src/tool_exec.rs
  - crates/app_shell_gpui/src/generation_view.rs
  - crates/app_shell_gpui/src/timeline_view.rs
  - crates/app_shell_gpui/src/toolbar_view.rs
  - crates/app_shell_gpui/src/theme.rs
  - crates/app_shell_gpui/src/tour_overlay_view.rs
-->

---
### Requirement: Project card menu

Right-clicking a recent-project card SHALL offer Open, Reveal in File Manager, Remove from Recents, and Delete Project (with a confirmation step before deletion).

#### Scenario: Delete requires confirmation

- **WHEN** the user picks Delete Project
- **THEN** a confirmation appears and the project is deleted only after confirming


<!-- @trace
source: context-menu-system
updated: 2026-07-10
code:
  - crates/generation_core/src/model_catalog.rs
  - crates/app_shell_gpui/src/inspector_view.rs
  - crates/app_shell_gpui/src/field_components.rs
  - crates/timeline_core/src/drag_payload.rs
  - crates/app_shell_gpui/src/text_area.rs
  - crates/app_shell_gpui/src/app_root.rs
  - crates/app_shell_gpui/src/chat_view.rs
  - crates/timeline_core/src/lib.rs
  - crates/app_shell_gpui/src/lib.rs
  - crates/app_shell_gpui/src/media_panel_view.rs
  - crates/app_shell_gpui/src/context_menu.rs
  - crates/app_shell_gpui/src/preview_view.rs
  - specs/rust-rewrite/98-ui-parity-audit.md
  - crates/agent_contract/src/tool_exec.rs
  - crates/app_shell_gpui/src/generation_view.rs
  - crates/app_shell_gpui/src/timeline_view.rs
  - crates/app_shell_gpui/src/toolbar_view.rs
  - crates/app_shell_gpui/src/theme.rs
  - crates/app_shell_gpui/src/tour_overlay_view.rs
-->

---
### Requirement: Asset and folder menus

Right-clicking a media asset SHALL offer Rename, Delete, and Reveal; right-clicking a folder SHALL offer Rename and Delete (assets move to the parent).

#### Scenario: Rename from the menu

- **WHEN** the user picks Rename on an asset
- **THEN** an inline editable field appears seeded with the current name and Enter commits via the standard rename path

<!-- @trace
source: context-menu-system
updated: 2026-07-10
code:
  - crates/generation_core/src/model_catalog.rs
  - crates/app_shell_gpui/src/inspector_view.rs
  - crates/app_shell_gpui/src/field_components.rs
  - crates/timeline_core/src/drag_payload.rs
  - crates/app_shell_gpui/src/text_area.rs
  - crates/app_shell_gpui/src/app_root.rs
  - crates/app_shell_gpui/src/chat_view.rs
  - crates/timeline_core/src/lib.rs
  - crates/app_shell_gpui/src/lib.rs
  - crates/app_shell_gpui/src/media_panel_view.rs
  - crates/app_shell_gpui/src/context_menu.rs
  - crates/app_shell_gpui/src/preview_view.rs
  - specs/rust-rewrite/98-ui-parity-audit.md
  - crates/agent_contract/src/tool_exec.rs
  - crates/app_shell_gpui/src/generation_view.rs
  - crates/app_shell_gpui/src/timeline_view.rs
  - crates/app_shell_gpui/src/toolbar_view.rs
  - crates/app_shell_gpui/src/theme.rs
  - crates/app_shell_gpui/src/tour_overlay_view.rs
-->
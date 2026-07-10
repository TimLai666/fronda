# music-tab Specification

## Purpose

TBD - created by archiving change 'captions-music-tabs'. Update Purpose after archive.

## Requirements

### Requirement: Mode, model, and prompt

The Music tab SHALL offer an input-mode menu (Video to Music / Text to Music), a model menu listing the catalog's music-capable audio entries, a duration scrub in text mode, and a real prompt field.

#### Scenario: Text mode exposes duration

- **WHEN** the user switches to Text to Music
- **THEN** the duration scrub appears and the source-span summary hides


<!-- @trace
source: captions-music-tabs
updated: 2026-07-10
code:
  - crates/app_shell_gpui/src/field_components.rs
  - crates/app_shell_gpui/src/generation_view.rs
  - crates/timeline_core/src/drag_payload.rs
  - crates/app_shell_gpui/src/tour_overlay_view.rs
  - crates/app_shell_gpui/src/text_area.rs
  - crates/agent_contract/src/tool_exec.rs
  - crates/app_shell_gpui/src/toolbar_view.rs
  - crates/app_shell_gpui/src/inspector_view.rs
  - crates/app_shell_gpui/src/app_root.rs
  - crates/app_shell_gpui/src/context_menu.rs
  - crates/timeline_core/src/lib.rs
  - crates/app_shell_gpui/src/theme.rs
  - crates/app_shell_gpui/src/media_panel_view.rs
  - specs/rust-rewrite/98-ui-parity-audit.md
  - crates/app_shell_gpui/src/preview_view.rs
  - crates/generation_core/src/model_catalog.rs
  - crates/app_shell_gpui/src/timeline_view.rs
  - crates/app_shell_gpui/src/lib.rs
-->

---
### Requirement: Cost and credit gating

The tab SHALL show the cost estimate for the selection and disable generation with an explanatory note when credits are insufficient or no backend is available.

#### Scenario: No backend

- **WHEN** Generate is pressed with no generation backend installed
- **THEN** the tab shows an unavailable note and no fake overlay runs

<!-- @trace
source: captions-music-tabs
updated: 2026-07-10
code:
  - crates/app_shell_gpui/src/field_components.rs
  - crates/app_shell_gpui/src/generation_view.rs
  - crates/timeline_core/src/drag_payload.rs
  - crates/app_shell_gpui/src/tour_overlay_view.rs
  - crates/app_shell_gpui/src/text_area.rs
  - crates/agent_contract/src/tool_exec.rs
  - crates/app_shell_gpui/src/toolbar_view.rs
  - crates/app_shell_gpui/src/inspector_view.rs
  - crates/app_shell_gpui/src/app_root.rs
  - crates/app_shell_gpui/src/context_menu.rs
  - crates/timeline_core/src/lib.rs
  - crates/app_shell_gpui/src/theme.rs
  - crates/app_shell_gpui/src/media_panel_view.rs
  - specs/rust-rewrite/98-ui-parity-audit.md
  - crates/app_shell_gpui/src/preview_view.rs
  - crates/generation_core/src/model_catalog.rs
  - crates/app_shell_gpui/src/timeline_view.rs
  - crates/app_shell_gpui/src/lib.rs
-->
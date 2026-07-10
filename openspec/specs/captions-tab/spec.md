# captions-tab Specification

## Purpose

TBD - created by archiving change 'captions-music-tabs'. Update Purpose after archive.

## Requirements

### Requirement: Caption styling controls

The Captions tab SHALL provide working controls for source, language, font family (from the bundled font list), size, color, background (with toggle), case, and profanity censoring, persisting into the caption configuration used at generation time.

#### Scenario: Style change reflects in the preview

- **WHEN** the user changes the font size and background color
- **THEN** the live preview box re-renders the sample caption with those values


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
### Requirement: Live preview with placement

The tab SHALL render a caption preview box with center guides and scrubbable X/Y placement fields that update the configured caption position.

#### Scenario: Scrubbing placement moves the sample

- **WHEN** the user scrubs the Y field downward
- **THEN** the sample caption in the preview moves accordingly and the position persists


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
### Requirement: Generation gating

Generate SHALL require transcribed words: with none available it shows why (no transcription provider / no speech) instead of a fake progress state, and during a real run it shows the transcribing overlay.

#### Scenario: No words available

- **WHEN** the user hits Generate with no transcription available
- **THEN** an explanatory note appears and no overlay spins

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
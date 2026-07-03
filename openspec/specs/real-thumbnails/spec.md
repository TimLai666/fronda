# real-thumbnails Specification

## Purpose

TBD - created by archiving change 'media-import-recents-thumbnails'. Update Purpose after archive.

## Requirements

### Requirement: Image media renders real thumbnails

Media panel tiles for Image entries SHALL render the source image file (External absolute path, or Project relative path resolved against the current project root). When the file is missing or the path cannot be resolved, the tile SHALL fall back to the type-colored placeholder. Video and audio tiles keep the type-colored placeholder: first-frame video thumbnails require a decoding subsystem (AVFoundation in the Swift baseline) that the Rust workspace intentionally does not include; introducing one is an explicit architecture decision outside this capability.

#### Scenario: Image tile shows the file

- **WHEN** an Image entry references an existing PNG via an absolute path
- **THEN** its tile renders that image

#### Scenario: Missing file falls back

- **WHEN** an Image entry's source file does not exist on disk
- **THEN** its tile renders the type-colored placeholder


<!-- @trace
source: media-import-recents-thumbnails
updated: 2026-07-03
code:
  - crates/app_shell_gpui/src/project_registry_store.rs
  - crates/app_shell_gpui/src/app_root.rs
  - crates/app_shell_gpui/src/mcp_service.rs
  - crates/app_shell_gpui/Cargo.toml
  - crates/app_shell_gpui/src/media_import.rs
  - crates/app_shell_gpui/src/media_panel_model.rs
  - crates/app_shell_gpui/src/lib.rs
  - crates/app_shell_gpui/src/editor_state_hub.rs
  - crates/app_shell_gpui/src/home_model.rs
  - crates/app_shell_gpui/src/media_panel_view.rs
-->

---
### Requirement: Project cards show the bundle thumbnail

Recent-project cards SHALL render the bundle's thumbnail.png when the file exists, and the placeholder block otherwise.

#### Scenario: Thumbnail present

- **WHEN** a registry entry's bundle contains thumbnail.png
- **THEN** the card renders that image

<!-- @trace
source: media-import-recents-thumbnails
updated: 2026-07-03
code:
  - crates/app_shell_gpui/src/project_registry_store.rs
  - crates/app_shell_gpui/src/app_root.rs
  - crates/app_shell_gpui/src/mcp_service.rs
  - crates/app_shell_gpui/Cargo.toml
  - crates/app_shell_gpui/src/media_import.rs
  - crates/app_shell_gpui/src/media_panel_model.rs
  - crates/app_shell_gpui/src/lib.rs
  - crates/app_shell_gpui/src/editor_state_hub.rs
  - crates/app_shell_gpui/src/home_model.rs
  - crates/app_shell_gpui/src/media_panel_view.rs
-->
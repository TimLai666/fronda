# media-import-ui Specification

## Purpose

TBD - created by archiving change 'media-import-recents-thumbnails'. Update Purpose after archive.

## Requirements

### Requirement: Import Media dialog feeds the shared manifest

The ImportMedia menu action SHALL present a multi-selection file picker and import each chosen file into the shared manifest via the import_media tool, using the file name as the entry name, the absolute path as filePath, and ClipType::from_extension for the type. Files whose extension is not recognized SHALL be skipped (logged) without aborting the remaining imports. Imported entries SHALL appear in the media panel through the revision mechanism and be visible over MCP.

#### Scenario: Import an image and a video

- **WHEN** the user imports photo.png and take1.mp4
- **THEN** the shared manifest gains an Image entry named photo.png and a Video entry named take1.mp4

#### Scenario: Unknown extension is skipped

- **WHEN** the user imports notes.txt together with take1.mp4
- **THEN** take1.mp4 is imported and notes.txt is skipped without an error dialog

#### Scenario: Cancel imports nothing

- **WHEN** the user cancels the picker
- **THEN** the manifest and revision are unchanged

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
# recent-projects Specification

## Purpose

TBD - created by archiving change 'media-import-recents-thumbnails'. Update Purpose after archive.

## Requirements

### Requirement: Opened projects persist in the recent-project registry

Successful load_bundle and save_as SHALL record the project path in a persisted ProjectRegistry (Fronda config directory, projects.json), updating last_opened_date on repeat opens without duplicating entries. A missing or corrupt registry file SHALL be treated as an empty registry. Registry persistence failures MUST NOT block opening or saving.

#### Scenario: Repeat open updates instead of duplicating

- **WHEN** the same project is opened twice
- **THEN** the registry holds one entry for it with the newer last_opened_date

#### Scenario: Corrupt registry recovers empty

- **WHEN** projects.json contains invalid JSON and the registry is loaded
- **THEN** an empty registry is returned and the next record overwrites the corrupt file


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
### Requirement: Home screen lists recent projects

The Home screen SHALL render one card per registry entry, sorted by last_opened_date descending, showing the project name and a relative time label (just now / N m ago / N h ago / N d ago). Clicking a card SHALL open the project via open_project_at. Hard-coded demo project data MUST NOT be the runtime data source.

#### Scenario: Card opens the project

- **WHEN** the user clicks a recent-project card whose bundle still exists
- **THEN** the project loads and the editor screen is shown

##### Example: Relative time labels

| now - last_opened | label |
| ------------------ | ----- |
| 30 seconds | just now |
| 5 minutes | 5m ago |
| 3 hours | 3h ago |
| 2 days | 2d ago |

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
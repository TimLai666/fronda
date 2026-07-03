# media-panel-binding Specification

## Purpose

TBD - created by archiving change 'bind-media-panel-to-shared-state'. Update Purpose after archive.

## Requirements

### Requirement: Media panel state maps from the shared manifest

`MediaPanelState::sync_from_manifest(manifest)` SHALL rebuild the panel's media items (id, name, clip type) from `manifest.entries` and its folder list from `manifest.folders`, replacing previous lists rather than appending, and preserving view-only state such as the active tab.

#### Scenario: Entries and folders map to panel lists

- **WHEN** sync_from_manifest is called with a manifest holding two entries and one folder
- **THEN** the state has exactly two media items (matching entry ids and names) and one folder

#### Scenario: Repeated sync is idempotent

- **WHEN** sync_from_manifest is called twice with the same manifest
- **THEN** the item and folder counts are unchanged after the second call


<!-- @trace
source: bind-media-panel-to-shared-state
updated: 2026-07-03
code:
  - crates/app_shell_gpui/src/media_panel_model.rs
  - crates/app_shell_gpui/src/media_panel_view.rs
-->

---
### Requirement: Media panel renders the shared manifest

The media panel Library grid SHALL render one tile per manifest entry (name text, icon by clip type: video ▶, audio ♪, image ⬜, text T) and update when the hub revision changes. An empty manifest SHALL render an empty grid. Hard-coded demo tiles MUST NOT be the runtime data source.

#### Scenario: Loaded project populates the grid

- **WHEN** a project whose manifest has entries is loaded and the panel renders
- **THEN** the grid shows one tile per entry with the entry's name

#### Scenario: Rename via MCP updates the tile

- **WHEN** an MCP rename_media call changes an entry's name and the panel renders afterward
- **THEN** the tile shows the new name


<!-- @trace
source: bind-media-panel-to-shared-state
updated: 2026-07-03
code:
  - crates/app_shell_gpui/src/media_panel_model.rs
  - crates/app_shell_gpui/src/media_panel_view.rs
-->

---
### Requirement: Tile hue is stable per media id

The view SHALL derive each tile's placeholder hue deterministically from the media id, producing a value in [0.0, 1.0) that is identical across renders for the same id.

#### Scenario: Same id same hue

- **WHEN** the hue is computed twice for media id "m1"
- **THEN** both computations return the same value within [0.0, 1.0)

<!-- @trace
source: bind-media-panel-to-shared-state
updated: 2026-07-03
code:
  - crates/app_shell_gpui/src/media_panel_model.rs
  - crates/app_shell_gpui/src/media_panel_view.rs
-->
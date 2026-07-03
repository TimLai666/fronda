# project-file-dialogs Specification

## Purpose

TBD - created by archiving change 'wire-project-file-dialogs'. Update Purpose after archive.

## Requirements

### Requirement: Open Project shows a directory picker and loads the choice

The OpenProject menu action (Cmd/Ctrl+O) SHALL present the platform directory picker (directories only, single selection) and, when a path is chosen, load it via the existing open_project_at flow (switching to the editor only on successful load). Cancelling the dialog MUST leave the app state unchanged.

#### Scenario: Choose a valid project

- **WHEN** the user triggers OpenProject and picks a directory containing project.json
- **THEN** the project loads into the shared state and the editor screen is shown

#### Scenario: Cancel the dialog

- **WHEN** the user triggers OpenProject and cancels the picker
- **THEN** no state changes and the current screen remains


<!-- @trace
source: wire-project-file-dialogs
updated: 2026-07-03
code:
  - crates/app_shell_gpui/src/editor_state_hub.rs
  - crates/app_shell_gpui/src/app_root.rs
-->

---
### Requirement: Save As writes to a new root and switches the project root

`EditorStateHub::save_as(root)` SHALL write the current shared timeline and manifest to the given directory via the narrow save path and record that directory as the new project root, so subsequent saves target it. On write failure the project root MUST remain unchanged. The SaveProjectAs menu action (Cmd/Ctrl+Shift+S) SHALL present the platform save dialog with a suggested `.palmier` name and call save_as with the chosen path.

#### Scenario: Save As then Save targets the new root

- **WHEN** save_as succeeds for a new directory and a later save() is called
- **THEN** project.json and media.json exist under the new directory and save() writes there, not the old root

#### Scenario: Save As from an unsaved project

- **WHEN** no project root is recorded and save_as is called with a valid directory
- **THEN** the state is written there and that directory becomes the project root

<!-- @trace
source: wire-project-file-dialogs
updated: 2026-07-03
code:
  - crates/app_shell_gpui/src/editor_state_hub.rs
  - crates/app_shell_gpui/src/app_root.rs
-->
# project-save Specification

## Purpose

TBD - created by archiving change 'save-project-from-shared-state'. Update Purpose after archive.

## Requirements

### Requirement: Shared state saves back to the open project

`EditorStateHub::save()` SHALL write the shared executor's timeline and media manifest to the recorded project root as `project.json` and `media.json` via `project_io::save_project_state`, touching no other files in the package. When no project is open (no recorded root) it SHALL return an error and write nothing.

#### Scenario: Save round-trips MCP edits

- **WHEN** a project is loaded, an MCP tool call creates folder "B-roll", and save() is called
- **THEN** reopening the package yields a manifest containing "B-roll" and the same timeline

#### Scenario: Save without an open project fails

- **WHEN** save() is called while no project root is recorded
- **THEN** it returns an error and no files are written


<!-- @trace
source: save-project-from-shared-state
updated: 2026-07-03
code:
  - crates/app_shell_gpui/src/app_root.rs
  - crates/project_io/src/lib.rs
  - crates/app_shell_gpui/src/editor_state_hub.rs
tests:
  - crates/project_io/tests/project_bundle.rs
-->

---
### Requirement: Narrow save preserves unrelated package content

`project_io::save_project_state(root, timeline, manifest)` SHALL write exactly `project.json` and `media.json` under the root (creating the directory if needed) and MUST NOT modify or delete any other file in the package, including chat sessions, transcripts, generation log, thumbnails, and the media directory.

#### Scenario: Chat files survive a save

- **WHEN** a package containing chat/session1.json is saved via save_project_state
- **THEN** chat/session1.json still exists with identical content, and project.json/media.json reflect the given state

##### Example: Files written

| File | Written by save_project_state |
| ---- | ----------------------------- |
| project.json | yes |
| media.json | yes |
| generation-log.json | no |
| chat/*.json | no |
| media/* | no |

<!-- @trace
source: save-project-from-shared-state
updated: 2026-07-03
code:
  - crates/app_shell_gpui/src/app_root.rs
  - crates/project_io/src/lib.rs
  - crates/app_shell_gpui/src/editor_state_hub.rs
tests:
  - crates/project_io/tests/project_bundle.rs
-->
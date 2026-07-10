# timeline-import-action Specification

## Purpose

TBD - created by archiving change 'xml-import-app-wiring'. Update Purpose after archive.

## Requirements

### Requirement: Import Timeline is a reachable app action

The app SHALL expose an Import Timeline action (File menu, ⌘⇧I) that opens a
file picker, reads the chosen XMEML/FCPXML file, detects the format from the
document content (falling back to the file extension), and imports it into the
current project. Detection SHALL return no format for content that is neither
XMEML nor FCPXML, and the action SHALL report the error rather than crash.

#### Scenario: shortcut and menu registration

- **WHEN** the File menu items and shortcut table are built
- **THEN** they SHALL include Import Timeline bound to ⌘⇧I, distinct from Import Media (⌘I)

#### Scenario: format detected from content over extension

- **WHEN** a file whose content begins with `<fcpxml …>` is imported through a `.xml`-named path
- **THEN** the FCPXML parser SHALL be selected


<!-- @trace
source: xml-import-app-wiring
updated: 2026-07-11
code:
  - crates/app_shell_gpui/src/app_root.rs
  - crates/app_shell_gpui/src/timeline_import.rs
  - crates/app_shell_gpui/src/menu.rs
  - crates/app_shell_gpui/src/lib.rs
  - crates/agent_contract/src/tool_exec.rs
  - crates/app_shell_gpui/src/help_view.rs
-->

---
### Requirement: Import adopts a new active timeline without dropping open work

Importing SHALL adopt the parsed timeline as the new active timeline and keep
the previously active timeline as a sibling — import MUST NOT overwrite or
discard the open timeline. The executor's revision SHALL advance so every view
resyncs, and the undo stack SHALL be cleared.

#### Scenario: previous timeline preserved as sibling

- **WHEN** a project with an active timeline imports an XML file
- **THEN** the imported timeline SHALL become active, the prior one SHALL appear in the sibling list, and the revision SHALL be greater than before


<!-- @trace
source: xml-import-app-wiring
updated: 2026-07-11
code:
  - crates/app_shell_gpui/src/app_root.rs
  - crates/app_shell_gpui/src/timeline_import.rs
  - crates/app_shell_gpui/src/menu.rs
  - crates/app_shell_gpui/src/lib.rs
  - crates/agent_contract/src/tool_exec.rs
  - crates/app_shell_gpui/src/help_view.rs
-->

---
### Requirement: Imported media relinks to the library or registers offline

For each referenced file, the import SHALL relink to an existing library entry
matched by filename; when none matches it SHALL register the referenced path
(recording the path even when the file is absent, so the clip shows offline
rather than being dropped). Every clip's `media_ref` SHALL be remapped to the
resolved manifest id, keyed by both the parser's file id (XMEML) and the
filename (FCPXML) so either parser's clips resolve.

#### Scenario: existing media relinked, missing media registered

- **WHEN** an imported timeline references `top.mp4` (already in the library) and `music.wav` (not present)
- **THEN** the video clip's `media_ref` SHALL resolve to the existing `top.mp4` entry id, and `music.wav` SHALL be newly registered and referenced by the audio clip

##### Example: relink counts

- **GIVEN** library media `top.mp4`, and an XMEML export whose clips reference `top.mp4` and `music.wav`
- **WHEN** the file is imported
- **THEN** relinked = 1 and registered = 1, and both clips' `media_ref` resolve to a manifest entry

<!-- @trace
source: xml-import-app-wiring
updated: 2026-07-11
code:
  - crates/app_shell_gpui/src/app_root.rs
  - crates/app_shell_gpui/src/timeline_import.rs
  - crates/app_shell_gpui/src/menu.rs
  - crates/app_shell_gpui/src/lib.rs
  - crates/agent_contract/src/tool_exec.rs
  - crates/app_shell_gpui/src/help_view.rs
-->
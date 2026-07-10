## Why

The XMEML and FCPXML import parsers (`render_core::xml_import`) are pure core with no caller — nothing in the running app can actually import a timeline, which is the "半套/偽完成" state the project rules forbid and exactly what "可以當剪輯軟體用了嗎" probes. This change makes import a real, reachable product feature without touching any on-disk contract or the agent/MCP tool surface.

## What Changes

- New **Import Timeline** action (File menu, ⌘⇧I) that opens a file picker, reads an XMEML/FCPXML file, detects the format from content (falling back to extension), and imports it.
- New pure module `app_shell_gpui::timeline_import`:
  - `import_timeline_from_xml(exec, content, format)` — parses via `xml_import::import_xml`, relinks each referenced file to the media library, remaps every clip's `media_ref` to the resolved manifest id, and adopts the result as a new active timeline.
  - Media relink: match an existing library entry by filename first (dedup); else register the referenced path via the `import_media` tool so a missing file still shows offline rather than being dropped. The relink map is keyed by BOTH the parser's file id (XMEML convention) and the filename (FCPXML convention) since the two parsers set `clip.media_ref` differently.
  - `strip_file_url` (handles `file://` / `file://localhost` / Windows `file:///C:/…`) and content/extension format detection.
- New executor method `ToolExecutor::adopt_timeline(timeline) -> id`: swaps in an externally-produced timeline as active, keeps the previously active one as a sibling (import NEVER overwrites open work), assigns a fresh id when missing, clears undo, bumps the revision (so every view resyncs).
- Help/shortcut surfaces updated (`help_view`, `menu` list + shortcut + tests).

## Non-Goals

- New agent/MCP tool for import (this is a UI action; the tool surface stays at its converged v2 count).
- Nested-sequence / retimed / keyframe / title reconstruction (inherited parser v1 limits; surfaced as notes).
- Persisting the imported timeline to `project.json` beyond the existing sibling-save path (autosave already covers it).
- Premiere/Resolve XML (parsers still `NotImplemented`).

## Capabilities

### New Capabilities

- `timeline-import-action`: the app-reachable import flow — file pick → parse → relink media → adopt as a new active timeline.

## Impact

- Affected code:
  - New: `crates/app_shell_gpui/src/timeline_import.rs`
  - Modified: `crates/app_shell_gpui/src/{lib.rs,menu.rs,app_root.rs,help_view.rs}`, `crates/agent_contract/src/tool_exec.rs` (`adopt_timeline` + test)
- No on-disk contract change, no new dependency, no agent/MCP tool-surface change. `adopt_timeline` is a new public executor method (non-breaking).

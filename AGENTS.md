# Fronda Rust-first repo rules

Primary implementation: `Fronda`, a cross-platform Rust app. Legacy behavioral reference: Palmier Pro on Swift 6.2, SwiftUI + AppKit, AVFoundation.

The Swift baseline targets macOS 26, arm64 only, non-sandboxed Developer ID. **Fronda is the primary codebase and is cross-platform** - it builds and runs on macOS, Windows, and Linux. UI is built with `gpui-ce` and must visually match the Swift version exactly. UI work does not require a macOS machine; develop and verify on any platform using gpui.

## Build

Primary Rust workflow:

```bash
cargo test --workspace
cargo check -p fronda-app-shell-gpui --features desktop-app --bin fronda
```

Legacy Swift compatibility baseline:

```bash
swift build
swift run
```

## Code style

- Keep comments minimal. Only write one when the _why_ is non-obvious. Don't restate what the code does, don't narrate the current change, don't leave `// removed X` breadcrumbs. One short line max - no multi-line comment blocks or paragraph docstrings.

## Design System

All UI styling MUST use `AppTheme` constants from `Sources/PalmierPro/UI/AppTheme.swift`. Never use hardcoded numeric values for:

- **Spacing/padding** -> `AppTheme.Spacing.*` (xxs through xxl)
- **Font sizes** -> `AppTheme.FontSize.*` (xxs through display)
- **Font weights** -> `AppTheme.FontWeight.*` (regular, medium, semibold, bold)
- **Corner radii** -> `AppTheme.Radius.*` (xs through xl)
- **Border widths** -> `AppTheme.BorderWidth.*` (hairline, thin, medium, thick)
- **Opacity** -> `AppTheme.Opacity.*` (subtle, faint, muted, medium, strong, prominent)
- **Icon frame sizes** -> `AppTheme.IconSize.*` (xs through xl)
- **Shadows** -> `AppTheme.Shadow.*` (sm, md, lg) via `.shadow(AppTheme.Shadow.md)`
- **Colors** -> `AppTheme.Text.*`, `AppTheme.Border.*`, `AppTheme.Background.*`
- **Animation durations** -> `AppTheme.Anim.*`

If a needed value doesn't exist in AppTheme, add it there first - don't hardcode it.

## Drag and drop

SwiftUI `.onDrop` on a parent view shadows every drop target inside its layout area on macOS 26 - even AppKit `NSDraggingDestination` children registered directly with the window. Inner `.onDrop` modifiers silently never fire while a parent `.onDrop` is active.

Rule: **any drop target that spans an area containing other drop targets must use native AppKit** (see `MediaPanelDropArea` in `Sources/PalmierPro/MediaPanel/`). Inner / leaf drops can stay SwiftUI `.onDrop`. Do not stack SwiftUI `.onDrop` modifiers in parent/child layouts.

## Voice

For Rust-side product and UI copy, Fronda speaks like a quietly capable desktop editor for filmmakers: direct, technical, calm, and confident. Prefer Apple HIG-style terseness over warmth. Never chatty or cute. Never marketing. When the product needs to ask for action, lead with the action verb; when it reports state, name the thing.

## Rust rewrite rules

This repo's primary implementation is the cross-platform Rust app `Fronda`. The current Swift codebase remains the behavioral reference until a Rust implementation explicitly replaces a subsystem.

- Treat `specs/rust-rewrite/` as the compatibility baseline for the rewrite. If behavior changes intentionally, update the relevant spec in the same change and mark the decision explicitly.
- For rewrite work, prefer preserving observable behavior over line-by-line source translation. Port the contract, not the syntax.
- Default Rust UI stack: `gpui-ce`. Use it for windows, panes, focus, input, drag/drop, and app shell behavior.
- `Fronda` is the Rust rewrite name. The current Swift module/resource/runtime identifiers such as `PalmierPro`, `palmier-pro`, and `palmier://` are still compatibility identifiers. Do not rename them piecemeal; any identifier migration must be explicit and spec-backed.
- Keep non-UI logic out of `gpui-ce` whenever possible. Timeline math, persistence, media-library logic, agent/MCP contracts, search/indexing state, and export planning should live in plain Rust crates/modules with no UI dependency.
- Core Rust logic must not depend on platform APIs directly. Wrap platform-specific behavior such as file dialogs, notifications, secure credential storage, updater hooks, trash/reveal-in-file-manager, and window chrome behind explicit adapters.
- Preserve compatibility with existing on-disk/project contracts unless a spec says otherwise:
  - `.palmier` project packages
  - `project.json`
  - `media.json`
  - `generation-log.json`
  - `chat/*.json`
  - agent tool names and schemas
  - MCP resource/tool surface
- All timeline math remains frame-based. Use integer project frames as the source of truth; never let source-native fps silently replace project fps in editing logic.
- Avoid expanding the Swift app with large new features unless explicitly requested. While the rewrite is in progress, prefer:
  - bug fixes
  - parity/spec capture
  - migration scaffolding
  - compatibility fixes
- When porting a subsystem, bring its tests/spec coverage with it. Do not mark a port complete if the behavior only exists informally in code.
- Test priority for Rust work:
  1. pure unit/property tests for math and state transitions
  2. serde/snapshot tests for file formats and agent/MCP contracts
  3. fixture-based integration tests for import/export/search/transcription/generation flows
  4. `gpui-ce` interaction tests only for behavior that is inherently UI-driven
- Keep Rust modules deterministic and explicit. Prefer small pure functions, explicit data flow, and stable serialized structures over hidden globals or view-owned business logic.
- Generation, account, and export state machines live in `crates/generation_core/` (crate name `generation_core`, package `fronda-generation-core`). Pure state machine logic with no platform dependencies.
- `ClipType::from_extension(ext)` in `core_model` classifies file extensions into clip types (covers aiff, aifc, flac).
- XML export supports source timecode (`SourceTimecode` struct + `format_timecode`/`timecode_tags` functions) in `render_core/src/xml_export.rs`.
- `MediaManifestEntry` has optional `source_timecode_frame`, `source_timecode_quanta`, `source_timecode_drop_frame` fields (upstream PR #136).
- `GenerationInput` implements `Default`.
- `MediaManifest::missing_entry_ids(callback)` returns IDs of entries whose local files are missing. Entries with `cached_remote_url` set are never considered missing (upstream PR #135).
- `ToolExecutor` has `media_offline_ids()`, `is_media_offline()`, and `is_media_unprocessable()` helpers that delegate to `missing_entry_ids()`.

## Upstream PR management

- Upstream PRs were audited once on 2026-06-25 (upstream HEAD `b9b4ad9`).
  Results are in `specs/rust-rewrite/97-upstream-pr-audit.md`.
- Do NOT re-fetch upstream or re-audit PRs unless there are new commits on
  the upstream `main` branch. Check with `git fetch upstream && git --no-pager log upstream/main --oneline | head -5` first.
- Only port upstream PRs that are explicitly requested or contain Rust-relevant
  bug fixes. Swift/AVFoundation/Metal-only PRs are automatically skipped.

## Upstream PR porting status

| PR   | Description                             | Status      | Rust Crate                                   |
| ---- | --------------------------------------- | ----------- | -------------------------------------------- |
| #8   | Colors + Effects via Metal              | DONE        | agent_contract (effects pipeline)            |
| #46  | Shape annotations + animation tools     | DONE        | core_model, agent_contract                   |
| #40  | Transcription language setting          | DONE        | core_model (Timeline.transcription_language) |
| #65  | Font weight in TextStyle                | DONE        | core_model (TextStyle.font_weight)           |
| #92  | Words-per-caption setting               | DONE        | search_core (CaptionConfig)                  |
| #105 | .aifc/.flac import support              | DONE        | core_model (ClipType::from_extension)        |
| #114 | Fix set_clip_properties rotation        | DONE        | timeline_core                                |
| #115 | Fix writePosition keyframe corruption   | DONE        | timeline_core                                |
| #129 | Fix keyframe loss on speed change       | DONE        | timeline_core (keyframes.rs)                 |
| #136 | XMEML source timecode                   | DONE        | render_core (xml_export.rs), core_model      |
| #144 | Validate speed/volume/opacity/trim      | DONE        | agent_contract (mutation.rs)                 |
| #94  | Export resolutions (2K, Match Timeline) | DONE        | render_core (ExportResolution)               |
| #135 | Missing-media cache pattern             | DONE        | core_model, agent_contract                   |
| #74  | naturalTimeScale for clip inserts       | DEFERRED    | AVFoundation-specific                        |
| #119 | Audio syncing multiple tracks           | NOT_STARTED | Swift-only, large feature                    |
| #133 | Project thumbnail main-thread hang      | DEFERRED    | Swift-specific pattern                       |

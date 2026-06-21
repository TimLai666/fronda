# PalmierPro

AI-native macOS video editor. Swift 6.2, SwiftUI + AppKit, AVFoundation. macOS 26 only, arm64 only. Non-sandboxed Developer ID app.

## Build

```bash
swift build
swift run
```

## Code style

- Keep comments minimal. Only write one when the _why_ is non-obvious. Don't restate what the code does, don't narrate the current change, don't leave `// removed X` breadcrumbs. One short line max — no multi-line comment blocks or paragraph docstrings.

## Design System

All UI styling MUST use `AppTheme` constants from `Sources/PalmierPro/UI/AppTheme.swift`. Never use hardcoded numeric values for:

- **Spacing/padding** → `AppTheme.Spacing.*` (xxs through xxl)
- **Font sizes** → `AppTheme.FontSize.*` (xxs through display)
- **Font weights** → `AppTheme.FontWeight.*` (regular, medium, semibold, bold)
- **Corner radii** → `AppTheme.Radius.*` (xs through xl)
- **Border widths** → `AppTheme.BorderWidth.*` (hairline, thin, medium, thick)
- **Opacity** → `AppTheme.Opacity.*` (subtle, faint, muted, medium, strong, prominent)
- **Icon frame sizes** → `AppTheme.IconSize.*` (xs through xl)
- **Shadows** → `AppTheme.Shadow.*` (sm, md, lg) via `.shadow(AppTheme.Shadow.md)`
- **Colors** → `AppTheme.Text.*`, `AppTheme.Border.*`, `AppTheme.Background.*`
- **Animation durations** → `AppTheme.Anim.*`

If a needed value doesn't exist in AppTheme, add it there first — don't hardcode it.

## Drag and drop

SwiftUI `.onDrop` on a parent view shadows every drop target inside its layout area on macOS 26 — even AppKit `NSDraggingDestination` children registered directly with the window. Inner `.onDrop` modifiers silently never fire while a parent `.onDrop` is active.

Rule: **any drop target that spans an area containing other drop targets must use native AppKit** (see `MediaPanelDropArea` in `Sources/PalmierPro/MediaPanel/`). Inner / leaf drops can stay SwiftUI `.onDrop`. Do not stack SwiftUI `.onDrop` modifiers in parent/child layouts.

## Voice

Palmier Pro speaks like a quietly capable native Mac app for filmmakers: direct, technical, calm, and
confident. Prefer Apple HIG-style terseness over warmth. Never chatty or cute. Never marketing. When the
product needs to ask for action, lead with the action verb; when it reports state, name the thing.

## Rust rewrite rules

This repo is being rewritten into a cross-platform Rust app. The current Swift codebase remains the behavioral reference until a Rust implementation explicitly replaces it.

- Treat `specs/rust-rewrite/` as the compatibility baseline for the rewrite. If behavior changes intentionally, update the relevant spec in the same change and mark the decision explicitly.
- For rewrite work, prefer preserving observable behavior over line-by-line source translation. Port the contract, not the syntax.
- Default Rust UI stack: `gpui-ce`. Use it for windows, panes, focus, input, drag/drop, and app shell behavior.
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

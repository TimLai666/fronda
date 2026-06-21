# Rust Rewrite Spec Baseline

This folder is the compatibility baseline for rewriting `fronda` from the current Swift/AppKit/SwiftUI/AVFoundation implementation into a cross-platform Rust application.

## Chosen UI stack

The rewrite target is:

- Rust for the full application codebase
- `gpui-ce` for the UI shell and interaction layer
- pure/non-UI Rust crates for timeline math, persistence, media-library logic, agent contracts, search/indexing state, generation workflows, and export planning wherever possible

`gpui-ce` matters here because it supports:

- GPU-accelerated desktop UI
- cross-platform backends
- action/shortcut handling
- integrated async execution
- test support for UI interactions

That means the rewrite should keep most business logic out of the view layer and reserve `gpui-ce` tests for window/panel/focus/shortcut/drag behavior.

## Source basis

These specs were derived from the current repository state, primarily from:

- `Sources/PalmierPro/**`
- `Tests/PalmierProTests/**`
- `README.md`
- `FAQ.md`
- the current MCP / agent / export / persistence contracts encoded in code

## How to read these specs

Each checklist item is a future acceptance requirement.

Format:

- `[ ] PREFIX-###`: a behavior that should be covered by automated tests in the Rust rewrite
- `Decision:`: a current behavior that exists today but likely needs an explicit product/platform decision during the rewrite

A rewrite milestone should not be considered done until the relevant items are either:

1. covered by passing automated tests, or
2. explicitly replaced by a documented product decision

## Closed-source boundary

Palmier's server-side generative processing is not in this repo. These docs only specify the **observable client-side contract**:

- request shaping
- model selection rules
- placeholder/result lifecycle
- persistence
- media handling
- UI behavior
- agent/MCP behavior
- search/transcription/indexing behavior

They do **not** specify private backend implementation details.

## Document map

- `01-foundation-and-project-model.md`
  - project packages
  - recent-project registry
  - persistence schema
  - media source model
- `02-media-library-and-project-workflows.md`
  - import/finalize
  - folders
  - drag/drop
  - paste
  - clipboard
  - relink
  - save-as-media
  - sample projects
- `03-timeline-editor-and-preview.md`
  - timeline math
  - tracks
  - clip mutations
  - linking
  - ripple/overwrite
  - snapping/range behavior
  - inspector behaviors
  - preview behaviors
- `04-export-rendering-and-interchange.md`
  - composition/export
  - XML interchange
  - self-contained project export
- `05-agent-mcp-and-chat.md`
  - chat sessions
  - mention/context system
  - tool contract
  - MCP server
  - agent panel UX
- `06-search-transcription-generation-and-shell.md`
  - search/indexing
  - transcript cache/search
  - captions
  - generation
  - account/settings/help/app shell
  - telemetry
- `99-test-matrix.md`
  - recommended Rust test layers
  - current Swift evidence
  - biggest current gaps

## Test philosophy for the Rust rewrite

1. **Pure-core unit/property tests first**
   - timeline math
   - overwrite/ripple logic
   - serde round-trips
   - ID shortening
   - search ranking
2. **Snapshot/contract tests second**
   - JSON project files
   - agent tool definitions
   - MCP resources
   - XML export
3. **Fixture-based integration tests third**
   - media import/finalization
   - export/rendering
   - transcription/search/indexing
4. **`gpui-ce` interaction tests last**
   - panel focus
   - shortcuts
   - send/stop behavior
   - mention picker
   - drag/drop routing

## Rewrite rule of thumb

Preserve observable behavior first, improve architecture second.

If the current Swift app has an externally visible behavior, the Rust rewrite should either:

- match it, or
- document why it is intentionally changing

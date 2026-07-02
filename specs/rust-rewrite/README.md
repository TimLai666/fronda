# Fronda Compatibility Spec Baseline

This folder is the compatibility baseline for the Fronda Rust codebase. It captures the observable contract that Fronda preserves from the inherited Swift/AppKit/SwiftUI/AVFoundation implementation unless a spec records an intentional change.

`Fronda` is the active product name of the Rust codebase, but the repo still carries inherited `PalmierPro` / `palmier-*` runtime identifiers. Those identifiers remain part of the compatibility surface until they are migrated explicitly.

## Chosen UI stack

The primary application stack is:

- Rust for the full application codebase
- `gpui-ce` for the UI shell and interaction layer
- pure/non-UI Rust crates for timeline math, persistence, media-library logic, agent contracts, search/indexing state, generation workflows, and export planning wherever possible

`gpui-ce` matters here because it supports:

- GPU-accelerated desktop UI
- cross-platform backends
- action/shortcut handling
- integrated async execution
- test support for UI interactions

That means Fronda should keep most business logic out of the view layer and reserve `gpui-ce` tests for window/panel/focus/shortcut/drag behavior.

## Source basis

These specs were derived from the current repository state, primarily from:

- `Package.swift`
- `Sources/PalmierPro/**`
- `Tests/PalmierProTests/**`
- `README.md`
- `FAQ.md`
- `CONTRIBUTING.md`
- `AGENTS.md`
- the current runtime / packaging / MCP / agent / export / persistence contracts encoded in code

## How to read these specs

Each checklist item is an acceptance requirement for the Rust codebase.

The repo also includes a structural CI guardrail at `scripts/check_rust_rewrite_specs.py` to make sure this spec set does not silently drift while Fronda is being built.

Current executable Rust coverage lives under:

- `crates/core_model/**`
- `crates/project_io/**`
- `crates/timeline_core/**`
- `fixtures/rust-rewrite/projects/**`

Wave-1 now includes fixture-backed `.palmier` save/write parity in `project_io`, and wave-3 has started in `timeline_core` with pure Rust timeline invariant and property tests.

The current `gpui-ce` shell lives under:

- `crates/app_shell_gpui/**`

The desktop shell binary is feature-gated behind `desktop-app` so the core workspace can keep fast portable tests while CI still checks a real `gpui-ce` desktop target.

Format:

- `[ ] PREFIX-###`: a behavior that should be covered by automated tests in Fronda
- `Decision:`: a current behavior that exists today but likely needs an explicit product/platform decision in Fronda

A Fronda milestone should not be considered done until the relevant items are either:

1. covered by passing automated tests, or
2. explicitly replaced by a documented product decision

## Closed-source boundary

Upstream Palmier server-side generative processing is not in this repo. These docs only specify the **observable client-side contract**:

- request shaping
- model selection rules
- placeholder/result lifecycle
- persistence
- media handling
- UI behavior
- agent/MCP behavior
- search/transcription/indexing behavior
- runtime, packaging, and design-token behavior

They do **not** specify private backend implementation details.

## Document map

- `00-runtime-packaging-design-and-shell.md`
  - current Swift/runtime baseline
  - package dependencies and resources
  - bundle, document, URL, and updater metadata
  - startup and app lifecycle
  - window/menu/shortcut contracts
  - AppTheme design tokens
  - settings/help/feedback UX
  - generation catalog schema and cost formulas
  - backend config keys
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
- `10-current-status-and-plan.md`
  - current Rust coverage snapshot
  - active repo positioning
  - staged documentation and implementation plan
- `11-identifier-migration-plan.md`
  - compatibility identifiers to preserve now
  - migration sequencing for Fronda-facing names
  - explicit hold list for appcast/updater/package ids
- `97-upstream-pr-audit.md`
  - audit of all upstream Swift PRs and their Rust porting status
  - Swift test coverage gaps vs Rust
  - recommended next actions
- `98-verification-plan.md`
  - execution waves for converting spec families into Rust tests
  - fixture / snapshot layout guidance
  - definition-of-done gates per verification wave
- `99-test-matrix.md`
  - recommended Rust test layers
  - current Swift evidence
  - biggest current gaps

## Test philosophy for Fronda

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
   - menu/shortcut routing
   - settings/help/feedback window behavior

## Rule of thumb

Preserve observable behavior first, improve architecture second.

If the current Swift app has an externally visible behavior, Fronda should either:

- match it, or
- document why it is intentionally changing

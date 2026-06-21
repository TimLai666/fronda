# Rust Rewrite Test Matrix

This file maps the spec families to recommended Rust test layers and the best current Swift evidence.

## A. Recommended Rust crate/test split

Suggested structure:

- `crates/runtime_contract`
  - package/runtime metadata snapshots
  - bundle/document/updater contract checks
  - design-token snapshots
  - settings/help/menu contract snapshots
  - generation catalog schema and cost formulas
- `crates/core_model`
  - timeline structs
  - clip math
  - keyframes
  - serde compatibility
- `crates/project_io`
  - `.palmier` package read/write
  - manifest/generation-log compatibility
  - recent-project registry
- `crates/media_library`
  - import/finalize bookkeeping
  - folders
  - relink
  - clipboard/paste routing
- `crates/timeline_core`
  - overwrite engine
  - ripple engine
  - link groups
  - project-settings retiming
  - snapping / geometry math
- `crates/render_core`
  - composition planning
  - export sizing
  - XML generation
  - self-contained bundle export
- `crates/agent_contract`
  - tool definitions
  - prompt snapshots
  - id shortening
  - session persistence
  - MCP resources
- `crates/search_core`
  - embedding store
  - frame sampler
  - transcript cache/search
  - caption building
  - generation workflow state machine
- `crates/app_shell_gpui`
  - `gpui-ce` views
  - panel visibility/layout
  - shortcuts
  - mention picker
  - chat send/stop behavior
  - settings/help/feedback/app shell glue

## B. Test-layer rules

- **Unit/property tests** should cover pure math and state transitions.
- **Serde round-trip tests** should lock old `.palmier` compatibility.
- **Snapshot tests** should lock:
  - agent tool definitions
  - agent prompt text
  - MCP resources
  - XML export
  - structured tool outputs
- **Fixture integration tests** should cover:
  - media import/finalize
  - search/indexing
  - transcription
  - rendering/export
- **`gpui-ce` UI tests** should be used only where user interaction itself is the contract.

## C. Family-to-layer mapping

| Spec families                                                     | Preferred Rust test layer                            | Best current Swift evidence                                                                     | Biggest gaps                                        |
| ----------------------------------------------------------------- | ---------------------------------------------------- | ----------------------------------------------------------------------------------------------- | --------------------------------------------------- |
| `RUN`, `PKG`, `BNDL`, `BOOT`, `WIN`, `MENU`, `KEY`, `UIX`, `THM`  | Snapshot + UI contract tests                         | mostly source-only: `Package.swift`, `Info.plist`, `Constants.swift`, `AppTheme.swift`          | package/UI-token drift detection                    |
| `SETUI`, `HELP`, `FBK`, `CAT`, `GPAY`, `COST`, `CFG`              | Snapshot + state-machine/unit tests                  | mostly source-only: `Settings/**`, `Help/**`, `Generation/Catalog/**`, `BackendConfig.swift`    | model catalog/backend config fixtures               |
| `CORE`, `FMT`, `RES`, `PCFG`                                      | Unit + serde round-trip                              | `ProjectRoundTripTests`, `MediaResolverTests`, `ClipMutationsTests`                             | file-package integration coverage                   |
| `PRJ`, `REC`                                                      | Temp-dir integration tests                           | `ProjectRegistryTests`                                                                          | full `VideoProject` open/save integration           |
| `MED`, `FLD`, `DRAG`, `PST`, `CCB`, `RLK`, `SAV`, `SMP`, `PSET`   | Temp-dir integration + some UI routing tests         | `MediaPanelTests`, `LottieImportTests`, `SegmentTrimTests`                                      | relink, drag/drop routing, save-as-media flows      |
| `TIM`, `TRK`, `CLP`, `LNK`, `RPL`, `SNP`, `RNG`, `INS`            | Pure-core unit/property tests                        | `Tests/PalmierProTests/Timeline/**`                                                             | some interaction glue still only implied by UI code |
| `PRV`, `RND`, `TXT`, `EXP`, `XML`, `BND`, `PAR`                   | Fixture integration + snapshot/golden tests          | `Tests/PalmierProTests/Export/**`, `Tests/PalmierProTests/Rendering/**`                         | preview/export golden parity across backends        |
| `TDEF`, `SES`, `MNT`, `AID`, `READ`, `MUT`, `UNDO`, `MCP`, `CHAT` | Snapshot + unit + contract tests                     | `Tests/PalmierProTests/Agent/**`                                                                | MCP transport, session lifecycle, panel UX          |
| `SRCH`, `TRN`, `CAP`                                              | Unit + fixture integration tests                     | `Tests/PalmierProTests/Search/**`, `Tests/PalmierProTests/Captions/**`, `TranscriptSearchTests` | loader lifecycle, some cache/install failure paths  |
| `EDT`, `GEN`, `ACC`, `SET`, `APP`, `TEL`                          | State-machine tests + selective UI/integration tests | mostly source-only, little/no direct test coverage                                              | largest current coverage gap                        |

## D. Existing Swift evidence by area

### Strongly covered already

- Timeline math and editing engines
  - `Tests/PalmierProTests/Timeline/ClipMathTests.swift`
  - `Tests/PalmierProTests/Timeline/ClipMutationsTests.swift`
  - `Tests/PalmierProTests/Timeline/KeyframeTests.swift`
  - `Tests/PalmierProTests/Timeline/LinkingTests.swift`
  - `Tests/PalmierProTests/Timeline/OverwriteEngineTests.swift`
  - `Tests/PalmierProTests/Timeline/RippleDeleteRangesTests.swift`
  - `Tests/PalmierProTests/Timeline/RippleEngineTests.swift`
  - `Tests/PalmierProTests/Timeline/RippleGapDeleteTests.swift`
  - `Tests/PalmierProTests/Timeline/SnapEngineTests.swift`
  - `Tests/PalmierProTests/Timeline/TimelineGeometryTests.swift`
  - `Tests/PalmierProTests/Timeline/TimelineRangeSelectionTests.swift`
  - `Tests/PalmierProTests/Timeline/TrackDisplayLabelTests.swift`
- Export/rendering/interchange
  - `Tests/PalmierProTests/Export/**`
  - `Tests/PalmierProTests/Rendering/**`
- Search/transcription/captions
  - `Tests/PalmierProTests/Search/**`
  - `Tests/PalmierProTests/Captions/**`
  - `Tests/PalmierProTests/Transcription/TranscriptSearchTests.swift`
- Agent tool contract
  - `Tests/PalmierProTests/Agent/**`
- Media-panel routing and manifest basics
  - `Tests/PalmierProTests/Media/MediaPanelTests.swift`
  - `Tests/PalmierProTests/Media/ProjectRegistryTests.swift`
  - `Tests/PalmierProTests/Media/ProjectRoundTripTests.swift`
  - `Tests/PalmierProTests/Media/MediaResolverTests.swift`

### Weakly covered or mostly uncovered today

These should become first-wave Rust acceptance tests:

1. full `.palmier` package open/save integration from the document layer
2. relink flows
3. save-clip-as-media and save-timeline-range-as-media
4. sample-project materialization
5. search model loader lifecycle
6. generation placeholder/upload/job/download lifecycle
7. rerun / AI-edit / upscale availability rules
8. account misconfiguration, billing whitelist, and credit logic
9. app-shell flows:
   - notifications
   - changelog gating
   - update badge
   - feedback screenshot capture
10. telemetry enable/disable + launch-latched behavior
11. MCP transport and resource listing
12. chat-session tab lifecycle and agent-panel UX under `gpui-ce`
13. runtime/package metadata and Info.plist drift checks
14. AppTheme/design-token snapshot checks
15. generation model catalog schema and cost-estimation fixtures

## E. What should use `gpui-ce` test support

Use `gpui-ce` UI tests for behavior that is inherently interactive and view-bound:

- pane visibility / maximize / layout preset switching
- main-menu command routing and keyboard shortcut routing between timeline and media panel
- agent input send/stop behavior
- mention picker open/close/navigation/insert
- media-panel selection navigation
- drag/drop routing where the contract is event-level rather than pure math

Do **not** push pure timeline/search/project logic into `gpui-ce` tests if a normal unit/property test can cover it.

## F. Release gate for “behaviorally compatible”

A Rust rewrite milestone should only be called behaviorally compatible when:

1. all relevant spec families for the shipped surface have passing automated tests,
2. all snapshot contracts are reviewed and stable,
3. all intentional deltas are listed as explicit product decisions,
4. cross-platform substitutions do not silently drop current user-visible behavior

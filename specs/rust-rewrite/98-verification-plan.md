# Rust Rewrite Verification Plan

This file is the execution index for turning the compatibility specs in this folder into passing automated tests for `Fronda`.

Use it together with:

- `README.md` for the scope and document map
- `99-test-matrix.md` for crate/layer recommendations
- `.github/workflows/ci.yml` for the current automation entry point
- `scripts/check_rust_rewrite_specs.py` for spec-baseline linting

## A. What is already automated

Current CI now protects two things:

1. the Swift baseline still builds and tests cleanly,
2. the Rust rewrite spec set remains structurally sane.

The spec lint is intentionally narrow for now. It validates:

- all required spec files exist,
- the spec README still maps the full document set,
- checklist IDs stay unique across the family docs,
- each family doc still contains executable acceptance items.

That is not product verification yet. It is only the guardrail that keeps the rewrite baseline from drifting before Rust implementation begins.

## B. Tracking rules for the rewrite

The numbered spec docs remain the source of truth for acceptance requirements.

Status meanings:

- `[ ]` not yet proven by Rust automation
- `[x]` implemented in Rust and passing in CI
- `Decision:` intentionally changed behavior; keep the decision in the relevant family doc

When Rust tests are added, each automated check should point back to the spec IDs it covers through one of these mechanisms:

- test module / test function names
- snapshot file names
- fixture manifest metadata
- test-case tables in the Rust crate README or tracking docs

Keep the linkage explicit enough that a failed test can be traced back to one or more spec IDs without guesswork.

## C. Suggested test asset layout

When the Rust workspace appears, keep compatibility fixtures outside UI crates where possible.

Suggested layout:

- `crates/*/src/**`
- `crates/*/tests/**`
- `tests/fixtures/projects/**`
- `tests/fixtures/media/**`
- `tests/fixtures/transcripts/**`
- `tests/fixtures/search/**`
- `tests/fixtures/xml/**`
- `tests/snapshots/**`

Suggested naming pattern:

- `spec_core_fmt.rs`
- `spec_media_relink.rs`
- `spec_timeline_ripple.rs`
- `spec_export_xml.rs`
- `spec_agent_tools.rs`
- `spec_search_transcripts.rs`
- `spec_shell_gpui.rs`

Prefer small files grouped by contract family over giant end-to-end test files.

## D. Execution waves

These waves are the recommended order for turning the spec set into executable Rust acceptance tests.

| Wave | Primary scope                     | Spec families                                                                                                                                             | Preferred automation first                        | Likely Rust targets                  |
| ---- | --------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------- | ------------------------------------ |
| 0    | Baseline guardrails               | all docs indirectly                                                                                                                                       | Swift baseline CI + spec lint                     | repo root, CI, fixtures              |
| 1    | Core data and persistence         | `CORE`, `FMT`, `RES`, `PCFG`, `PRJ`, `REC`                                                                                                                | unit + serde + temp-dir integration               | `core_model`, `project_io`           |
| 2    | Media library workflows           | `MED`, `FLD`, `DRAG`, `PST`, `CCB`, `RLK`, `SAV`, `SMP`, `PSET`                                                                                           | temp-dir integration + fixture tests              | `media_library`, `project_io`        |
| 3    | Timeline editing core             | `TIM`, `TRK`, `CLP`, `LNK`, `RPL`, `SNP`, `RNG`, `INS`                                                                                                    | unit + property tests                             | `timeline_core`                      |
| 4    | Preview, rendering, export        | `PRV`, `RND`, `TXT`, `EXP`, `XML`, `BND`, `PAR`                                                                                                           | fixture integration + snapshots                   | `render_core`                        |
| 5    | Agent, MCP, chat contracts        | `TDEF`, `SES`, `MNT`, `AID`, `READ`, `MUT`, `UNDO`, `MCP`, `CHAT`                                                                                         | contract tests + snapshots + unit tests           | `agent_contract`, `app_shell_gpui`   |
| 6    | Search, transcription, generation | `SRCH`, `TRN`, `CAP`, `GEN`                                                                                                                               | fixture integration + state-machine tests         | `search_core`, generation crates     |
| 7    | Runtime and shell UX              | `RUN`, `PKG`, `BNDL`, `BOOT`, `WIN`, `MENU`, `KEY`, `UIX`, `THM`, `SETUI`, `HELP`, `FBK`, `CAT`, `GPAY`, `COST`, `CFG`, `EDT`, `ACC`, `SET`, `APP`, `TEL` | snapshots + selective `gpui-ce` interaction tests | `runtime_contract`, `app_shell_gpui` |

## E. First acceptance backlog

These are the highest-value checks to land early even if the corresponding subsystem is only partially ported.

1. `.palmier` open/save compatibility fixtures
2. `project.json`, `media.json`, `generation-log.json`, and `chat/*.json` serde round-trips
3. relink flows, including rejection cases
4. save-clip-as-media and save-selection-as-media
5. timeline ripple / overwrite / speed-change property tests
6. composition-planning tests for audio/video insert timing
7. XML export snapshots
8. agent tool definition snapshots
9. MCP resource listing / schema contract tests
10. transcript cache and transcript search fixtures
11. generation placeholder → upload → result lifecycle tests
12. app-shell notification / changelog / update badge state tests
13. AppTheme / design-token snapshots for the rewrite shell
14. editor pane layout / maximize / restore tests under `gpui-ce`
15. menu / shortcut routing tests under `gpui-ce`
16. drag/drop tests only where event routing itself is the contract

## F. Definition of done for a wave

A wave is only done when all of the following are true:

1. every mapped spec family has at least one Rust automated test path,
2. all fixtures and snapshots needed for that wave are committed and stable,
3. CI runs those checks on every push and pull request,
4. every intentional behavior delta is recorded as a `Decision:` in the relevant spec doc,
5. no behavior is called compatible based only on manual testing.

## G. What should stay out of `gpui-ce` tests

Use `gpui-ce` tests only for behavior that is inherently interactive:

- focus routing
- pane visibility and layout
- menu/shortcut dispatch
- mention picker behavior
- chat send/stop UX
- drag/drop routing

Do not move pure timeline math, file persistence, export planning, or search ranking into UI tests if a normal Rust unit/integration test can prove the same contract.

## H. Near-term repo tasks

As Rust implementation starts, the next concrete repo changes should be:

1. add a Rust workspace with crate boundaries that match `99-test-matrix.md`,
2. add fixture directories under `tests/fixtures/`,
3. add the first serde compatibility tests for project files,
4. add pure-core timeline property tests,
5. add snapshot support for XML and agent/MCP contracts,
6. add `gpui-ce` tests only after non-UI cores are already isolated.

## I. Release gate for calling Fronda compatible

Do not call a Rust milestone behaviorally compatible unless:

- the relevant spec IDs have passing automated tests,
- the compatibility fixtures are checked in,
- the remaining uncovered IDs are explicitly listed,
- any Palmier identifier migration is intentional and documented,
- platform substitutions do not silently remove user-visible behavior.

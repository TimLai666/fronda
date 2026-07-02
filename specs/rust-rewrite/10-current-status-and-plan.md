# Fronda Current Status and Repair Plan

Last reviewed: 2026-06-29

## Positioning

This repo is now documented as a Rust-first project:

- `Fronda` is the primary product and implementation.
- The Rust workspace under `crates/` is the active development surface.
- The inherited Swift app remains in-repo as a compatibility reference and a legacy runtime target.
- Compatibility specs under `specs/rust-rewrite/` are the contract Fronda must satisfy or explicitly change.

## Current Rust footprint

Workspace members currently include:

- `agent_contract`
- `app_contract`
- `app_shell_gpui`
- `mcp_server`
- `core_model`
- `project_io`
- `render_core`
- `search_core`
- `timeline_core`
- `generation_core`
- `media_library`
- `audio_core`
- `search_visual`

## Verified baseline

As of this review:

- `cargo test --workspace` passes across the Rust workspace.
- `cargo check -p fronda-app-shell-gpui --features desktop-app --bin fronda` passes.
- CI already runs Rust tests, `gpui-ce` shell compile checks, spec validation, and the legacy Swift baseline.

## What was inconsistent before this pass

The repo previously had several documentation mismatches:

1. top-level docs treated Rust as a future rewrite rather than the active codebase
2. Rust shell docs still described the app shell as a scaffold or placeholder effort
3. model/docs text still referred to the Swift app as the primary consumer
4. CI job names still framed the Rust workspace as rewrite-only rather than the main product surface
5. identifier migration was implied in several places, but not written down as an explicit policy

## What this documentation pass changes

This pass establishes a stable repo narrative:

1. README, CONTRIBUTING, FAQ, AGENTS, and spec entrypoints are Rust-first
2. legacy Swift instructions remain available, but are clearly secondary
3. CI wording now reflects Fronda as the main workspace
4. app shell and model docs no longer describe the Rust side as a placeholder effort
5. identifier migration is tracked explicitly in `11-identifier-migration-plan.md`

## Repair plan

### Phase 1: Documentation baseline

Done in this pass:

1. switch root docs to Rust-first wording
2. keep Swift docs only as compatibility/reference sections
3. add explicit status and migration docs

### Phase 2: Contract cleanup

Partly done, with more cleanup still worth doing:

1. continue replacing future-tense "rewrite" wording inside detailed spec files when it refers to the active Fronda codebase rather than hypothetical future work
2. normalize encoding-corrupted legacy punctuation inside older spec files where it hurts readability
3. add cross-links from subsystem READMEs to the relevant spec families
4. keep legacy Swift mentions only where they describe an actual compatibility contract, not repo positioning

### Phase 3: Implementation proof

After wording is stable:

1. expand Rust acceptance coverage for media-library workflows, search lifecycle, app shell interactions, and account/settings flows
2. record remaining uncovered spec families in one place before claiming behavioral compatibility
3. keep using Swift only as evidence and fallback, not as the repo's primary narrative

## Scope guard

This plan does **not** mean every inherited Palmier identifier should be renamed immediately.

The following remain compatibility-sensitive and should change only via an explicit migration:

- `.palmier`
- `PalmierPro`
- `palmier-pro`
- `palmier://`
- Sparkle feed metadata and updater contracts

See `11-identifier-migration-plan.md`.

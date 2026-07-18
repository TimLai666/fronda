# generation-backend Specification

## Purpose

TBD - created by archiving change 'generation-backend-d6'. Update Purpose after archive.

## Requirements

### Requirement: Generation tools submit through a configurable backend

When a generation backend is installed, generate_video/generate_image/generate_audio SHALL submit the request through the backend's submit seam and register a pending media entry (generation_status "generating" with the returned backend job id) rather than returning the unavailable error; when no backend is installed they SHALL return the existing honest "requires a remote API" error unchanged. The pending entry SHALL be completable by the existing generation-recovery path.

#### Scenario: Submit with a backend installed

- **WHEN** generate_image runs with a backend installed and valid args
- **THEN** the backend receives a submit call, a media entry with generation_status "generating" and the backend job id is added, and the tool returns a non-error result carrying the mediaRef

#### Scenario: No backend keeps the honest error

- **WHEN** any generate tool runs with no backend installed
- **THEN** it returns the existing unavailable error and adds no media entry


<!-- @trace
source: generation-backend-d6
updated: 2026-07-18
code:
  - crates/app_shell_gpui/src/lib.rs
  - specs/rust-rewrite/98-generation-protocol.md
  - crates/app_shell_gpui/src/editor_state_hub.rs
  - AGENTS.md
  - specs/rust-rewrite/99-decisions-2026-07-17.md
  - crates/agent_contract/src/tool_exec.rs
  - crates/app_shell_gpui/src/http_generation_backend.rs
  - crates/generation_core/src/lib.rs
-->

---
### Requirement: Configurable HTTP generation backend

A concrete HTTP backend SHALL implement submit and poll against Fronda Generation Protocol v1 (POST /v1/generate, GET /v1/jobs/{id}, bearer auth), resolving its base URL and token from configuration; missing configuration SHALL yield no backend (honest error preserved). Request building and response parsing SHALL be pure and unit-tested; the live round-trip needs a configured endpoint and is not auto-tested.

#### Scenario: Poll maps job status to outcome

- **WHEN** the poll endpoint returns status "succeeded" with resultUrls
- **THEN** resume_job returns a Success outcome carrying those URLs; "failed" maps to Failure; "queued"/"running" return an error so the entry stays pending for a later retry

#### Scenario: Missing config yields no backend

- **WHEN** the generation URL or token is absent
- **THEN** from_config returns None and the generate tools keep the honest error

<!-- @trace
source: generation-backend-d6
updated: 2026-07-18
code:
  - crates/app_shell_gpui/src/lib.rs
  - specs/rust-rewrite/98-generation-protocol.md
  - crates/app_shell_gpui/src/editor_state_hub.rs
  - AGENTS.md
  - specs/rust-rewrite/99-decisions-2026-07-17.md
  - crates/agent_contract/src/tool_exec.rs
  - crates/app_shell_gpui/src/http_generation_backend.rs
  - crates/generation_core/src/lib.rs
-->
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

---
### Requirement: Self-hosted generation gateway with pluggable providers

A gateway service SHALL implement Fronda Generation Protocol v1.1 (submit, poll, and a providers catalog) and route each request to a provider selected by kind and optional provider name, defaulting per kind when unspecified. Providers SHALL be registered behind a common trait so multiple providers per kind coexist, each with its own bring-your-own-key configuration. Phase 1 SHALL ship stub providers so the full submit→poll→succeeded loop runs with no external keys.

#### Scenario: Stub loop completes end to end

- **WHEN** a client submits a generate request to the gateway and polls the returned job id
- **THEN** the poll transitions running → succeeded and returns a result URL, with no external provider key required

#### Scenario: Provider selection and defaulting

- **WHEN** a request omits the provider field
- **THEN** the gateway routes to the kind's default provider; an unknown named provider yields an explicit error

#### Scenario: Auth is enforced

- **WHEN** a request omits or mismatches the bearer token
- **THEN** the gateway responds 401 and does not run the job

<!-- @trace
source: generation-gateway-stub
updated: 2026-07-18
code:
  - crates/app_shell_gpui/src/http_generation_backend.rs
  - crates/generation_gateway/Cargo.toml
  - crates/generation_gateway/src/stub.rs
  - specs/rust-rewrite/99-decisions-2026-07-17.md
  - specs/rust-rewrite/VERIFY-2026-07-18-generation-gateway.md
  - crates/generation_gateway/src/jobs.rs
  - crates/generation_gateway/src/server.rs
  - crates/generation_gateway/src/main.rs
  - Cargo.toml
  - crates/generation_gateway/src/registry.rs
  - crates/generation_gateway/src/protocol.rs
  - crates/generation_gateway/src/lib.rs
  - AGENTS.md
  - crates/generation_gateway/src/provider.rs
  - crates/generation_gateway/src/config.rs
  - specs/rust-rewrite/98-generation-protocol.md
tests:
  - crates/generation_gateway/tests/gateway_http.rs
-->

---
### Requirement: Gateway serves generated media and hosts a real image provider

The gateway SHALL store generated media bytes and serve them at `GET /v1/results/{id}` (bearer, correct content-type), so a succeeded job's resultUrls are fetchable real media (not placeholder schemes). A Gemini image provider SHALL implement the provider trait using the Gemini generateContent REST surface with a bring-your-own key, decoding the inlineData image bytes into the result store; it is registered only when a key is configured.

#### Scenario: Real bytes round-trip through the pipeline (key-free)

- **WHEN** a mock provider returns image bytes and a client submits then polls to success
- **THEN** the resultUrl serves those exact bytes with the correct content-type

#### Scenario: Gemini registers only with a key

- **WHEN** no Gemini API key is configured
- **THEN** the image catalog lists only the stub provider and the gateway still starts

<!-- @trace
source: gemini-image-provider
updated: 2026-07-18
code:
  - crates/generation_gateway/src/config.rs
  - crates/generation_gateway/src/lib.rs
  - crates/agent_contract/src/tools.rs
  - crates/generation_gateway/Cargo.toml
  - crates/generation_gateway/src/results.rs
  - crates/app_shell_gpui/src/http_generation_backend.rs
  - crates/generation_gateway/src/main.rs
  - crates/app_shell_gpui/src/generation_view.rs
  - specs/rust-rewrite/VERIFY-2026-07-18-generation-gateway.md
  - crates/generation_core/src/lib.rs
  - specs/rust-rewrite/98-generation-protocol.md
  - crates/agent_contract/src/tool_exec.rs
  - crates/generation_gateway/src/server.rs
  - AGENTS.md
  - specs/rust-rewrite/99-decisions-2026-07-17.md
  - crates/generation_gateway/src/gemini.rs
tests:
  - crates/generation_gateway/tests/gemini_pipeline.rs
-->

---
### Requirement: No-key image provider (Pollinations)

The gateway SHALL register a keyless Pollinations image provider that fetches a generated image from `{base}/prompt/{url-encoded prompt}` (base configurable) and stores the returned bytes in the result store, so real AI media flows end to end with no credential. It coexists with the stub and the (key-gated) Gemini provider; the image default remains the stub.

#### Scenario: Keyless real-media round-trip (mock)

- **WHEN** a client submits with provider "pollinations" against a mock returning JPEG bytes and polls to success
- **THEN** the resultUrl serves those exact bytes with the returned content-type, with no API key anywhere

#### Scenario: Always registered

- **WHEN** the gateway starts with no keys configured
- **THEN** the image catalog lists both stub and pollinations

<!-- @trace
source: pollinations-image-provider
updated: 2026-07-18
code:
  - crates/generation_gateway/src/main.rs
  - crates/generation_gateway/src/server.rs
  - crates/generation_gateway/Cargo.toml
  - specs/rust-rewrite/98-generation-protocol.md
  - AGENTS.md
  - specs/rust-rewrite/VERIFY-2026-07-18-generation-gateway.md
  - crates/generation_gateway/src/config.rs
  - crates/generation_gateway/src/lib.rs
  - specs/rust-rewrite/99-decisions-2026-07-17.md
  - crates/generation_gateway/src/pollinations.rs
tests:
  - crates/generation_gateway/tests/pollinations_pipeline.rs
  - crates/generation_gateway/tests/gemini_pipeline.rs
-->
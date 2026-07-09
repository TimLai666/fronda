# model-catalog Specification

## Purpose

TBD - created by archiving change 'model-catalog-wiring'. Update Purpose after archive.

## Requirements

### Requirement: Real catalog behind list_models

The list_models tool SHALL return the real model catalog defined in generation_core (mirroring the upstream Swift ModelConfig lists field-for-field), filtered by the requested kind, with the hardcoded placeholder list removed.

#### Scenario: Video models listed

- **WHEN** the agent calls list_models for video
- **THEN** the response contains exactly the catalog's video entries with their real ids and display names


<!-- @trace
source: model-catalog-wiring
updated: 2026-07-10
code:
  - AGENTS.md
  - crates/audio_core/src/silence_detector.rs
  - .spectra.yaml
  - crates/app_shell_gpui/src/editor_state_hub.rs
  - crates/timeline_core/src/word_cut.rs
  - crates/agent_contract/Cargo.toml
  - crates/agent_contract/src/tools.rs
  - crates/app_shell_gpui/src/text_input.rs
  - crates/app_shell_gpui/src/generation_view.rs
  - crates/generation_core/src/lib.rs
  - crates/app_shell_gpui/src/main.rs
  - crates/mcp_server/src/json_rpc.rs
  - crates/app_shell_gpui/src/lib.rs
  - crates/app_shell_gpui/src/mcp_service.rs
  - crates/generation_core/src/model_catalog.rs
  - crates/mcp_server/src/server.rs
  - crates/agent_contract/src/tool_exec.rs
  - crates/app_shell_gpui/src/text_area.rs
  - crates/timeline_core/src/lib.rs
  - crates/mcp_server/src/session.rs
  - specs/rust-rewrite/98-ui-parity-audit.md
  - crates/agent_contract/src/lib.rs
  - crates/app_shell_gpui/src/feedback_view.rs
tests:
  - crates/mcp_server/tests/spec_mcp_sessions.rs
  - crates/mcp_server/tests/spec_mcp_contract.rs
  - crates/agent_contract/tests/spec_tool_snapshots.rs
-->

---
### Requirement: Free-tier gating

Model availability SHALL follow upstream #249: a model is available when the account is paid or the model is not paid_only. The paid flag SHALL come from an injected account-state seam; with no seam installed the executor SHALL treat the account as free tier.

#### Scenario: Free tier sees paid model as gated

- **WHEN** no account seam is installed and a paid_only model is listed
- **THEN** the entry is marked unavailable/upgrade-required rather than hidden, and generate with that model returns an explicit gating error

#### Scenario: Paid account passes

- **WHEN** the account seam reports paid
- **THEN** paid_only models list as available and generate accepts them

<!-- @trace
source: model-catalog-wiring
updated: 2026-07-10
code:
  - AGENTS.md
  - crates/audio_core/src/silence_detector.rs
  - .spectra.yaml
  - crates/app_shell_gpui/src/editor_state_hub.rs
  - crates/timeline_core/src/word_cut.rs
  - crates/agent_contract/Cargo.toml
  - crates/agent_contract/src/tools.rs
  - crates/app_shell_gpui/src/text_input.rs
  - crates/app_shell_gpui/src/generation_view.rs
  - crates/generation_core/src/lib.rs
  - crates/app_shell_gpui/src/main.rs
  - crates/mcp_server/src/json_rpc.rs
  - crates/app_shell_gpui/src/lib.rs
  - crates/app_shell_gpui/src/mcp_service.rs
  - crates/generation_core/src/model_catalog.rs
  - crates/mcp_server/src/server.rs
  - crates/agent_contract/src/tool_exec.rs
  - crates/app_shell_gpui/src/text_area.rs
  - crates/timeline_core/src/lib.rs
  - crates/mcp_server/src/session.rs
  - specs/rust-rewrite/98-ui-parity-audit.md
  - crates/agent_contract/src/lib.rs
  - crates/app_shell_gpui/src/feedback_view.rs
tests:
  - crates/mcp_server/tests/spec_mcp_sessions.rs
  - crates/mcp_server/tests/spec_mcp_contract.rs
  - crates/agent_contract/tests/spec_tool_snapshots.rs
-->
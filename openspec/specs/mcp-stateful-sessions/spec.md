# mcp-stateful-sessions Specification

## Purpose

TBD - created by archiving change 'mcp-stateful-sessions'. Update Purpose after archive.

## Requirements

### Requirement: Session-routed requests

The MCP HTTP server SHALL create a session on initialize, return its id in the Mcp-Session-Id response header, and route subsequent requests carrying that header to the same session state. A request with an unknown or expired session id SHALL receive a JSON-RPC error without touching any session.

#### Scenario: Initialize opens a session

- **WHEN** a client sends initialize without a session header
- **THEN** the response carries a new Mcp-Session-Id and the store tracks the session

#### Scenario: Expired session rejected

- **WHEN** a request carries a session id older than the TTL
- **THEN** the server responds with a JSON-RPC error identifying the invalid session and creates no new state


<!-- @trace
source: mcp-stateful-sessions
updated: 2026-07-10
code:
  - crates/agent_contract/Cargo.toml
  - crates/mcp_server/src/server.rs
  - crates/generation_core/src/model_catalog.rs
  - crates/app_shell_gpui/src/editor_state_hub.rs
  - crates/agent_contract/src/tools.rs
  - crates/app_shell_gpui/src/mcp_service.rs
  - crates/app_shell_gpui/src/lib.rs
  - crates/generation_core/src/lib.rs
  - AGENTS.md
  - crates/app_shell_gpui/src/generation_view.rs
  - crates/app_shell_gpui/src/text_area.rs
  - crates/agent_contract/src/tool_exec.rs
  - crates/mcp_server/src/json_rpc.rs
  - crates/app_shell_gpui/src/feedback_view.rs
  - crates/app_shell_gpui/src/main.rs
  - crates/timeline_core/src/lib.rs
  - crates/timeline_core/src/word_cut.rs
  - crates/audio_core/src/silence_detector.rs
  - specs/rust-rewrite/98-ui-parity-audit.md
  - crates/mcp_server/src/session.rs
  - crates/app_shell_gpui/src/text_input.rs
  - crates/agent_contract/src/lib.rs
  - .spectra.yaml
tests:
  - crates/agent_contract/tests/spec_tool_snapshots.rs
  - crates/mcp_server/tests/spec_mcp_sessions.rs
  - crates/mcp_server/tests/spec_mcp_contract.rs
-->

---
### Requirement: Legacy sessionless compatibility

Requests without an Mcp-Session-Id header SHALL keep today's single-shared-executor behavior so existing clients continue to work unchanged.

#### Scenario: Old client without header

- **WHEN** a client that never sends the header calls a tool
- **THEN** the call executes against the shared executor exactly as before this change


<!-- @trace
source: mcp-stateful-sessions
updated: 2026-07-10
code:
  - crates/agent_contract/Cargo.toml
  - crates/mcp_server/src/server.rs
  - crates/generation_core/src/model_catalog.rs
  - crates/app_shell_gpui/src/editor_state_hub.rs
  - crates/agent_contract/src/tools.rs
  - crates/app_shell_gpui/src/mcp_service.rs
  - crates/app_shell_gpui/src/lib.rs
  - crates/generation_core/src/lib.rs
  - AGENTS.md
  - crates/app_shell_gpui/src/generation_view.rs
  - crates/app_shell_gpui/src/text_area.rs
  - crates/agent_contract/src/tool_exec.rs
  - crates/mcp_server/src/json_rpc.rs
  - crates/app_shell_gpui/src/feedback_view.rs
  - crates/app_shell_gpui/src/main.rs
  - crates/timeline_core/src/lib.rs
  - crates/timeline_core/src/word_cut.rs
  - crates/audio_core/src/silence_detector.rs
  - specs/rust-rewrite/98-ui-parity-audit.md
  - crates/mcp_server/src/session.rs
  - crates/app_shell_gpui/src/text_input.rs
  - crates/agent_contract/src/lib.rs
  - .spectra.yaml
tests:
  - crates/agent_contract/tests/spec_tool_snapshots.rs
  - crates/mcp_server/tests/spec_mcp_sessions.rs
  - crates/mcp_server/tests/spec_mcp_contract.rs
-->

---
### Requirement: SSE notifications

The server SHALL accept an event-stream request tied to a session and deliver notifications over it, including notifications/tools/list_changed whenever the advertised tool surface changes.

#### Scenario: Tool surface change broadcast

- **WHEN** the advertised tool availability changes while a session holds an open event stream
- **THEN** that stream receives a notifications/tools/list_changed event

<!-- @trace
source: mcp-stateful-sessions
updated: 2026-07-10
code:
  - crates/agent_contract/Cargo.toml
  - crates/mcp_server/src/server.rs
  - crates/generation_core/src/model_catalog.rs
  - crates/app_shell_gpui/src/editor_state_hub.rs
  - crates/agent_contract/src/tools.rs
  - crates/app_shell_gpui/src/mcp_service.rs
  - crates/app_shell_gpui/src/lib.rs
  - crates/generation_core/src/lib.rs
  - AGENTS.md
  - crates/app_shell_gpui/src/generation_view.rs
  - crates/app_shell_gpui/src/text_area.rs
  - crates/agent_contract/src/tool_exec.rs
  - crates/mcp_server/src/json_rpc.rs
  - crates/app_shell_gpui/src/feedback_view.rs
  - crates/app_shell_gpui/src/main.rs
  - crates/timeline_core/src/lib.rs
  - crates/timeline_core/src/word_cut.rs
  - crates/audio_core/src/silence_detector.rs
  - specs/rust-rewrite/98-ui-parity-audit.md
  - crates/mcp_server/src/session.rs
  - crates/app_shell_gpui/src/text_input.rs
  - crates/agent_contract/src/lib.rs
  - .spectra.yaml
tests:
  - crates/agent_contract/tests/spec_tool_snapshots.rs
  - crates/mcp_server/tests/spec_mcp_sessions.rs
  - crates/mcp_server/tests/spec_mcp_contract.rs
-->
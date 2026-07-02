# mcp-runtime Specification

## Purpose

TBD - created by archiving change 'wire-mcp-server-into-rust-shell'. Update Purpose after archive.

## Requirements

### Requirement: MCP server starts automatically with the desktop app

When the Fronda desktop app launches and the MCP enabled preference is on (or unset), the shell SHALL start the MCP server on a background thread bound to 127.0.0.1:19789. The gpui main thread MUST NOT block on server startup or operation.

#### Scenario: Launch with preference unset

- **WHEN** the app launches and the MCP enabled preference has never been set
- **THEN** the MCP server starts and an HTTP JSON-RPC initialize request to http://127.0.0.1:19789/mcp receives a valid initialize response

#### Scenario: Launch with preference disabled

- **WHEN** the app launches and the MCP enabled preference is off
- **THEN** the MCP server is not started and connections to 127.0.0.1:19789 are refused


<!-- @trace
source: wire-mcp-server-into-rust-shell
updated: 2026-07-02
code:
  - crates/agent_contract/src/tool_exec.rs
  - crates/mcp_server/src/server.rs
  - crates/app_shell_gpui/src/lib.rs
  - crates/app_shell_gpui/src/editor_state_hub.rs
  - crates/app_shell_gpui/src/agent_panel_view.rs
  - crates/app_shell_gpui/src/mcp_service.rs
-->

---
### Requirement: MCP server identifies as fronda

The MCP server SHALL report `fronda` as its server name: `McpConfig::default().server_name` MUST equal `"fronda"`, and the JSON-RPC initialize response MUST return `serverInfo.name` = `"fronda"`. All other protocol surface (port 19789, tool names and schemas, `palmier://` resource URIs) MUST remain unchanged.

#### Scenario: Initialize response carries the new name

- **WHEN** a client sends an initialize request to the running server
- **THEN** the response contains serverInfo.name equal to "fronda"

##### Example: Identity fields

| Field | Value |
| ----- | ----- |
| serverInfo.name | fronda |
| serverInfo.version | 1.0.0 |
| default bind address | 127.0.0.1:19789 |
| resource URI scheme | palmier:// (unchanged) |


<!-- @trace
source: wire-mcp-server-into-rust-shell
updated: 2026-07-02
code:
  - crates/agent_contract/src/tool_exec.rs
  - crates/mcp_server/src/server.rs
  - crates/app_shell_gpui/src/lib.rs
  - crates/app_shell_gpui/src/editor_state_hub.rs
  - crates/app_shell_gpui/src/agent_panel_view.rs
  - crates/app_shell_gpui/src/mcp_service.rs
-->

---
### Requirement: Settings toggle starts and stops the server at runtime

The settings UI SHALL expose an MCP server on/off toggle backed by the existing MCP enabled preference key. Turning the toggle off SHALL stop the running server and release the port; turning it on SHALL start the server, without requiring an app restart. Stop MUST be idempotent: stopping an already-stopped server MUST NOT error.

#### Scenario: Disable at runtime

- **WHEN** the user turns the MCP toggle off while the server is running
- **THEN** the server stops, the port is released (a new listener can bind 127.0.0.1:19789), and subsequent connection attempts are refused

#### Scenario: Re-enable at runtime

- **WHEN** the user turns the MCP toggle back on after disabling it
- **THEN** the server starts again and initialize requests succeed


<!-- @trace
source: wire-mcp-server-into-rust-shell
updated: 2026-07-02
code:
  - crates/agent_contract/src/tool_exec.rs
  - crates/mcp_server/src/server.rs
  - crates/app_shell_gpui/src/lib.rs
  - crates/app_shell_gpui/src/editor_state_hub.rs
  - crates/app_shell_gpui/src/agent_panel_view.rs
  - crates/app_shell_gpui/src/mcp_service.rs
-->

---
### Requirement: Agent panel reflects real server status

The agent panel status indicator SHALL reflect the actual MCP server state using the existing `McpServerStatus` states: Starting while the server is binding, Running once accepting connections, Stopped when disabled, and Failed with an error message when startup fails.

#### Scenario: Successful startup

- **WHEN** the server binds and begins accepting connections
- **THEN** the agent panel shows the Running status

#### Scenario: Port already in use

- **WHEN** another process already occupies 127.0.0.1:19789 at startup
- **THEN** the status becomes Failed with an error message naming the bind failure, and the rest of the app remains fully functional

<!-- @trace
source: wire-mcp-server-into-rust-shell
updated: 2026-07-02
code:
  - crates/agent_contract/src/tool_exec.rs
  - crates/mcp_server/src/server.rs
  - crates/app_shell_gpui/src/lib.rs
  - crates/app_shell_gpui/src/editor_state_hub.rs
  - crates/app_shell_gpui/src/agent_panel_view.rs
  - crates/app_shell_gpui/src/mcp_service.rs
-->
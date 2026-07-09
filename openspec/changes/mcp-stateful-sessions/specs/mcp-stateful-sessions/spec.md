## ADDED Requirements

### Requirement: Session-routed requests

The MCP HTTP server SHALL create a session on initialize, return its id in the Mcp-Session-Id response header, and route subsequent requests carrying that header to the same session state. A request with an unknown or expired session id SHALL receive a JSON-RPC error without touching any session.

#### Scenario: Initialize opens a session

- **WHEN** a client sends initialize without a session header
- **THEN** the response carries a new Mcp-Session-Id and the store tracks the session

#### Scenario: Expired session rejected

- **WHEN** a request carries a session id older than the TTL
- **THEN** the server responds with a JSON-RPC error identifying the invalid session and creates no new state

### Requirement: Legacy sessionless compatibility

Requests without an Mcp-Session-Id header SHALL keep today's single-shared-executor behavior so existing clients continue to work unchanged.

#### Scenario: Old client without header

- **WHEN** a client that never sends the header calls a tool
- **THEN** the call executes against the shared executor exactly as before this change

### Requirement: SSE notifications

The server SHALL accept an event-stream request tied to a session and deliver notifications over it, including notifications/tools/list_changed whenever the advertised tool surface changes.

#### Scenario: Tool surface change broadcast

- **WHEN** the advertised tool availability changes while a session holds an open event stream
- **THEN** that stream receives a notifications/tools/list_changed event

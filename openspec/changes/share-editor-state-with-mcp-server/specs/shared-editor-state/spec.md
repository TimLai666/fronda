## ADDED Requirements

### Requirement: MCP server and UI share a single editor state

The shell SHALL own exactly one `ToolExecutor` behind `Arc<Mutex<...>>` (the `EditorStateHub`), and the MCP server SHALL operate on that shared instance rather than constructing its own. A mutation made through an MCP tool call MUST be visible to subsequent reads from either side.

#### Scenario: MCP mutation is visible to MCP reads

- **WHEN** an MCP client calls a mutation tool (e.g. create_folder) and then calls get_timeline
- **THEN** the get_timeline response reflects the mutation

#### Scenario: External state change is visible over MCP

- **WHEN** shell code locks the shared executor and modifies the timeline, and an MCP client then calls get_timeline
- **THEN** the response reflects the externally applied change

### Requirement: Tool executor exposes a revision counter

`ToolExecutor` SHALL expose `revision() -> u64` that strictly increases after each successful mutating tool execution. Read-only tools (the existing read-tool family such as get_*/list_*/search_*) MUST NOT increment the revision. A tool call that returns an error MUST NOT increment the revision.

#### Scenario: Mutation increments revision

- **WHEN** a mutating tool executes successfully
- **THEN** revision() returns a value greater than before the call

#### Scenario: Read-only tool leaves revision unchanged

- **WHEN** a read-only tool such as get_timeline executes successfully
- **THEN** revision() returns the same value as before the call

##### Example: Revision transitions

| Action | Revision before | Revision after |
| ------ | --------------- | -------------- |
| get_timeline (read) | 0 | 0 |
| create_folder (mutation, success) | 0 | 1 |
| split_clip with missing args (error) | 1 | 1 |
| load_project | 1 | 2 |

### Requirement: Project load replaces shared state without server restart

`EditorStateHub::load_project(timeline, media_manifest)` SHALL replace the shared executor's timeline and media manifest in place, clear the undo stack, and increment the revision. A running MCP server MUST serve the new project state on the next request without being restarted.

#### Scenario: Load project while server is running

- **WHEN** load_project is called while the MCP server is running, and an MCP client then calls get_timeline
- **THEN** the response reflects the newly loaded timeline

### Requirement: MCP toggle restart preserves shared state

Stopping and restarting the MCP server via the settings toggle SHALL NOT reset the shared editor state: the restarted server MUST operate on the same `Arc<Mutex<ToolExecutor>>` instance.

#### Scenario: State survives server restart

- **WHEN** the user disables the MCP toggle, re-enables it, and an MCP client calls get_timeline
- **THEN** the response reflects the state as it was before the server was stopped

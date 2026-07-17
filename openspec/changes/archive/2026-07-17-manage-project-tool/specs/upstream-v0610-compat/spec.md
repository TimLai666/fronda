## ADDED Requirements

### Requirement: manage_project consolidates the MCP project tools

The MCP tool surface SHALL expose a single manage_project tool (action = list | open | create | close) replacing get_projects/open_project/new_project/close_project, with per-action unknown-key validation, a name/id/path exactly-one selector for open (UUID-format id check, case-insensitive unique name resolution), and list rows carrying a visible field that equals active under Fronda's single-open-project model (upstream #299; MCP tool count 56 → 53, in-app surface unchanged).

#### Scenario: Open by case-insensitive name

- **WHEN** manage_project is called with action "open" and a name differing only in case from one registered project
- **THEN** that project opens; an ambiguous name yields an explicit error

#### Scenario: Unknown keys are rejected per action

- **WHEN** manage_project is called with action "list" plus an unrelated key
- **THEN** the call fails validation instead of silently ignoring the key

# ai-edit-tab Specification

## Purpose

TBD - created by archiving change 'ai-edit-tab-functional'. Update Purpose after archive.

## Requirements

### Requirement: Real upscale catalog

The Upscale picker SHALL list the transcribed upstream upscale model catalog from generation_core (not hardcoded rows), and selecting a model SHALL persist in the tab state.

#### Scenario: Catalog drives the picker

- **WHEN** the user opens the Upscale picker
- **THEN** the rows are exactly the catalog's upscale entries with their real names


<!-- @trace
source: ai-edit-tab-functional
updated: 2026-07-10
code:
  - crates/agent_contract/src/tool_exec.rs
-->

---
### Requirement: Actions dispatch real tools

With a media asset selected, Upscale SHALL call upscale_media, Music SHALL call generate_music, and Sound Effects SHALL call generate_audio through the shared executor; tool errors and backend-unavailable results SHALL surface as an explicit status line, never a fake progress state.

#### Scenario: No backend

- **WHEN** the user triggers Music with no generation backend connected
- **THEN** the tab shows the tool's explicit unavailable message and no spinner runs

#### Scenario: No selection

- **WHEN** no media asset is selected
- **THEN** the action rows render disabled and clicks do nothing


<!-- @trace
source: ai-edit-tab-functional
updated: 2026-07-10
code:
  - crates/agent_contract/src/tool_exec.rs
-->

---
### Requirement: Rerun replays generation input

For an AI-generated asset, Rerun SHALL rebuild the generate call from the asset's stored generation_input (same model and parameters) and dispatch it; assets without generation_input SHALL show Rerun disabled.

#### Scenario: Rerun a generated clip

- **WHEN** the user hits Rerun on an asset whose generation_input holds model and prompt
- **THEN** the corresponding generate tool is invoked with those recorded parameters

<!-- @trace
source: ai-edit-tab-functional
updated: 2026-07-10
code:
  - crates/agent_contract/src/tool_exec.rs
-->
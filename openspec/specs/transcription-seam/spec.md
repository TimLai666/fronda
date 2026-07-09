# transcription-seam Specification

## Purpose

TBD - created by archiving change 'transcription-provider-seam'. Update Purpose after archive.

## Requirements

### Requirement: Provider seam

The executor SHALL accept an injected TranscriptionProvider that turns a media source into word-level stamps (word text, start/end seconds in source time) honoring the timeline's transcription_language; with no provider installed, transcription-dependent flows keep today's behavior (empty words, "No transcribable speech").

#### Scenario: No provider installed

- **WHEN** a transcription-dependent tool runs with no provider and no injected words
- **THEN** it returns the existing "No transcribable speech" boundary error unchanged


<!-- @trace
source: transcription-provider-seam
updated: 2026-07-10
code:
  - crates/agent_contract/src/tools.rs
  - crates/generation_core/src/model_catalog.rs
  - crates/app_shell_gpui/src/editor_state_hub.rs
  - crates/agent_contract/src/lib.rs
  - crates/agent_contract/Cargo.toml
  - AGENTS.md
  - crates/audio_core/src/silence_detector.rs
  - crates/app_shell_gpui/src/generation_view.rs
  - crates/app_shell_gpui/src/text_input.rs
  - crates/mcp_server/src/session.rs
  - crates/app_shell_gpui/src/feedback_view.rs
  - crates/app_shell_gpui/src/lib.rs
  - crates/timeline_core/src/word_cut.rs
  - crates/app_shell_gpui/src/text_area.rs
  - crates/timeline_core/src/lib.rs
  - crates/mcp_server/src/server.rs
  - specs/rust-rewrite/98-ui-parity-audit.md
  - .spectra.yaml
  - crates/generation_core/src/lib.rs
  - crates/app_shell_gpui/src/main.rs
  - crates/app_shell_gpui/src/mcp_service.rs
  - crates/mcp_server/src/json_rpc.rs
  - crates/agent_contract/src/tool_exec.rs
tests:
  - crates/mcp_server/tests/spec_mcp_contract.rs
  - crates/agent_contract/tests/spec_tool_snapshots.rs
  - crates/mcp_server/tests/spec_mcp_sessions.rs
-->

---
### Requirement: Word stamps map to project frames

Transcribing a clip SHALL map each word's source-time stamps into project frames using the clip's placement: source_offset_seconds = trim_start_frame / fps and the clip's speed factor, matching the silence-detector placement convention, and store them as the executor's timeline words.

#### Scenario: Trimmed clip offsets words

- **WHEN** a clip with trim_start_frame 60 at 30fps is transcribed and a word starts at 3.0s source time
- **THEN** the word's project position reflects source second 3.0 minus the 2.0s trim offset scaled by speed, placed relative to the clip's start_frame

##### Example: Placement math at speed 1.0

| trim_start_frame | fps | word start (source s) | clip start_frame | project frame |
|---|---|---|---|---|
| 60 | 30 | 3.0 | 100 | 130 |
| 0 | 30 | 1.0 | 0 | 30 |

<!-- @trace
source: transcription-provider-seam
updated: 2026-07-10
code:
  - crates/agent_contract/src/tools.rs
  - crates/generation_core/src/model_catalog.rs
  - crates/app_shell_gpui/src/editor_state_hub.rs
  - crates/agent_contract/src/lib.rs
  - crates/agent_contract/Cargo.toml
  - AGENTS.md
  - crates/audio_core/src/silence_detector.rs
  - crates/app_shell_gpui/src/generation_view.rs
  - crates/app_shell_gpui/src/text_input.rs
  - crates/mcp_server/src/session.rs
  - crates/app_shell_gpui/src/feedback_view.rs
  - crates/app_shell_gpui/src/lib.rs
  - crates/timeline_core/src/word_cut.rs
  - crates/app_shell_gpui/src/text_area.rs
  - crates/timeline_core/src/lib.rs
  - crates/mcp_server/src/server.rs
  - specs/rust-rewrite/98-ui-parity-audit.md
  - .spectra.yaml
  - crates/generation_core/src/lib.rs
  - crates/app_shell_gpui/src/main.rs
  - crates/app_shell_gpui/src/mcp_service.rs
  - crates/mcp_server/src/json_rpc.rs
  - crates/agent_contract/src/tool_exec.rs
tests:
  - crates/mcp_server/tests/spec_mcp_contract.rs
  - crates/agent_contract/tests/spec_tool_snapshots.rs
  - crates/mcp_server/tests/spec_mcp_sessions.rs
-->
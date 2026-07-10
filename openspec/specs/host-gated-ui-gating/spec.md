# host-gated-ui-gating Specification

## Purpose

TBD - created by archiving change 'host-gated-ui-gating'. Update Purpose after archive.

## Requirements

### Requirement: Executor exposes host-seam availability

`ToolExecutor` SHALL expose `is_generation_available()` (true iff a
`GenerationBackend` is installed) and `is_transcription_available()` (true iff a
`TranscriptionProvider` is installed) so UI surfaces can gate up front without
attempting a tool call.

#### Scenario: flags track installed seams

- **WHEN** an executor has no backend or provider installed
- **THEN** both `is_generation_available()` and `is_transcription_available()` SHALL be false; installing a `GenerationBackend` SHALL make only `is_generation_available()` true


<!-- @trace
source: host-gated-ui-gating
updated: 2026-07-11
code:
  - crates/app_shell_gpui/src/media_panel_view.rs
  - crates/agent_contract/src/tool_exec.rs
  - crates/app_shell_gpui/src/ai_edit_tab_view.rs
  - crates/app_shell_gpui/src/generation_view.rs
-->

---
### Requirement: Generation UI reads as coming soon when no backend

With no generation backend, the generation panel and the AI-edit tab SHALL
disable their generate actions and display a "coming soon" notice up front,
rather than accepting a submission that then reports unavailable. When a backend
is installed, behavior SHALL be unchanged.

#### Scenario: no backend disables submit up front

- **WHEN** the generation panel renders with no generation backend
- **THEN** the Generate button SHALL be disabled and a persistent "AI generation is coming soon" status SHALL show, before any submission

#### Scenario: AI-edit actions gated

- **WHEN** the AI-edit tab renders with no generation backend
- **THEN** its generation actions (generate, re-run, upscale) SHALL be disabled with the same coming-soon notice


<!-- @trace
source: host-gated-ui-gating
updated: 2026-07-11
code:
  - crates/app_shell_gpui/src/media_panel_view.rs
  - crates/agent_contract/src/tool_exec.rs
  - crates/app_shell_gpui/src/ai_edit_tab_view.rs
  - crates/app_shell_gpui/src/generation_view.rs
-->

---
### Requirement: Captions UI reads as coming soon when no transcription provider

Auto-captions require transcript words that only a transcription provider
produces. With no provider installed, the Captions tab SHALL disable Generate
Captions and show a "coming soon" notice instead of surfacing an empty "No
transcribable speech" result. When a provider is installed, behavior SHALL be
unchanged.

#### Scenario: no provider disables caption generation

- **WHEN** the Captions tab renders with no transcription provider
- **THEN** Generate Captions SHALL be disabled and an "auto-captions are coming soon" notice SHALL show

<!-- @trace
source: host-gated-ui-gating
updated: 2026-07-11
code:
  - crates/app_shell_gpui/src/media_panel_view.rs
  - crates/agent_contract/src/tool_exec.rs
  - crates/app_shell_gpui/src/ai_edit_tab_view.rs
  - crates/app_shell_gpui/src/generation_view.rs
-->
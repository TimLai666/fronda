## Why

Before the first public release, features that depend on un-connected host backends (AI generation, transcription) shouldn't let a user do work and then hit a dev-ish "no backend is connected" result. They already degrade without crashing, but the failure is discovered only AFTER the user fills a form / clicks Generate. Surface the gate up front as "coming soon" so a first-run user isn't led into a dead end.

## What Changes

- `ToolExecutor::is_generation_available()` / `is_transcription_available()` — cheap seam-presence checks the UI queries on render.
- **Generation panel** (`generation_view`): with no backend, disable the Generate button (`submit_enabled = can_submit && gen_available`) and show a persistent "AI generation is coming soon" status instead of the post-submit "unavailable" message.
- **AI-edit tab** (`ai_edit_tab_view`): gate every generation action (`enabled`, `can_rerun`, upscale) on `gen_available`; show the same coming-soon status.
- **Captions tab** (`media_panel_view`): captions need transcript words a provider produces; gate the Generate Captions button on `is_transcription_available()` and show "Auto-captions are coming soon" instead of an empty "No transcribable speech".
- Treatment is "label + disable, keep visible" (not hide) — preserves discoverability and signals the roadmap.

## Non-Goals

- No behavior change when a backend/provider IS installed (all existing flows unchanged).
- Not removing or hiding the panels/tabs.
- Not connecting any backend (generation/transcription remain host-gated — that's the point).
- No copy change to the agent/MCP tool results (this is UI-only).

## Capabilities

### New Capabilities

- `host-gated-ui-gating`: generation and captions UI read as "coming soon" and disable their actions when their host seam is not connected.

## Impact

- Affected code:
  - Modified: `crates/agent_contract/src/tool_exec.rs` (two availability methods + test), `crates/app_shell_gpui/src/{generation_view.rs,ai_edit_tab_view.rs,media_panel_view.rs}`
- No on-disk contract change, no dependency, no tool-surface change. Two new non-breaking public executor methods.

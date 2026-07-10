# send-feedback Specification

## Purpose

TBD - created by archiving change 'send-feedback-tool'. Update Purpose after archive.

## Requirements

### Requirement: send_feedback agent tool

The agent tool surface SHALL include a `send_feedback` tool taking a `message` string, which submits product feedback through the injected FeedbackSender host seam. When no sender is connected the tool SHALL return an "unavailable" error naming the missing capability, mirroring the remove_silence no-decoder boundary.

#### Scenario: Feedback sent through the seam

- **WHEN** the agent calls send_feedback with a non-empty message and a FeedbackSender is installed
- **THEN** the sender receives a payload containing the message and diagnostics, and the tool reports success

#### Scenario: No sender installed

- **WHEN** send_feedback is called on an executor without a FeedbackSender
- **THEN** the tool returns an error stating feedback is unavailable and no state changes


<!-- @trace
source: send-feedback-tool
updated: 2026-07-11
code:
  - crates/app_shell_gpui/src/theme.rs
  - crates/render_core/src/effects.rs
  - crates/timeline_core/src/compound.rs
  - crates/app_shell_gpui/src/help_view.rs
  - crates/app_shell_gpui/src/project_lister.rs
  - crates/agent_contract/src/prompt_caching.rs
  - .spectra.yaml
  - crates/agent_contract/src/agent_loop.rs
  - crates/app_shell_gpui/src/project_navigator.rs
  - crates/app_shell_gpui/src/global_shortcuts.rs
  - crates/app_shell_gpui/src/main.rs
  - crates/timeline_core/src/inspector.rs
  - crates/timeline_core/src/workflow.rs
  - specs/rust-rewrite/97-upstream-pr-audit.md
  - crates/render_core/src/xml_export.rs
  - crates/timeline_core/src/project_settings.rs
  - crates/app_shell_gpui/src/timeline_import.rs
  - crates/generation_core/src/model_catalog.rs
  - crates/agent_contract/src/envelope.rs
  - crates/app_shell_gpui/src/mcp_service.rs
  - crates/app_shell_gpui/src/app_root.rs
  - crates/agent_contract/Cargo.toml
  - crates/app_shell_gpui/src/generation_view.rs
  - crates/app_shell_gpui/src/video_export.rs
  - crates/agent_contract/src/mention.rs
  - crates/agent_contract/src/timeline_v2.rs
  - crates/audio_core/src/lib.rs
  - crates/audio_core/src/beat_detector.rs
  - crates/app_shell_gpui/src/text_area.rs
  - specs/rust-rewrite/98-ui-parity-audit.md
  - AGENTS.md
  - specs/rust-rewrite/05-agent-mcp-and-chat.md
  - crates/app_shell_gpui/src/menu.rs
  - crates/app_shell_gpui/src/tour_overlay_view.rs
  - crates/app_shell_gpui/src/chat_view.rs
  - crates/generation_core/src/lib.rs
  - crates/timeline_core/src/clip_clipboard.rs
  - crates/timeline_core/src/multicam.rs
  - crates/agent_contract/src/id_short.rs
  - crates/render_core/src/compositor.rs
  - crates/timeline_core/src/word_cut.rs
  - crates/mcp_server/src/session.rs
  - crates/render_core/src/audio_plan.rs
  - crates/agent_contract/src/read_tools.rs
  - crates/app_shell_gpui/src/feedback_view.rs
  - crates/app_shell_gpui/src/ai_edit_tab_view.rs
  - crates/agent_contract/src/organize.rs
  - crates/render_core/src/fcpxml_export.rs
  - crates/project_io/src/lib.rs
  - crates/agent_contract/src/lib.rs
  - crates/media_library/src/lib.rs
  - crates/app_shell_gpui/src/audio_export.rs
  - crates/app_shell_gpui/src/timeline_view.rs
  - crates/timeline_core/src/edit.rs
  - crates/app_shell_gpui/src/media_import.rs
  - crates/render_core/src/text.rs
  - crates/app_shell_gpui/src/export_model.rs
  - crates/app_shell_gpui/src/platform_adapter.rs
  - crates/app_shell_gpui/src/preview_render.rs
  - crates/app_shell_gpui/src/text_input.rs
  - crates/app_shell_gpui/src/lib.rs
  - crates/app_shell_gpui/src/toolbar_view.rs
  - crates/app_shell_gpui/src/field_components.rs
  - crates/app_shell_gpui/src/export_view.rs
  - crates/agent_contract/src/tools.rs
  - crates/app_shell_gpui/src/preview_view.rs
  - crates/agent_contract/src/tool_exec.rs
  - crates/audio_core/src/silence_detector.rs
  - crates/mcp_server/src/server.rs
  - crates/timeline_core/src/drag_payload.rs
  - crates/timeline_core/src/lib.rs
  - crates/app_shell_gpui/src/editor_state_hub.rs
  - crates/core_model/src/timeline.rs
  - crates/mcp_server/src/json_rpc.rs
  - crates/app_shell_gpui/src/context_menu.rs
  - crates/app_shell_gpui/src/anthropic_transport.rs
  - crates/app_shell_gpui/src/matte_writer.rs
  - crates/app_shell_gpui/src/inspector_view.rs
  - crates/render_core/src/xml_import.rs
  - crates/agent_contract/src/mutation.rs
  - crates/timeline_core/src/clip_properties.rs
  - crates/core_model/src/multicam.rs
  - crates/app_shell_gpui/src/media_panel_view.rs
  - crates/render_core/src/lib.rs
  - crates/core_model/src/lib.rs
  - crates/core_model/src/project_file.rs
tests:
  - crates/timeline_core/tests/spec_clip_mutations.rs
  - crates/render_core/tests/spec_xml_export.rs
  - crates/core_model/tests/compatibility.rs
  - crates/agent_contract/src/test_helpers.rs
  - crates/agent_contract/tests/spec_multicam_tools.rs
  - crates/timeline_core/tests/spec_multicam.rs
  - crates/timeline_core/tests/spec_keyframes.rs
  - crates/timeline_core/tests/spec_overwrite.rs
  - crates/mcp_server/tests/spec_mcp_contract.rs
  - crates/timeline_core/tests/spec_timeline_math.rs
  - crates/timeline_core/tests/spec_range_selection.rs
  - crates/timeline_core/tests/spec_ripple_engine.rs
  - crates/agent_contract/tests/spec_tool_snapshots.rs
  - crates/timeline_core/tests/spec_snapping.rs
  - crates/timeline_core/tests/spec_track_ops.rs
  - crates/timeline_core/tests/spec_workflow.rs
  - crates/timeline_core/tests/spec_linking.rs
  - crates/render_core/tests/spec_composition_plan.rs
  - crates/timeline_core/tests/spec_preview_behavior.rs
  - crates/mcp_server/tests/spec_mcp_sessions.rs
-->

---
### Requirement: Session dedup and cap

The executor SHALL reject a message identical to one already sent in the current session, and SHALL reject any send after 8 successful sends in the session (upstream #152 semantics), each with a distinct explanatory error.

#### Scenario: Duplicate message rejected

- **WHEN** send_feedback is called twice with the same message in one session
- **THEN** the second call returns a duplicate-feedback error and the sender is not invoked again

#### Scenario: Session cap reached

- **WHEN** 8 feedbacks were already sent this session and a 9th distinct message is submitted
- **THEN** the tool returns a session-limit error and the sender is not invoked

<!-- @trace
source: send-feedback-tool
updated: 2026-07-11
code:
  - crates/app_shell_gpui/src/theme.rs
  - crates/render_core/src/effects.rs
  - crates/timeline_core/src/compound.rs
  - crates/app_shell_gpui/src/help_view.rs
  - crates/app_shell_gpui/src/project_lister.rs
  - crates/agent_contract/src/prompt_caching.rs
  - .spectra.yaml
  - crates/agent_contract/src/agent_loop.rs
  - crates/app_shell_gpui/src/project_navigator.rs
  - crates/app_shell_gpui/src/global_shortcuts.rs
  - crates/app_shell_gpui/src/main.rs
  - crates/timeline_core/src/inspector.rs
  - crates/timeline_core/src/workflow.rs
  - specs/rust-rewrite/97-upstream-pr-audit.md
  - crates/render_core/src/xml_export.rs
  - crates/timeline_core/src/project_settings.rs
  - crates/app_shell_gpui/src/timeline_import.rs
  - crates/generation_core/src/model_catalog.rs
  - crates/agent_contract/src/envelope.rs
  - crates/app_shell_gpui/src/mcp_service.rs
  - crates/app_shell_gpui/src/app_root.rs
  - crates/agent_contract/Cargo.toml
  - crates/app_shell_gpui/src/generation_view.rs
  - crates/app_shell_gpui/src/video_export.rs
  - crates/agent_contract/src/mention.rs
  - crates/agent_contract/src/timeline_v2.rs
  - crates/audio_core/src/lib.rs
  - crates/audio_core/src/beat_detector.rs
  - crates/app_shell_gpui/src/text_area.rs
  - specs/rust-rewrite/98-ui-parity-audit.md
  - AGENTS.md
  - specs/rust-rewrite/05-agent-mcp-and-chat.md
  - crates/app_shell_gpui/src/menu.rs
  - crates/app_shell_gpui/src/tour_overlay_view.rs
  - crates/app_shell_gpui/src/chat_view.rs
  - crates/generation_core/src/lib.rs
  - crates/timeline_core/src/clip_clipboard.rs
  - crates/timeline_core/src/multicam.rs
  - crates/agent_contract/src/id_short.rs
  - crates/render_core/src/compositor.rs
  - crates/timeline_core/src/word_cut.rs
  - crates/mcp_server/src/session.rs
  - crates/render_core/src/audio_plan.rs
  - crates/agent_contract/src/read_tools.rs
  - crates/app_shell_gpui/src/feedback_view.rs
  - crates/app_shell_gpui/src/ai_edit_tab_view.rs
  - crates/agent_contract/src/organize.rs
  - crates/render_core/src/fcpxml_export.rs
  - crates/project_io/src/lib.rs
  - crates/agent_contract/src/lib.rs
  - crates/media_library/src/lib.rs
  - crates/app_shell_gpui/src/audio_export.rs
  - crates/app_shell_gpui/src/timeline_view.rs
  - crates/timeline_core/src/edit.rs
  - crates/app_shell_gpui/src/media_import.rs
  - crates/render_core/src/text.rs
  - crates/app_shell_gpui/src/export_model.rs
  - crates/app_shell_gpui/src/platform_adapter.rs
  - crates/app_shell_gpui/src/preview_render.rs
  - crates/app_shell_gpui/src/text_input.rs
  - crates/app_shell_gpui/src/lib.rs
  - crates/app_shell_gpui/src/toolbar_view.rs
  - crates/app_shell_gpui/src/field_components.rs
  - crates/app_shell_gpui/src/export_view.rs
  - crates/agent_contract/src/tools.rs
  - crates/app_shell_gpui/src/preview_view.rs
  - crates/agent_contract/src/tool_exec.rs
  - crates/audio_core/src/silence_detector.rs
  - crates/mcp_server/src/server.rs
  - crates/timeline_core/src/drag_payload.rs
  - crates/timeline_core/src/lib.rs
  - crates/app_shell_gpui/src/editor_state_hub.rs
  - crates/core_model/src/timeline.rs
  - crates/mcp_server/src/json_rpc.rs
  - crates/app_shell_gpui/src/context_menu.rs
  - crates/app_shell_gpui/src/anthropic_transport.rs
  - crates/app_shell_gpui/src/matte_writer.rs
  - crates/app_shell_gpui/src/inspector_view.rs
  - crates/render_core/src/xml_import.rs
  - crates/agent_contract/src/mutation.rs
  - crates/timeline_core/src/clip_properties.rs
  - crates/core_model/src/multicam.rs
  - crates/app_shell_gpui/src/media_panel_view.rs
  - crates/render_core/src/lib.rs
  - crates/core_model/src/lib.rs
  - crates/core_model/src/project_file.rs
tests:
  - crates/timeline_core/tests/spec_clip_mutations.rs
  - crates/render_core/tests/spec_xml_export.rs
  - crates/core_model/tests/compatibility.rs
  - crates/agent_contract/src/test_helpers.rs
  - crates/agent_contract/tests/spec_multicam_tools.rs
  - crates/timeline_core/tests/spec_multicam.rs
  - crates/timeline_core/tests/spec_keyframes.rs
  - crates/timeline_core/tests/spec_overwrite.rs
  - crates/mcp_server/tests/spec_mcp_contract.rs
  - crates/timeline_core/tests/spec_timeline_math.rs
  - crates/timeline_core/tests/spec_range_selection.rs
  - crates/timeline_core/tests/spec_ripple_engine.rs
  - crates/agent_contract/tests/spec_tool_snapshots.rs
  - crates/timeline_core/tests/spec_snapping.rs
  - crates/timeline_core/tests/spec_track_ops.rs
  - crates/timeline_core/tests/spec_workflow.rs
  - crates/timeline_core/tests/spec_linking.rs
  - crates/render_core/tests/spec_composition_plan.rs
  - crates/timeline_core/tests/spec_preview_behavior.rs
  - crates/mcp_server/tests/spec_mcp_sessions.rs
-->
## Summary

清掉 2026-07-17 各移植波留下的四個小 follow-ups：(1) `ProjectAudioSource::capture_date_seconds` 的 ffmpeg metadata 實作（讓 app 內 sync 種子窗真正生效）；(2) app_root Home sidebar 換用 #319 的 `sidebar_row_button`；(3) `ai_edit_tab_view` 依 #327 群組化（EditorPanelGroup）；(4) `cmd_set_clip_properties` 的 legacy text 鍵退場（v2 契約明定 text 走 update_text）。

## Non-Goals

- NDF-NTSC frameDuration（data-model 決策）、SkillCatalog/Community、multicam 功能 UI、pixel parity

## Impact

- Affected specs: 無 delta
- Affected code: crates/app_shell_gpui/src/{audio_source.rs,app_root.rs,ai_edit_tab_view.rs}；crates/agent_contract/src/{tool_exec.rs,mutation.rs,tools.rs}

## Summary

決策執行後累積的 follow-ups 三線：(1) settings AI pane 增 `whisperModelPath` 欄位（讀寫 preferences.json，D4 的模型路徑自此有 UI）；(2) multicam mic/overlay 改角——`switch_segment` 以 executor 內部操作暴露（不動 agent tool surface，維持鏡射政策），Multicam tab 補 Mic/Overlay chips；同場補 Swift `handlePanelClick` 的跨面板選取清除語意與 focus ring 淡入（gpui 不可行則如實記錄）；(3) 界外機械性 clippy 清理（僅零行為變更項）。

## Non-Goals

- ProjectSeams 補 transcription provider（agent open_project 邊緣案例，記錄待後續）
- 新 agent 工具（switch_segment 不進 tool surface）

## Impact

- Affected code: (1) crates/app_shell_gpui/src/{settings_view.rs,transcribe.rs helper 重用}；(2) crates/agent_contract/src/tool_exec.rs（內部方法）＋ crates/app_shell_gpui/src/{inspector_view.rs,app_root.rs,editor_view.rs}；(3) 其餘 app_shell 檔案的機械性 clippy

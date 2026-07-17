## Summary

殘餘小件一次掃清：(1) `ProjectSeams` 補 transcription provider（agent open_project 路徑換 root 後 provider 一致）；(2) app_root/inspector_view/chat_view 的 16 個界外機械性 clippy 項；(3) Multicam tab 對 overlay clip 補 Layout chips（Swift context menu 有、D7 只給 program——既有偏差消化）。

## Impact

- Affected code: crates/agent_contract/src/tool_exec.rs（ProjectSeams）；crates/app_shell_gpui/src/{editor_state_hub.rs,app_root.rs,inspector_view.rs,chat_view.rs}

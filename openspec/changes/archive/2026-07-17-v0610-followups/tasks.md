## 1. 實作

- [x] 1.1 [P] app_shell 三項：capture_date_seconds ffmpeg 實作＋pure 解析測試；render_home sidebar 換 sidebar_row_button；ai_edit_tab_view 依 Swift AIEditTab 群組化（panel_components）。驗證：`cargo test -p fronda-app-shell-gpui` 兩相全綠、desktop check。
- [x] 1.2 [P] agent_contract：cmd_set_clip_properties legacy text 鍵退場（先確認上游 v2 schema 無此鍵），拒絕測試＋不回歸。驗證：`cargo test -p fronda-agent-contract` 全綠。
- [x] 1.3 `cargo test --workspace` 全綠；AGENTS.md follow-up 註記消化。驗證：內容審查。

## 1. 實作

- [x] 1.1 依 design 移植 #327：新 panel_components.rs（EditorPanelGroup/EditorActionFooter/controls）、inspector_view 六 tab 與 media_panel_view 三 tab 重組、theme.rs 新常數（Swift AppTheme 逐字）；inspector update_text 遷移 nested style。先寫常數/結構測試再改。驗證：`cargo test -p fronda-app-shell-gpui` 全綠、desktop check 通過。
- [x] 1.2 `cargo test --workspace` 全綠；AGENTS.md porting table 增列 #327（含 inspector 遷移註記）。驗證：內容審查。

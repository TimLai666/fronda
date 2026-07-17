## 1. 實作

- [x] 1.1 依 design「pane_prefs 模組」與「AppRoot 接線」實作 EDT-003 持久化：新 pane_prefs.rs（load/save、保留他鍵、原子寫、pure 測試含壞檔/他鍵保留/round-trip）；app_root.rs 開機載入套用＋toggle/unmaximize 後 save（路徑可注入）。先寫紅測試再實作。驗證：`cargo test -p fronda-app-shell-gpui pane` 全綠。
- [x] 1.2 `cargo test --workspace` 全綠；specs/rust-rewrite/03-timeline-editor-and-preview.md EDT-003 條文補「(implemented 2026-07-17, change pane-visibility-persistence; sizes remain deferred)」註記；AGENTS.md 無需變更。驗證：內容審查。

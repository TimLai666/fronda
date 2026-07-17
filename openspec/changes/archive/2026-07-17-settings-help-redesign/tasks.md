## 1. 實作

- [x] 1.1 依 design 移植 #319 全量（settings panes 重排、Skills UI＋SkillStore::save、MCP help、Shortcuts、Home sidebar row），先寫 skill save/驗證 pure 測試與結構測試再實作。驗證：`cargo test -p fronda-app-shell-gpui` 兩相全綠、desktop check 通過。
- [x] 1.2 `cargo test --workspace` 全綠；AGENTS.md porting table 增列 #319（#199 skills 條目同步更新）。驗證：內容審查。

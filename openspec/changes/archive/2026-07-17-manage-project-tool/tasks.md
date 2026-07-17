## 1. 實作

- [x] 1.1 依 design「manage_project 契約逐字對齊 b8a1491d」實作 spec「manage_project consolidates the MCP project tools」：tools.rs 定義（刪四舊工具、host split 調整、4 處 count 斷言 56→53）、mutation.rs 逐 action 驗證、tool_exec.rs dispatch 與錯誤訊息逐字對齊、ProjectNavigator open-by-name（借 close 的解析模式，mock 同步）、mcp_instructions projectNavigation 新文。先 transplant 上游測試意圖（每 action 成功/驗證失敗、name 解析、visible==active）再實作。驗證：`cargo test -p fronda-agent-contract` 與 `cargo test -p fronda-mcp-server`、`cargo test -p fronda-app-shell-gpui` 全綠。
- [x] 1.2 `cargo test --workspace` 全綠、desktop check 通過；AGENTS.md porting table 增列 #299（含 session-隔離 follow-up 註記）。驗證：內容審查。

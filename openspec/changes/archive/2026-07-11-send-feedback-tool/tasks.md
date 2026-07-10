## 1. Seam 與工具

- [x] 1.1 crates/agent_contract/src/tool_exec.rs：定義 FeedbackPayload（message、app_version、timeline 摘要欄位）與 FeedbackSender trait（send(&FeedbackPayload) -> Result<(), String>）；ToolExecutor 加 feedback_sender: Option<Arc<dyn FeedbackSender>> 與 set_feedback_sender、以及 session 狀態（sent_messages: HashSet<String>、sent_count: usize）
- [x] 1.2 cmd_send_feedback：message 空值驗證、去重檢查、8 次上限檢查、無 sender 回 unavailable 錯誤、成功後記錄 message 與遞增計數；dispatch arm 保持單行 => self. 形式以符合 dispatched_tools 測試啟發式
- [x] 1.3 crates/agent_contract/src/tools.rs：send_feedback ToolDefinition（描述沿用上游語意）；工具數 63→64，更新檔頭歷史與 SYSTEM_INSTRUCTION 一行

## 2. 斷言與測試

- [x] 2.1 更新四處工具數斷言：crates/agent_contract/src/tools.rs、crates/agent_contract/tests/spec_tool_snapshots.rs、crates/mcp_server/src/server.rs、crates/mcp_server/tests/spec_mcp_contract.rs
- [x] 2.2 單元測試：無 sender 錯誤、mock sender 收到 payload、重複訊息拒絕、第 9 次拒絕（計數以成功送出為準）
- [x] 2.3 cargo test --workspace EXIT=0 驗證

## 3. Host 後端（gated，留待後端決策）

- [x] 3.1 後端決策已定（2026-07-11）：**不建 Convex/自有回饋後端**，改為把「送出回饋」連結到 Fronda 的 GitHub issues。FeedbackSender seam 保留（host 仍可裝），產品路徑改走 GitHub，實作見後續 change `feedback-github-link`（App 選單開 issues 頁面、`send_feedback` 無 sender 時回傳 GitHub 指引）。Convex client 任務取消。

## 1. Seam 與工具

- [x] 1.1 crates/agent_contract/src/tool_exec.rs：定義 FeedbackPayload（message、app_version、timeline 摘要欄位）與 FeedbackSender trait（send(&FeedbackPayload) -> Result<(), String>）；ToolExecutor 加 feedback_sender: Option<Arc<dyn FeedbackSender>> 與 set_feedback_sender、以及 session 狀態（sent_messages: HashSet<String>、sent_count: usize）
- [x] 1.2 cmd_send_feedback：message 空值驗證、去重檢查、8 次上限檢查、無 sender 回 unavailable 錯誤、成功後記錄 message 與遞增計數；dispatch arm 保持單行 => self. 形式以符合 dispatched_tools 測試啟發式
- [x] 1.3 crates/agent_contract/src/tools.rs：send_feedback ToolDefinition（描述沿用上游語意）；工具數 63→64，更新檔頭歷史與 SYSTEM_INSTRUCTION 一行

## 2. 斷言與測試

- [x] 2.1 更新四處工具數斷言：crates/agent_contract/src/tools.rs、crates/agent_contract/tests/spec_tool_snapshots.rs、crates/mcp_server/src/server.rs、crates/mcp_server/tests/spec_mcp_contract.rs
- [x] 2.2 單元測試：無 sender 錯誤、mock sender 收到 payload、重複訊息拒絕、第 9 次拒絕（計數以成功送出為準）
- [x] 2.3 cargo test --workspace EXIT=0 驗證

## 3. Host 後端（gated，留待後端決策）

- [ ] 3.1 app_shell_gpui 端 FeedbackSender 實作：上游走 Convex SDK action feedback:send 帶帳號 session 認證——需決定 Convex Rust client 或自有 API 端點後才能實作；在該決策前本 change 完成至 seam 邊界即可 archive，此任務移入後續 change

## Why

`send_feedback` 是 63 個上游 agent 工具中唯一未移植的（audit 2026-07-05 已查證：上游經 Convex SDK action feedback:send 以帳號 session 認證送出）。工具層的邏輯（#152：每 session 去重、8 次/session 上限、附診斷資訊）是純邏輯，可先移植；只有實際傳送需要後端 seam。

## What Changes

- `agent_contract` 新增 `send_feedback` 工具（工具數 63→64）：message 參數、session 內重複訊息去重、8 次/session 上限（超過回明確錯誤）、payload 附診斷（app 版本、時間線摘要）
- 新增 `FeedbackSender` host seam trait（與 MatteWriter/ExportHost 同模式）：`send(FeedbackPayload) -> Result<(), String>`；未接 host 時工具回 "unavailable" 錯誤（與 remove_silence 無 decoder 的邊界一致）
- 工具數斷言更新（4 檔：tools.rs、spec_tool_snapshots.rs、mcp_server/server.rs、spec_mcp_contract.rs）與 SYSTEM_INSTRUCTION 一行說明
- 後端實作（Convex 或自有 API）為 host-gated，不在本 change 範圍

## Non-Goals

- 不實作實際網路傳送（Convex Rust client 或替代後端是獨立決策）
- 不做 UI 的 feedback 表單送出佈線（feedback_view 的 Send 按鈕走同 seam 是後續）

## Capabilities

### New Capabilities

- `send-feedback`: agent 可代使用者送出產品回饋——去重、限額、附診斷，經 FeedbackSender seam 傳送

### Modified Capabilities

(none)

## Impact

- Affected specs: send-feedback（新增）
- Affected code:
  - New: (none)
  - Modified: crates/agent_contract/src/tool_exec.rs, crates/agent_contract/src/tools.rs, crates/agent_contract/tests/spec_tool_snapshots.rs, crates/mcp_server/src/server.rs, crates/mcp_server/tests/spec_mcp_contract.rs
  - Removed: (none)

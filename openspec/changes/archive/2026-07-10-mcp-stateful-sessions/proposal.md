## Why

上游 #250 讓 MCP server 支援有狀態 session（Mcp-Session-Id 路由、SSE 串流、tools/list_changed 通知）。Rust 端純核心已完成（mcp_server::session 的 SessionStore：LRU 32、1 小時 TTL、注入時鐘、單調 seq；parse_session_id），但 HTTP 層仍是單一共享 executor、無 SSE、無變更通知。剩餘部分多為純 Rust 佈線，可自主實作。

## What Changes

- HTTP 層依 Mcp-Session-Id 路由到 per-session 狀態：initialize 產生 session id 並回頭（header），後續請求依 id 取 session；未知/過期 session 回 JSON-RPC 錯誤
- per-session 隔離範圍決策：session 級狀態（undo stack、clip presets、timeline words 等 session-scoped 資料）隔離，媒體庫與 timeline 仍為共享 hub 狀態（與 UI 一致）
- Content-Length framing 與現行讀取迴圈的相容整併
- SSE 端點（GET + Accept: text/event-stream）：保持連線、送出 notifications
- tools/list_changed 通知：工具面變更時（如 host seam 掛載改變可用性）向活躍 SSE session 廣播
- 現有單 executor HTTP 行為對無 session header 的舊 client 維持相容

## Non-Goals

- 不做認證變更（#122 bearer auth 已完成且不動）
- 不做 WebSocket 傳輸

## Capabilities

### New Capabilities

- `mcp-stateful-sessions`: MCP HTTP server 的 session 路由、SSE 串流與 tools/list_changed 通知

### Modified Capabilities

(none)

## Impact

- Affected specs: mcp-stateful-sessions（新增）
- Affected code:
  - New: (none)
  - Modified: crates/mcp_server/src/server.rs, crates/mcp_server/src/session.rs
  - Removed: (none)

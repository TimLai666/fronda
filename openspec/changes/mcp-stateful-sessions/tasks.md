## 1. Session 路由

- [x] 1.1 crates/mcp_server/src/session.rs：SessionStore 補「取用即續命」與過期查詢的明確 API（touch/get 分離若尚未有）；session 狀態結構定義哪些是 per-session（協商後的 client info、SSE 連線 handle、通知佇列）——媒體庫與 timeline 維持共享 hub，不複製 executor
- [x] 1.2 crates/mcp_server/src/server.rs：initialize 產生 session id 並在回應 header 加 Mcp-Session-Id；後續請求 parse_session_id → store 查找，未知/過期回 JSON-RPC error（code 與 message 依 MCP spec 的 invalid session 慣例）；無 header 請求走既有共享路徑（相容性測試釘住）
- [x] 1.3 Content-Length framing：確認現行讀取迴圈對帶 body 的 POST 已正確消費 Content-Length；補齊分塊讀取與過大 body 的上限防護

## 2. SSE 與通知

- [x] 2.1 GET + Accept: text/event-stream 升級為長連線：寫入 event-stream header 後把 TcpStream 註冊到該 session 的通知通道；心跳（註解行）維持連線
- [x] 2.2 tools/list_changed：executor 工具面可用性變化的來源點（host seam 掛載/卸載）觸發廣播——server 端提供 notify_tools_changed()，hub 掛載 seam 時呼叫；對每個活躍 SSE 連線寫 notifications/tools/list_changed 事件，寫入失敗即清理該連線
- [x] 2.3 斷線與 TTL 清理：SSE 連線關閉時自 session 移除；session 過期時關閉其連線

## 3. 驗證

- [x] 3.1 整合測試（現有 spec_mcp_contract 模式）：initialize→header 回傳→帶 id 呼叫 tools/list、未知 id 錯誤、無 header 舊行為、TTL 過期（注入時鐘）
- [x] 3.2 SSE 測試：建立 event-stream 連線後觸發 notify_tools_changed，斷言收到 list_changed 事件框架
- [x] 3.3 cargo test --workspace EXIT=0

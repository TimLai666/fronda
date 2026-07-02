## 1. mcp_server crate：可停止的 server 與改名

- [ ] 1.1 依 design 決策「在 McpServer 增加可停止的執行模式（shutdown handle）」為 McpServer 增加優雅停止能力：啟動後提供 shutdown handle，呼叫 stop 後 accept 迴圈結束、port 釋放，stop 為冪等（重複呼叫不報錯）。驗證：crates/mcp_server 新增測試，啟動 server 後透過 handle 停止，斷言可對 127.0.0.1 同一 port 重新 bind 成功，且重複 stop 不 panic
- [ ] 1.2 實作需求「MCP server identifies as fronda」與 design 決策「server 識別名稱改為 fronda（MCP-001 更新）」：將 McpConfig::default() 的 server_name 由 "palmier-pro" 改為 "fronda"，initialize 回應的 serverInfo.name 隨之為 "fronda"。驗證：更新 spec_mcp_contract.rs 的 MCP-001 測試斷言為 "fronda"，cargo test -p fronda-mcp-server 全數通過

## 2. shell 端 MCP 生命週期 glue

- [ ] 2.1 實作需求「MCP server starts automatically with the desktop app」，依 design 決策「shell 端新增 McpService glue（背景執行緒生命週期管理）」與「沿用 settings_storage 既有的 MCP enabled 偏好 key，預設啟用」：在 crates/app_shell_gpui 新增 mcp_service 模組並在 Cargo.toml 加入 fronda-mcp-server 依賴，app 啟動時讀取 MCP enabled 偏好（app_contract::settings_storage 的 SETUI-011 key，未設定視為啟用），啟用時在背景執行緒啟動 McpServer，gpui 主執行緒不阻塞。驗證：cargo check -p fronda-app-shell-gpui --features desktop-app --bin fronda 通過；本機跑 app 後用 curl 對 http://127.0.0.1:19789/mcp 發 initialize，收到 serverInfo.name = "fronda"
- [ ] 2.2 實作需求「Agent panel reflects real server status」：mcp_service 將狀態變化回寫 AgentPanelModel 的 McpServerStatus，bind 中為 Starting、開始接受連線為 Running、停用為 Stopped、bind 失敗為 Failed 且帶錯誤訊息，bind 失敗不 panic、不影響 app 其他功能。驗證：新增狀態轉換測試涵蓋「偏好未設定視為啟用」與「port 被占用轉 Failed 帶訊息」；手動先占用 19789 再啟動 app，agent panel 顯示紅燈與錯誤訊息且 app 可正常操作

## 3. 設定 UI 切換

- [ ] 3.1 實作需求「Settings toggle starts and stops the server at runtime」：settings 的 Agent 區塊加入 MCP server 開關，綁定 SETUI-011 偏好，關閉時即時停止 server 並釋放 port，開啟時即時啟動，無需重啟 app。驗證：手動在 app 內關閉開關後 curl 連線被拒、重開後 initialize 成功；開關狀態重啟 app 後保持

## 4. 文件與 spec 同步

- [ ] 4.1 更新 README.md 的 Connecting via MCP 章節：移除「Rust shell 尚未啟動 MCP server」過渡說明，client 設定範例（Claude Code / Codex / Cursor）server 名稱改為 fronda，並更新 Compatibility identifiers 章節註明 MCP server name 已遷移為 fronda。驗證：內容審閱，README 中不再出現「MCP server 名稱仍為 palmier-pro」的敘述
- [ ] 4.2 更新 specs/rust-rewrite/05-agent-mcp-and-chat.md 的 MCP-001（server name 記載改為 fronda）與 specs/rust-rewrite/11-identifier-migration-plan.md（把 MCP server name 標記為已遷移項目並記錄決策）。驗證：以 grep 確認兩份 spec 中 MCP server name 的記載為 fronda，且 CI 的 spec 驗證 job 通過

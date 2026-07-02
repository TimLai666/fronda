## 1. agent_contract：revision 計數

- [ ] 1.1 實作需求「Tool executor exposes a revision counter」，依 design 決策「ToolExecutor 增加 revision 計數（agent_contract）」：ToolExecutor 增加 revision() -> u64，唯讀工具（既有 read_tools 家族 get_*/list_*/search_*）成功執行不遞增，其餘工具成功執行遞增，執行失敗不遞增。驗證：crates/agent_contract 新增測試涵蓋 spec 的 Revision transitions 例表前三列（get_timeline 讀取不變、add_track 成功 +1、split_clip 缺參數錯誤不變），cargo test -p fronda-agent-contract 全過

## 2. mcp_server：共享 executor 建構

- [ ] 2.1 依 design 決策「McpServer 增加共享 executor 建構方式（mcp_server）」：新增 McpServer::with_shared_executor(config, Arc<Mutex<ToolExecutor>>)，既有 new(config, executor) 改為包裝後委派、行為不變。驗證：crates/mcp_server 新增測試——以 with_shared_executor 在 ephemeral port 啟動，外部鎖住 executor 修改 timeline 後透過 HTTP tools/call get_timeline 讀到變更（實作需求「MCP server and UI share a single editor state」的 External state change is visible over MCP scenario），反向以 HTTP 呼叫 mutation 工具後外部讀 executor 看到變更；既有測試全過

## 3. shell：EditorStateHub 與掛載

- [ ] 3.1 依 design 決策「shell 新增 EditorStateHub 作為唯一共享狀態（app_shell_gpui）」：新增 crates/app_shell_gpui/src/editor_state_hub.rs，提供 global()、executor()、revision()（Mutex poisoned 時回傳 0 不 panic）、load_project(timeline, manifest)。實作需求「Project load replaces shared state without server restart」：load_project 就地替換 timeline/manifest、清空 undo stack、遞增 revision。驗證：新增測試斷言 load_project 後 executor 內容被替換且 revision 遞增、undo stack 為空；測試使用非 global 實例
- [ ] 3.2 實作需求「MCP toggle restart preserves shared state」，依 design 決策「McpService 掛共享 executor、設定切換不重置狀態」：McpService::start 改用 EditorStateHub 的 Arc 以 with_shared_executor 啟動 server。驗證：新增測試——stop 後再 start，hub 的 executor Arc 以 Arc::ptr_eq 斷言不變；cargo check -p fronda-app-shell-gpui --features desktop-app --bin fronda 通過
- [ ] 3.3 依 design 決策「agent panel 以 revision 觸發更新」：AgentPanelView::render 讀取 EditorStateHub::revision() 並在變動時走既有 cx.notify() 重繪路徑。驗證：cargo check --features desktop-app 通過；手動跑 app，curl 呼叫 add_track 後再 get_timeline 看到新 track（實作需求「MCP server and UI share a single editor state」的 MCP mutation is visible to MCP reads scenario）

## 4. 全面驗證

- [ ] 4.1 跑 cargo test --workspace 與 CI 等效 cargo clippy --workspace --tests -- -D warnings 全過，並手動驗證：跑 app 後 curl 依序呼叫 add_track、get_timeline，回應包含新增的 track；關閉再開啟設定的 MCP 開關後 get_timeline 狀態延續。驗證：上述指令輸出與 curl 回應內容審閱

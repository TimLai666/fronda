## Why

MCP server 目前啟動時自行建立 ToolExecutor::new(Timeline::default(), MediaManifest::default())，操作的是一份與 UI 無關的空白專案狀態：agent 透過 MCP 的讀取工具看不到使用者實際開啟的專案，mutation 工具的變更也不會反映在畫面或存檔中。Swift 版的 MCPService 以 editorProvider closure 對著當前 EditorViewModel 操作，Rust 版需要等價的共享機制，否則 MCP 只是接著假資料的協定殼。

## What Changes

- agent_contract 的 ToolExecutor 增加 revision 計數：每次工具成功執行 mutation 後遞增，讓 UI 能偵測「MCP 改了狀態」而重繪
- mcp_server 的 McpServer 增加共享建構方式：接受外部的 Arc<Mutex<ToolExecutor>>，取代只能自建 executor 的現況；既有 new() 保留
- app_shell_gpui 新增 shell 層共享狀態（EditorStateHub）：擁有唯一一份 Arc<Mutex<ToolExecutor>>，MCP server 啟動時掛這一份；提供 load_project(timeline, manifest) 讓日後專案開啟流程把真實狀態載入，以及 revision() 供 UI 偵測變更
- McpService 啟動 server 時改用 EditorStateHub 的共享 executor（含設定開關重啟後仍掛同一份）
- agent panel 每次 render 讀取 revision，MCP 造成的狀態變更會觸發 UI 更新路徑

## Non-Goals

- 不實作專案開啟／載入流程本身（shell 目前尚無真實專案載入管線，EditorStateHub 只提供載入介面）
- 不改動 MCP 協定表面（tool 名稱、schema、resource、port、server name）
- 不做 UI 端 timeline 畫面與 core_model::Timeline 的完整綁定（timeline view 目前是 scaffold 狀態，另案處理）
- 不處理多視窗、多專案同時開啟的情境
- 不動 legacy Swift 版

## Capabilities

### New Capabilities

- `shared-editor-state`: shell 層唯一共享的 ToolExecutor 狀態（EditorStateHub），MCP server 與 UI 讀寫同一份專案狀態，並以 revision 計數傳遞變更訊號

### Modified Capabilities

(none)

## Impact

- Affected specs: 新增 `shared-editor-state`
- Affected code:
  - Modified:
    - crates/agent_contract/src/tool_exec.rs
    - crates/mcp_server/src/server.rs
    - crates/app_shell_gpui/src/mcp_service.rs
    - crates/app_shell_gpui/src/agent_panel_view.rs
    - crates/app_shell_gpui/src/lib.rs
  - New:
    - crates/app_shell_gpui/src/editor_state_hub.rs
  - Removed: (none)

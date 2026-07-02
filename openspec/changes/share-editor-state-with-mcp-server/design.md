## Context

wire-mcp-server-into-rust-shell 完成後，MCP server 隨 app 啟動並可連線，但 McpService::start 為每次啟動自建 ToolExecutor::new(Timeline::default(), MediaManifest::default())，且 McpServer::new 內部把 executor 包成私有的 Arc<Mutex<ToolExecutor>>，外界無法共持。結果是 MCP 與 UI 各自持有不同狀態。

行為基準是 Swift 版 MCPService：以 editorProvider closure 取得當前 EditorViewModel，工具直接操作使用者開啟的專案。

現況限制：Rust shell 尚無專案載入管線，timeline view 等 UI 仍是 scaffold。因此本 change 的目標是建立「唯一共享狀態＋變更訊號」的管道，讓 MCP 與未來的專案載入流程都掛同一份 ToolExecutor，而不是完成整條 UI 綁定。

## Goals / Non-Goals

**Goals:**

- shell 內只存在一份 ToolExecutor，MCP server 與 UI 共持（Arc<Mutex>）
- MCP mutation 後 UI 有可靠的變更訊號（revision 計數）可觸發重繪
- 提供 load_project 介面，日後專案開啟流程能把真實 timeline/manifest 載入同一份狀態
- 設定開關停開 MCP server 後仍掛同一份狀態，不會重置

**Non-Goals:**

- 專案開啟／載入流程本身
- timeline view 與 core_model::Timeline 的完整渲染綁定
- MCP 協定表面變更、多專案／多視窗、Swift 版

## Decisions

### ToolExecutor 增加 revision 計數（agent_contract）

在 ToolExecutor 增加 u64 revision 欄位與 revision() 讀取方法，execute() 成功回傳且該工具屬於 mutation（會改動 timeline、manifest 或 undo stack 的工具）時遞增。放在 agent_contract 而非 shell，因為「狀態被改過幾次」是狀態本身的屬性，且可純測試。替代方案是在 shell 用 wrapper 包住每次 MCP 呼叫，但 MCP server 內部直接呼叫 executor.execute，wrapper 需要改 server 的呼叫路徑，侵入更大。判斷是否 mutation 以工具執行結果實際造成狀態變更為準則過重，採白名單以外皆視為 mutation 的簡化規則：唯讀工具（get_*、list_*、search_* 等既有 read_tools 家族）不遞增，其餘成功執行即遞增。

### McpServer 增加共享 executor 建構方式（mcp_server）

新增 McpServer::with_shared_executor(config, Arc<Mutex<ToolExecutor>>)，內部欄位型別不變；既有 new(config, executor) 保留並改為包裝後委派，行為不變。替代方案是把 new 的參數直接改成 Arc<Mutex<...>>，會破壞既有測試與呼叫端，且 new 的自包語意對測試仍有用。

### shell 新增 EditorStateHub 作為唯一共享狀態（app_shell_gpui）

新增 editor_state_hub 模組：EditorStateHub 擁有 Arc<Mutex<ToolExecutor>>，提供 executor()（clone Arc）、revision()（鎖後讀取）、load_project(timeline, manifest)（以新狀態重建 executor 內容並遞增 revision）。與 McpService 相同採 process-wide singleton（OnceLock），因為一個 app 只有一份當前專案狀態。替代方案是把 hub 塞進 McpService，但狀態的生命週期長於 MCP server（關掉 MCP 後 UI 仍要用），職責也不同，分開。

### McpService 掛共享 executor、設定切換不重置狀態

McpService::start 改為從 EditorStateHub 取得 Arc 並以 with_shared_executor 啟動。使用者關閉再開啟 MCP 開關時，server 重啟但掛的是同一個 Arc，agent 看到的狀態延續。load_project 需要能在 server 運行中替換內容：因為共享的是 Mutex 內容而非 Arc 本身，load_project 鎖住後就地替換 timeline/manifest/undo stack 即可，MCP 下一次請求自然讀到新專案。

### agent panel 以 revision 觸發更新

AgentPanelView::render 已每幀讀取 McpService 狀態，擴充為同時讀取 hub revision 並存入 view；revision 變動時呼叫 cx.notify() 的既有重繪路徑。這是最小可見整合點，完整的 timeline 畫面綁定屬於後續 change。

## Implementation Contract

- 行為：app 啟動後，透過 MCP 呼叫一個 mutation 工具（例如 create_folder），再呼叫 get_timeline，讀到的是同一份被修改後的狀態；EditorStateHub::revision() 在該次 mutation 後嚴格遞增。呼叫 EditorStateHub::load_project 後，MCP 的 get_timeline 回傳新載入的 timeline 內容，無需重啟 server。關閉 MCP 開關再開啟，get_timeline 仍回傳關閉前的狀態（狀態不因 server 重啟而重置）。
- 介面／資料形狀：ToolExecutor::revision() -> u64；McpServer::with_shared_executor(McpConfig, Arc<Mutex<ToolExecutor>>) -> McpServer；EditorStateHub::global() -> &'static EditorStateHub（內含鎖）、executor() -> Arc<Mutex<ToolExecutor>>、revision() -> u64、load_project(Timeline, MediaManifest)。
- 失敗模式：Mutex poisoned 時 MCP 端沿用既有的 Executor lock poisoned 錯誤回應；hub 的 revision() 在 poisoned 時回傳最後已知值或 0，不 panic。唯讀工具執行不遞增 revision。
- 驗收標準：
  - agent_contract 新測試：唯讀工具執行後 revision 不變、mutation 工具成功後 revision +1、失敗的 mutation 不遞增
  - mcp_server 新測試：以 with_shared_executor 啟動，外部先鎖住 executor 改 timeline，透過 HTTP get_timeline 讀到該變更；反向透過 HTTP mutation 後外部讀 executor 看到變更
  - app_shell_gpui 新測試：load_project 後 executor 內容被替換且 revision 遞增；stop/start McpService 後 hub 的 Arc 不變（Arc::ptr_eq）
  - cargo test --workspace 全過；cargo check -p fronda-app-shell-gpui --features desktop-app --bin fronda 通過
  - 手動驗證：跑 app，curl 呼叫 create_folder 後再 list_folders 看到新資料夾
- 範圍界線：in scope＝revision 計數、共享 executor 建構、EditorStateHub、McpService 掛載、agent panel revision 讀取；out of scope＝專案載入流程、timeline 畫面渲染綁定、多專案、協定變更。

## Risks / Trade-offs

- [MCP 工具長時間持鎖會卡 UI 執行緒的 revision 讀取] → UI 端讀取使用 try_lock 或短暫 lock；MCP 端已有 MCP_TOOL_EXECUTION_TIMEOUT_MS 契約防 runaway
- [白名單式唯讀判定漏列新唯讀工具會造成多餘重繪] → 多餘遞增只影響效能不影響正確性；測試鎖定既有 read_tools 家族
- [singleton hub 讓測試共用全域狀態] → 與 McpService 相同模式：測試一律用非 global 的實例建構
- [load_project 就地替換時 undo stack 語意] → 載入新專案即清空 undo stack，與開新專案的預期一致，寫入 spec scenario

## Context

crates/mcp_server 已完整實作 MCP 協定（HTTP JSON-RPC、TcpListener、預設 127.0.0.1:19789），並有 spec 契約測試（crates/mcp_server/tests/spec_mcp_contract.rs），但 crates/app_shell_gpui 沒有依賴它，桌面 app 執行期間沒有任何 MCP server 在跑。UI 側已存在半成品：app_contract::agent_panel_model::McpServerStatus（Starting / Running / Stopped / Failed）與 agent panel 的狀態燈，settings_storage 也已定義 MCP enabled 偏好 key（SETUI-011），只差真實後端。

行為基準是 Swift 版：AppDelegate 啟動時呼叫 startMCPService()，偏好未設定時預設啟用，設定頁可切換。

另外 McpConfig 預設 server_name 目前是相容名 palmier-pro（MCP-001）。Rust 版 MCP 尚無任何實際使用者，本 change 一併把名稱改為 fronda，作為 identifier migration plan 中第一個明確執行的改名項目。

## Goals / Non-Goals

**Goals:**

- Fronda 桌面 app 啟動時自動啟動 MCP server（偏好啟用時），關閉偏好時可即時停止
- Agent panel 狀態燈反映真實狀態，啟動失敗（如 port 被占用）要可見且不影響 app 其他功能
- MCP server 識別名稱改為 fronda，並同步更新契約測試、rust-rewrite specs 與 README

**Non-Goals:**

- 不改 palmier:// resource URI、.palmier 副檔名、auth callback scheme
- 不改 port、tool 名稱與 schema、resource 內容
- 不做網路曝露（非 loopback）設定 UI
- 不動 legacy Swift 版

## Decisions

### 在 McpServer 增加可停止的執行模式（shutdown handle）

現有 McpServer::start() 是阻塞的無限 accept 迴圈，無法停止。為支援設定切換即時生效，在 mcp_server crate 增加優雅停止能力：start 回傳（或另提供）一個 shutdown handle，內部以 atomic 旗標＋喚醒機制（例如對自身 port 發一個 dummy 連線、或將 listener 設為 non-blocking 輪詢旗標）結束 accept 迴圈。替代方案是「關閉偏好後只提示重啟 app 生效」，實作最省但不符合 Swift 版可即時切換的行為基準，故不採用。

### shell 端新增 McpService glue（背景執行緒生命週期管理）

在 app_shell_gpui 新增一個 mcp_service 模組（對應 Swift 的 MCPService）：負責讀取偏好、建 ToolExecutor、在背景執行緒跑 McpServer、把狀態變化回寫 AgentPanelModel::McpServerStatus。gpui 主執行緒不被阻塞；狀態更新透過既有的 model 更新機制通知 UI。替代方案是把生命週期邏輯直接塞進 app_root，但 app_root 已過大且此邏輯可獨立測試，故獨立成模組。

### 沿用 settings_storage 既有的 MCP enabled 偏好 key，預設啟用

偏好語意對齊 Swift 版：key 未設定時視為啟用。使用 app_contract::settings_storage 既有的 SETUI-011 key，不新造 key。設定 UI 的切換直接呼叫 McpService 的 start/stop。

### server 識別名稱改為 fronda（MCP-001 更新）

McpConfig::default() 的 server_name 由 palmier-pro 改為 fronda，initialize 回應的 serverInfo.name 隨之改變。同步更新 spec_mcp_contract.rs 的 MCP-001 測試斷言、specs/rust-rewrite/05-agent-mcp-and-chat.md 與 11-identifier-migration-plan.md 的記載、README 的 client 設定範例（claude mcp add fronda ...）。不提供舊名 alias：MCP client 是以 URL 連線，名稱只是顯示識別，且 Rust 版尚無存量使用者。

## Implementation Contract

- 行為：啟動 Fronda 桌面 app（偏好啟用或未設定時），對 http://127.0.0.1:19789/mcp POST 一個 initialize JSON-RPC 請求，回應的 serverInfo.name 為 "fronda"。在設定頁關閉 MCP 開關後，同一請求連線被拒絕（server 已停止）；重新開啟後恢復可連。
- 介面／資料形狀：mcp_server crate 對外提供可停止的 server 執行介面（啟動後回傳 shutdown handle，handle 提供 stop 操作）；app_shell_gpui::mcp_service 對 shell 暴露 start/stop/目前狀態；狀態型別沿用 app_contract::agent_panel_model::McpServerStatus，不新增變體。
- 失敗模式：TcpListener bind 失敗（含 port 被占用）時，狀態轉為 Failed 並帶錯誤訊息，agent panel 顯示紅燈；app 其餘功能不受影響，不 panic、不阻塞啟動。stop 為冪等操作，對已停止的 server 呼叫 stop 不報錯。
- 驗收標準：
  - cargo test -p fronda-mcp-server 全數通過，其中 MCP-001 測試斷言 server_name 為 "fronda"
  - mcp_server 新增測試：啟動後可透過 shutdown handle 停止，停止後 port 釋放（可重新 bind）
  - app_shell_gpui 或 app_contract 新增測試：偏好未設定視為啟用、Failed 狀態帶錯誤訊息的狀態轉換
  - cargo check -p fronda-app-shell-gpui --features desktop-app --bin fronda 通過
  - 手動驗證：本機跑 app 後以 curl 對 127.0.0.1:19789/mcp 發 initialize 收到 serverInfo.name = "fronda"
- 範圍界線：in scope＝上述生命週期、狀態回報、改名、對應文件與 spec 更新；out of scope＝palmier:// URI 等其他識別碼、tool/schema 契約、網路曝露 UI、Swift 版。

## Risks / Trade-offs

- [阻塞 accept 迴圈的停止機制做錯會殘留執行緒或占住 port] → 以「停止後可重新 bind 同 port」的測試直接驗證釋放行為
- [ToolExecutor 由 MCP 執行緒與 UI 共用時的狀態一致性] → 沿用 mcp_server 既有 Arc<Mutex<ToolExecutor>> 模式；本 change 不引入新的共用面
- [改名後外部教學或舊文件仍寫 palmier-pro] → README 與 rust-rewrite specs 同一 change 內更新；MCP client 以 URL 連線，名稱不影響既有連線設定的可用性
- [19789 被其他程式占用導致使用者以為功能壞掉] → Failed 狀態在 agent panel 顯示錯誤訊息，明確指出 port 衝突

## Why

Rust 版 Fronda 的 MCP 協定實作已完整移植到 crates/mcp_server（HTTP JSON-RPC、預設 127.0.0.1:19789、loopback-only、支援 bearer token），但 gpui 桌面 shell 沒有依賴此 crate、也沒有任何啟動呼叫，導致 Rust 版執行時 AI agent 完全連不到 MCP，只有 legacy Swift 版可用。Agent panel 的 McpServerStatus UI 也因此沒有真實後端。同時，Rust 版 MCP server 尚未有任何既有使用者，是把 server 識別名稱從相容名 palmier-pro 改為 fronda 的最低成本時機。

## What Changes

- app_shell_gpui 新增對 fronda-mcp-server crate 的依賴，桌面 app 啟動時在背景執行緒自動啟動 MCP server（對齊 Swift 版 AppDelegate 啟動即開的行為）
- 提供「啟用 MCP server」偏好設定，預設開啟；未設定時視為開啟（對齊 Swift 版 io.palmier.pro.mcp.enabled 的預設語意），可在設定 UI 切換，切換即時生效（開＝啟動、關＝停止）
- Agent panel 的 McpServerStatus（Starting / Running / Stopped / Failed）改為反映真實 server 狀態，包含 port 被占用等啟動失敗情形
- **BREAKING**：MCP server 識別名稱由 palmier-pro 改為 fronda（McpConfig 預設值與 initialize 回應的 serverInfo.name）。Rust 版尚無可連線的使用者，實際破壞面僅限文件範例
- 更新 README 的 MCP 章節與 client 設定範例（Claude Code / Codex / Cursor 改用 fronda 名稱），移除「Rust shell 尚未啟動 MCP」的過渡說明
- 更新 specs/rust-rewrite/05-agent-mcp-and-chat.md 與 specs/rust-rewrite/11-identifier-migration-plan.md，把 server name 改名記錄為明確的 spec-backed 決策

## Non-Goals

- 不改動 MCP resource URI scheme palmier:// 、專案副檔名 .palmier 、auth callback scheme palmier://callback 等其他相容識別碼（仍依 identifier migration plan 整批處理）
- 不改動 port 19789、tool 名稱與 schema、resource 內容等既有 MCP 契約
- 不修改 legacy Swift 版的 MCP 行為與名稱
- 不新增網路曝露（非 loopback）相關 UI；McpConfig 既有的 auth token 能力維持原狀

## Capabilities

### New Capabilities

- `mcp-runtime`: Rust 桌面 shell 的 MCP server 生命週期管理（自動啟動、偏好設定開關、狀態回報）與 server 識別名稱 fronda

### Modified Capabilities

(none)

## Impact

- Affected specs: 新增 `mcp-runtime`；同步更新 specs/rust-rewrite/05-agent-mcp-and-chat.md、specs/rust-rewrite/11-identifier-migration-plan.md 中的 server name 記載
- Affected code:
  - Modified:
    - crates/app_shell_gpui/Cargo.toml
    - crates/app_shell_gpui/src/app_root.rs
    - crates/app_shell_gpui/src/agent_panel_view.rs
    - crates/app_shell_gpui/src/settings_view.rs
    - crates/app_contract/src/agent_panel_model.rs
    - crates/app_contract/src/settings_storage.rs
    - crates/mcp_server/src/server.rs
    - crates/mcp_server/tests/spec_mcp_contract.rs
    - README.md
  - New: (視實作而定，可能新增 crates/app_shell_gpui/src/mcp_service.rs 作為 shell 端的 server 生命週期 glue)
  - Removed: (none)

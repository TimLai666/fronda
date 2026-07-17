## Summary

移植上游 #299（`b8a1491d`）：四個 MCP-only 專案管理工具（get_projects/open_project/new_project/close_project）整併為單一 `manage_project`（action=list|open|create|close）。repo 慣例是嚴格鏡射上游 tool surface；上游 v0.6.10 HEAD 確認此形態穩定。MCP 工具數 56→53。

## Motivation

2026-07-17 audit 判定 PORT tier2（M）：上游已整併，Rust 仍是四工具舊面。逐 action unknown-key 驗證、name/id/path 三選一 selector（非空、UUID 格式檢查）、open 支援 name 大小寫不敏感解析、payload 的 visible 欄位、projectNavigation 指示改寫，全部是純契約邏輯，直接對映。

## Proposed Solution

依 `b8a1491d` 逐字對齊 schema/描述/錯誤訊息；`ProjectNavigator::open` 補 name 定址（close 側已有 case-insensitive + ambiguity 邏輯可借用）。三個移植決策（見 design）：visible 欄位在單專案模型輸出等於 active；open 回傳維持現狀（不順手補 snapshot，pre-existing 分歧另記）；MCP per-session 隔離子項維持 DEFERRED（#250）。

## Non-Goals

- MCP per-session executor / boundProject 寫入 guard（#250 deferral 的 follow-up，porting table 註記）
- open 回傳補 fps/resolution/timelines snapshot（pre-existing 分歧，非 #299 造成）
- Swift saveBeforeClosing 修復（NSDocument-only；Rust close 本就無條件同步原子存檔）

## Impact

- Affected specs: `upstream-v0610-compat`（ADDED：manage_project requirement）
- Affected code:
  - Modified: crates/agent_contract/src/{tools.rs,tool_exec.rs,mutation.rs,lib.rs}
  - Modified: crates/app_shell_gpui/src/project_navigator.rs（open-by-name）
  - Modified: 4 個 tool-count 斷言位置（tools.rs、spec_tool_snapshots.rs、spec_mcp_contract.rs、mcp_server/server.rs）

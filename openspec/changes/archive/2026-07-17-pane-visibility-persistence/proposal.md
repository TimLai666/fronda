## Summary

實作 EDT-003 的跨啟動面板 visibility 持久化。spec `03-timeline-editor-and-preview.md` 的 EDT-003 已打勾（「Pane visibility state for media/inspector/agent persists across launches」），但實測 pane.rs 只有 in-session 狀態機、無任何持久化 I/O——spec 記錄與現實不符（2026-07-17 查核）。修法：實作它（而非改 spec——Swift 行為確實持久化）。

## Motivation

editor-shell-parity handoff 的 backlog 明列「Pane visibility and size persistence across launches remains deferred」；EDT-003 是 spec 承諾。`preferences.json`（`fronda_config_dir()`，mcp_service 已用）提供現成落點，無需新 platform adapter。

## Proposed Solution

新 `pane_prefs` 模組：讀/寫 `preferences.json` 的 pane visibility 三鍵（media/inspector/agent），read-modify-write 保留未知鍵（不得清掉 mcp 的鍵），原子寫（temp+rename，沿用 project_io 慣例）。AppRoot 開機載入套用；toggle_pane / maximize-恢復後保存。timeline pane 與 preview 不持久化（Swift 同——EDT-003 只列三面板）。尺寸持久化不在本次（EDT-003 範圍外，另列 backlog）。

## Non-Goals

- 面板尺寸（agent/media/inspector 寬、timeline 高）持久化
- preset 持久化
- preferences schema 重構或 mcp_service 改動

## Impact

- Affected specs: 無 delta（EDT-003 既有條文從「記錄不符」變為真實成立；spec 文字不變）
- Affected code:
  - New: crates/app_shell_gpui/src/pane_prefs.rs
  - Modified: crates/app_shell_gpui/src/{lib.rs,app_root.rs}

## Why

專案已能載入共享狀態且 MCP 可編輯，但沒有任何儲存管線：menu 的 SaveProject（Cmd/Ctrl+S）是空 no-op，MCP 或未來 UI 編輯的成果無法落地，重開 app 即遺失。project_io 已有完整的 bundle 寫入基礎，缺的是「只把共享狀態中的 timeline 與 manifest 寫回已開啟專案」的窄路徑。

## What Changes

- project_io 新增 save_project_state(root, timeline, manifest)：只寫 project.json 與 media.json 兩個檔案，不動 bundle 其他內容（chat、transcripts、media 目錄等），避免以不完整的記憶體狀態覆蓋磁碟上的完整專案
- EditorStateHub 新增 save()：有 project_root 時把共享 executor 的 timeline/manifest 寫回該目錄；沒有開啟中的專案（root 為 None）回傳錯誤說明
- AppRoot 的 SaveProject 選單分支接上 hub.save()，失敗以 eprintln 記錄（錯誤 UI 元件另案）

## Non-Goals

- SaveProjectAs（需要檔案選擇對話框，於 open-dialog change 一併處理）
- dirty 標記與未儲存變更提示
- chat sessions、transcripts、generation log、media 檔案的寫回（共享狀態尚未持有）
- 自動儲存

## Capabilities

### New Capabilities

- `project-save`: 把共享編輯狀態的 timeline 與 manifest 寫回已開啟的 .palmier 專案

### Modified Capabilities

(none)

## Impact

- Affected specs: 新增 `project-save`
- Affected code:
  - Modified:
    - crates/project_io/src/lib.rs
    - crates/app_shell_gpui/src/editor_state_hub.rs
    - crates/app_shell_gpui/src/app_root.rs
  - New: (none)
  - Removed: (none)

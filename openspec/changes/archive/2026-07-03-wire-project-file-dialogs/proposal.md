## Why

OpenProject（Cmd/Ctrl+O）與 SaveProjectAs（Cmd/Ctrl+Shift+S）目前是空 no-op 或只切畫面：AppRoot::open_project_at 已能載入專案但沒有任何 UI 入口能提供路徑，另存新檔也無路可走。gpui-ce 已內建跨平台檔案對話框（App::prompt_for_paths、App::prompt_for_new_path），不需新增依賴即可接通。

## What Changes

- EditorStateHub 新增 save_as(root)：以既有窄儲存把目前狀態寫到新目錄並把該目錄記為新的 project_root
- AppRoot 的 OpenProject 分支改為開啟目錄選擇對話框（.palmier 為目錄套件，directories 模式），選定後走既有 open_project_at；取消則不動作
- AppRoot 的 SaveProjectAs 分支開啟另存對話框（prompt_for_new_path，預設檔名附 .palmier 副檔名），選定後呼叫 hub.save_as；失敗以 eprintln 記錄
- 對話框結果以 gpui 的 cx.spawn 非同步接收，不阻塞 UI 執行緒

## Non-Goals

- 最近專案清單（home view 的 recent projects 仍為 scaffold，另案）
- 儲存成功／失敗的 UI 回饋元件
- 媒體匯入對話框（ImportMedia 選單）
- 對話框的檔案類型過濾細節（gpui PathPromptOptions 不含副檔名過濾）

## Capabilities

### New Capabilities

- `project-file-dialogs`: 以平台檔案對話框開啟與另存 .palmier 專案

### Modified Capabilities

(none)

## Impact

- Affected specs: 新增 `project-file-dialogs`
- Affected code:
  - Modified:
    - crates/app_shell_gpui/src/editor_state_hub.rs
    - crates/app_shell_gpui/src/app_root.rs
  - New: (none)
  - Removed: (none)

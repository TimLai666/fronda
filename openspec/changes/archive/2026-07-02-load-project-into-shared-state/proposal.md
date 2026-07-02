## Why

EditorStateHub 已讓 MCP 與 UI 共用同一份 ToolExecutor，但那份狀態永遠是空白專案：shell 沒有把 .palmier 專案載入共享狀態的管線，timeline 畫面也仍渲染 TimelineState 的寫死示範資料（with_default_tracks），與真實專案狀態完全脫節。project_io::ProjectBundle::open 已能完整讀取 .palmier 套件，core_model::Timeline 與 timeline_model::TimelineState 結構可直接對應，缺的只是中間的橋。

## What Changes

- EditorStateHub 增加 load_bundle(path)：用 project_io::ProjectBundle::open 讀取 .palmier 套件，把 timeline 與 manifest 載入共享 executor（沿用既有 load_project 語意：清 undo、遞增 revision），並記錄目前專案根目錄；讀取失敗回傳錯誤不 panic
- timeline_model 增加純函式對應：從 core_model::Timeline 建出 TimelineState（Track → TrackRow、Clip → ClipSlot、fps、total_frames 取所有 clip 的最大結束 frame），clip 標籤優先用 MediaManifest::display_name_for 的名稱
- TimelineView 改為渲染共享狀態：以 hub revision 偵測變更並重建 TimelineState（保留 zoom/scroll/playhead 等純 view 狀態），空專案顯示空 timeline；寫死示範資料退場為測試用途
- AppRoot 增加 open_project_at(path)：載入成功切到 Editor 畫面，失敗留在原畫面；menu 的 NewProject 改為把空白專案載入 hub 後開 editor（讓 MCP 看到的狀態與畫面一致）

## Non-Goals

- 不做檔案選擇對話框（平台 adapter，OpenProject 選單項的實際檔案挑選另案）
- 不做 UI 端編輯操作回寫（拖曳剪輯等 timeline 互動仍是 scaffold）
- 不載入 chat sessions、transcripts、visual indexes 進共享狀態（ToolExecutor 尚無對應欄位）
- 不做專案儲存回寫（save 管線另案）
- 不動 media panel 的畫面綁定

## Capabilities

### New Capabilities

- `project-load`: 把 .palmier 專案載入共享編輯狀態，並讓 timeline 畫面渲染該狀態

### Modified Capabilities

(none)

## Impact

- Affected specs: 新增 `project-load`
- Affected code:
  - Modified:
    - crates/app_shell_gpui/Cargo.toml
    - crates/app_shell_gpui/src/editor_state_hub.rs
    - crates/app_shell_gpui/src/timeline_model.rs
    - crates/app_shell_gpui/src/timeline_view.rs
    - crates/app_shell_gpui/src/app_root.rs
  - New: (none)
  - Removed: (none)

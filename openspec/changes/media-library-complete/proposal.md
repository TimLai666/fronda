## Why

UI parity audit row 2：媒體庫是 PARTIAL→STUB——搜尋框是靜態標籤、無資料夾（tiles/breadcrumb/新增/改名）、無 View/Sort/Filter 選單、無多選、無 banners 與計數。Swift MediaTab 是編輯器的核心工作面，這些缺口讓媒體管理實質不可用。

## What Changes

- 搜尋框改用 text_field::TextField：即時過濾 grid（名稱比對），清除 X；moment/transcript 搜尋（search_core 既有索引）在索引可用時列 moment 結果
- 資料夾系統 UI：folder tiles（MediaManifestEntry.folder_id 既有欄位 + list_folders 工具既有）、雙擊進入、breadcrumb 返回、New Folder（create_folder 工具若存在否則 executor API）、資料夾改名（rename_media 既有）
- View 選單（Folders/Flat/Grouped）、Sort 選單（名稱/日期/類型）、Filter 選單（類型/AI/清除）——狀態存 media panel state
- 多選（cmd/ctrl-click + shift-click 範圍；marquee 後續）與批次操作（刪除）
- item-count 列與 index-status 顯示（search_core 索引狀態既有 API）
- 空狀態接線（media_empty_state 目前 dead code）

## Non-Goals

- 拖放（drag-drop-system change）
- 右鍵選單（context-menu-system change）
- 縮圖尺寸 presets 與 swap/toast banners（polish 後續批）

## Capabilities

### New Capabilities

- `media-library-ui`: 可用的媒體庫——即時搜尋、資料夾導覽、檢視/排序/過濾、多選、狀態列

### Modified Capabilities

(none)

## Impact

- Affected specs: media-library-ui（新增）
- Affected code:
  - New: (none)
  - Modified: crates/app_shell_gpui/src/media_panel_view.rs, crates/app_shell_gpui/src/editor_state_hub.rs
  - Removed: (none)

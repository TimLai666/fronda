## Why

UI parity audit row 6：外部與跨面板拖放整層缺失——無法從檔案總管拖檔案進媒體庫、無法把資產拖上 timeline 或生成參考位。gpui 拖放目前只有 timeline 內部的 clip 移動/修剪。這是剪輯器的基本互動語言。

## What Changes

- 檔案總管 → 媒體面板：gpui external file drop（調查 gpui-ce 的 external drag API：ExternalPaths/on_drop——以實際 API 為準）→ 匯入既有 import 流程；拖曳懸停高亮
- 媒體資產 → timeline：資產 tile 可拖（gpui on_drag 帶 asset id payload），timeline 軌道為 drop target（放置即 add_clips 於指標 frame 位置）；拖曳中顯示插入指示
- 媒體資產 → 生成參考位：generation panel 的 ref tiles 接受資產 drop（generation-panel-functional 的 click-to-pick 之外的第二路徑）
- 共用 drag payload 型別（既有 drag_payload 模組擴充）與 drop 高亮樣式（AppTheme）

## Non-Goals

- timeline 內部 clip 拖曳（已存在）
- 拖出 app（export drag）
- chat 面板的媒體 drop mention（後續）

## Capabilities

### New Capabilities

- `drag-drop`: 跨面板與外部拖放——檔案匯入、資產上 timeline、資產進生成參考

### Modified Capabilities

(none)

## Impact

- Affected specs: drag-drop（新增）
- Affected code:
  - New: (none)
  - Modified: crates/app_shell_gpui/src/media_panel_view.rs, crates/app_shell_gpui/src/timeline_view.rs, crates/app_shell_gpui/src/generation_view.rs, crates/app_shell_gpui/src/drag_payload.rs
  - Removed: (none)

## Why

UI parity audit row 7：Rust 全 app 沒有任何右鍵選單。Swift 在專案卡（Open/Reveal/Remove from Recents/Delete）、媒體資產與資料夾 tiles 都有 context menus。缺這層讓桌面應用的基本操作語言斷裂。

## What Changes

- 可重用 context menu 元件（gpui：右鍵觸發、絕對定位 popover、項目 hover、點外/Esc 關閉、分隔線、危險項紅色）——調查 gpui-ce 是否有內建 menu 基元，無則以現有 popover 模式自建
- 專案卡選單：Open、Reveal in File Manager（PlatformAdapter 既有 reveal 介面）、Remove from Recents（registry 既有）、Delete Project（確認對話）
- 媒體資產選單：Rename（inline TextField）、Delete、Reveal、（AI 標籤重跑等後續）
- 資料夾選單：Rename、Delete（內容移回上層）

## Non-Goals

- timeline clip 右鍵選單（Swift 也沒有——audit 確認）
- 選單鍵盤導航（方向鍵）第一版不做

## Capabilities

### New Capabilities

- `context-menus`: 右鍵選單系統與專案卡/媒體資產/資料夾三個接入點

### Modified Capabilities

(none)

## Impact

- Affected specs: context-menus（新增）
- Affected code:
  - New: crates/app_shell_gpui/src/context_menu.rs
  - Modified: crates/app_shell_gpui/src/app_root.rs, crates/app_shell_gpui/src/media_panel_view.rs
  - Removed: (none)

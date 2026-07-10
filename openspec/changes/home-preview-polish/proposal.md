## Why

UI parity audit rows 10/12/14/15/16 的中小型缺口合批：Home 專案卡缺 hover 態/刪除鈕/檔案遺失覆蓋、樣本專案硬編碼、Open Project 無檔案面板；Preview 缺 aspect/fps/resolution/zoom 選單與 Capture Frame；tour spotlight 沒挖洞；welcome 簡化版；工具列缺 Add-Text 鈕。

## What Changes

- Home：專案卡 hover（縮放/陰影/邊框 AppTheme）、hover 垃圾桶鈕 + 刪除確認、file-missing 覆蓋（路徑存在性檢查）；Open Project 接 cx.prompt_for_paths（export 既有模式）；樣本 strip 標注為資料待接（SampleProjectService 網路 gated——顯示區塊保留但註記）
- Preview：Aspect-Ratio/Frame-Rate/Resolution/Zoom 選單接 timeline_core::project_presets（資料既有）+ set_project_settings 工具；Capture Frame 鈕（preview_render 既有 compositor 輸出 → 存 PNG 進 media/ + 註冊資產，matte writer 模式）
- Tour：spotlight 挖洞（目標元素 bounds 的 overlay 遮罩 + 高亮框——查 tour anchors 的座標來源可行性，不可行則記錄阻擋）
- Welcome 對照 Swift 補齊；Toolbar Add-Text 鈕（serif T → add_texts 工具於 playhead）

## Non-Goals

- SampleProjectService 網路下載（backend gated）
- IdentityStrip（帳號系統 gated）

## Capabilities

### New Capabilities

- `home-preview-polish`: Home 卡片互動、Preview 設定選單與擷取幀、tour spotlight、Add-Text

### Modified Capabilities

(none)

## Impact

- Affected specs: home-preview-polish（新增）
- Affected code:
  - New: (none)
  - Modified: crates/app_shell_gpui/src/app_root.rs, crates/app_shell_gpui/src/preview_view.rs, crates/app_shell_gpui/src/tour_overlay_view.rs, crates/app_shell_gpui/src/toolbar_view.rs
  - Removed: (none)

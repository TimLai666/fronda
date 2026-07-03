## Why

timeline 畫面已渲染共享狀態，但完全不可操作：無法選取剪輯、無法拖曳移動、播放頭固定、Undo/Redo/Delete/Split 選單全是 no-op。所有底層能力都已存在——agent_contract 的 move_clips/remove_clips/split_clip/undo/redo 工具（undo-tracked、遞增 revision）、timeline_core 的 snapping 純函式、revision 驅動重繪的管道——缺的是 UI 互動層把手勢轉成工具呼叫。

## What Changes

- timeline_model 增加互動狀態與純邏輯：selected_clip_ids、剪輯拖曳 session（起點、指標 frame、以 timeline_core snapping 求 snap 目標）、播放頭 scrub 換算；全部可純測試
- TimelineView 接上手勢：點 ruler／拖曳 ruler 移動播放頭；點剪輯選取（高亮）；水平拖曳剪輯，放開時透過共享 executor 呼叫 move_clips（同軌、snap 後 frame），revision 機制自動重繪並保留 undo
- AppRoot 選單接線：Undo/Redo 透過共享 executor 執行 undo/redo 工具；Delete 刪除選取剪輯（remove_clips）；SplitAtPlayhead 對選取剪輯在播放頭處 split_clip
- 拖曳期間顯示既有 snap_x_frame 黃色對齊線

## Non-Goals

- 跨軌拖曳（v1 僅同軌水平移動）、剪輯 trim 手把、rubber-band 多選、複製貼上
- 播放（transport）與預覽同步
- TrimStartToPlayhead/TrimEndToPlayhead/SelectAll 選單（維持 no-op，後續 change）
- 拖放媒體到 timeline

## Capabilities

### New Capabilities

- `timeline-interactions`: timeline 的播放頭 scrub、剪輯選取、同軌拖曳移動（含 snap），以及 Undo/Redo/Delete/Split 選單經共享 executor 生效

### Modified Capabilities

(none)

## Impact

- Affected specs: 新增 `timeline-interactions`
- Affected code:
  - Modified:
    - crates/app_shell_gpui/src/timeline_model.rs
    - crates/app_shell_gpui/src/timeline_view.rs
    - crates/app_shell_gpui/src/app_root.rs
    - crates/app_shell_gpui/Cargo.toml
  - New: (none)
  - Removed: (none)

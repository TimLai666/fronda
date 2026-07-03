## Why

transport 快捷鍵（Space、J/K/L、逐格、跳格）在 menu 已定義但全是 no-op，播放頭只能用滑鼠拖。Swift 基準的 transport 由 VideoEngine（AVFoundation）驅動；Rust 版尚無解碼子系統，但「時間推進」本身不依賴解碼：播放頭以計時器按 fps 與倍率前進即可成立，這也是 preview 未來接上渲染時的時間來源。

## What Changes

- timeline_model 新增 TransportState 純邏輯：playing/rate、Space 切換播放暫停、J/K/L（J 反向播放且重按倍增至 -8x、K 暫停、L 正向播放重按倍增至 8x）、tick(dt) 依 fps 與 rate 推進播放頭並夾在 [0, total_frames]（撞界即停）、逐格 ±1、跳格 ±5（對齊 Swift skipForward 預設）
- TimelineView 驅動：播放中以約 30Hz 計時器 tick 並重繪；transport 方法供選單呼叫
- AppRoot 接線：PlayPause/PlayBackward/PauseJkl/PlayForward/StepFrameBackward/StepFrameForward/SkipFramesBackward/SkipFramesForward 轉呼叫 timeline_view
- 播放推進是 view 狀態，不產生 undo、不觸發 mutation

## Non-Goals

- 影音實際播放與 preview 畫面渲染（需要解碼子系統，見 spec 記載的架構邊界）
- MarkIn/MarkOut 標記（獨立功能面）
- 播放時的自動捲動跟隨

## Capabilities

### New Capabilities

- `transport-controls`: 鍵盤 transport（播放暫停、JKL 倍率、逐格、跳格）以計時器推進播放頭

### Modified Capabilities

(none)

## Impact

- Affected specs: 新增 `transport-controls`
- Affected code:
  - Modified:
    - crates/app_shell_gpui/src/timeline_model.rs
    - crates/app_shell_gpui/src/timeline_view.rs
    - crates/app_shell_gpui/src/app_root.rs
  - New: (none)
  - Removed: (none)

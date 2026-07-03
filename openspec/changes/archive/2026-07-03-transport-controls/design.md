## Context

menu.rs 已定義 transport 快捷鍵（Space、J/K/L、左右鍵逐格、Shift+左右跳格）並路由到 app_root 的空分支。Swift 基準：play/pause 為布林、skipForward 預設 5 frames、實際播放由 AVFoundation VideoEngine 驅動（Rust 無對應解碼子系統）。TimelineState 已有 playhead_frame/total_frames/fps 與 revision 重繪管道；gpui 提供 background executor timer 與 cx.spawn。

## Goals / Non-Goals

**Goals:**

- Space/J/K/L/逐格/跳格全部生效，播放頭以正確速率推進並在邊界停止
- transport 邏輯純函式化（TransportState），時間推進可用假 dt 單元測試
- 播放不產生 undo、不碰共享 executor

**Non-Goals:**

- 影音解碼與 preview 渲染、MarkIn/Out、播放自動捲動

## Decisions

### TransportState 純邏輯（timeline_model）

TransportState { rate: f64 }（rate 0 = 暫停）。方法：toggle_play()（0↔1.0）、jkl_forward()（rate<=0 → 1.0，否則 ×2 上限 8.0）、jkl_backward()（rate>=0 → -1.0，否則 ×2 下限 -8.0）、jkl_pause()（rate=0）、is_playing()。TimelineState 增加 transport 欄位與 transport_tick(dt_seconds) -> bool：以 rate × fps × dt 累積小數 frame（浮點累積器避免低速率下取整永遠為 0），推進 playhead 並夾在 [0, total_frames]，撞界時 rate 歸零；回傳是否有變化。step_frames(delta)：暫停並把 playhead 夾界移動（逐格 ±1、跳格 ±5 由呼叫端給值）。

### TimelineView 計時器驅動

transport 指令方法（transport_toggle_play/jkl_* /step）改變 TransportState 後，若進入播放且 ticker 未跑，cx.spawn 一個迴圈：await background timer 33ms → WeakEntity::update 呼叫 transport_tick 並 cx.notify()，tick 回傳 false（已暫停或 view 已卸載）即結束迴圈。以 ticker_running: bool 防重複 spawn。

### AppRoot 轉呼叫

八個 transport 分支各自 timeline_view.update 呼叫對應方法；timeline_view 不存在（Home 畫面）時無動作。

## Implementation Contract

- 行為：按 Space 播放頭開始以 1x 前進（fps 30 時約每秒 30 frames）、再按停止；L 連按依序 1x/2x/4x/8x，J 為負向同規則，K 停止；播放到 total_frames 或倒帶到 0 自動停止；左右鍵逐格 ±1、Shift+左右 ±5，皆夾在 [0, total_frames]。播放推進不改變共享 executor 的 revision。
- 介面／資料形狀：TransportState 與方法如 Decisions；TimelineState::transport_tick(dt: f64) -> bool、step_frames(delta: i64)；TimelineView::transport_toggle_play(cx)、transport_jkl(direction, cx)、transport_step(delta, cx)。
- 失敗模式：view 卸載時 ticker 迴圈自然結束；dt 異常大（系統休眠）時單次推進仍夾界。
- 驗收標準：
  - timeline_model 新測試：toggle 播放暫停、JKL 倍增與上限（1→2→4→8→8、負向對稱）、tick 以 dt=1.0 rate=1 fps=30 前進 30 frames、低速率小 dt 的小數累積（rate 1、fps 30、dt 0.02 連續 5 次 → 前進 3 frames）、撞 0 與撞 total_frames 停止、step 夾界
  - cargo test --workspace、clippy -D warnings、desktop-app check 全過；app smoke
  - 手動抽查：跑 app 按 Space 播放頭移動（人工項）
- 範圍界線：in scope＝上述 transport；out of scope＝Non-Goals 全部。

## Risks / Trade-offs

- [33ms 計時器與 fps 不整除造成推進抖動] → 浮點累積器保證平均速率正確；顯示層本來就以 frame 取整
- [無解碼的「播放」只有播放頭移動] → 記入 spec 為明確架構邊界：transport 是時間來源，畫面渲染屬解碼子系統

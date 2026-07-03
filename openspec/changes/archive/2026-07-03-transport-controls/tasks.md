## 1. TransportState 純邏輯

- [x] 1.1 實作需求「Keyboard transport drives the playhead」與「Frame stepping and skipping」的純邏輯，依 design 決策「TransportState 純邏輯（timeline_model）」：TransportState（toggle_play/jkl_forward/jkl_backward/jkl_pause/is_playing）、TimelineState::transport_tick(dt) 浮點累積推進與夾界停止、step_frames(delta) 暫停並夾界移動。驗證：新增測試覆蓋 spec 的 Tick advancement 例表三列、JKL 倍增上限（1→2→4→8→8 與負向）、Space 語意（0↔1）、撞界歸零、step/skip 夾界

## 2. view 與選單接線

- [x] 2.1 依 design 決策「TimelineView 計時器驅動」與「AppRoot 轉呼叫」：TimelineView 增 transport_toggle_play/transport_jkl/transport_step 與 33ms ticker（cx.spawn 迴圈、ticker_running 防重複、tick 回 false 即停）；app_root 八個 transport 分支接線。驗證：cargo check -p fronda-app-shell-gpui --features desktop-app --bin fronda 通過；手動抽查 Space 播放（人工項記錄於完成報告）

## 3. 全面驗證

- [x] 3.1 cargo test --workspace、cargo clippy --workspace --tests -- -D warnings 全過；app smoke（啟動加 MCP initialize）。驗證：指令輸出審閱

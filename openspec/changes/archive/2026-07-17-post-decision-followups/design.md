## Decisions

(1) settings AI/Agent pane 加「Whisper model」路徑欄位（TextField＋Browse 免做——純文字路徑即可；空→移除鍵）；讀寫沿 pane_prefs 的 read/write_prefs_root。(2) `ToolExecutor` 內部 pub 方法 `switch_multicam_segment`（包 timeline_core::switch_segment＋undo snapshot＋revision），不註冊為工具；Multicam tab 依 clip 分類（program/mic/overlay）顯示對應 chips（Swift context menu 的語意搬到 tab）。選取清除：照 Swift handlePanelClick（media focus 清 clip selection 等——讀 Swift 逐條移植）。ring 淡入：gpui 有 animation API 就 0.2s easeOut，沒有就記錄。(3) clippy：redundant closure/float precision/sort_by_key/map_or/strip-prefix/assign-op 等機械項，逐檔 clean 後測試不變。

## Implementation Contract

- 各線測試綠；(2) 的 switch e2e 過 executor（mic 段改角後 timeline 符合 engine 測試預期）；(3) clippy 警告數下降且零行為變更。
- 兩相全綠、desktop check。

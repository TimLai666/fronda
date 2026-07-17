## Decisions

focused pane：AppRoot 欄位 `focused_pane: Option<PaneId>`；各 pane 卡片 on_mouse_down capture 設定（不干擾既有 listener）；pane_card 接受 focused 旗標→ accent 邊框（Swift PanelFocusRing 樣式：讀 in-tree Swift 取 accent 色/寬）。maximize 的 pane 自動 focused。sizes：pane_prefs 新 `paneSizes {agent,media,inspector,timelineHeight}`（f32，缺鍵用預設）；divider drag 結束（on_mouse_up 或每次 clamp 後 debounce——選實作簡單者）與 toggle 後儲存；載入在 AppRoot::new 套用並過 clamp（視窗小於存值時 preview-min 保護生效）。

## Implementation Contract

- pane_prefs round-trip 測試（sizes+visibility 並存、他鍵保留）；focused 狀態純測試；載入 clamp 測試。
- `cargo test -p fronda-app-shell-gpui` 兩相全綠、desktop check。

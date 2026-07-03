## 1. model 對應

- [x] 1.1 實作需求「Media panel state maps from the shared manifest」，依 design 決策「media_panel_model 增加 MediaItem 與 items_from_manifest 純對應」：新增 MediaItem 與 MediaPanelState::sync_from_manifest（重建 items 與 folders、保留分頁狀態）。驗證：新增測試——兩 entries 一 folder 對應數量與名稱、重複呼叫冪等、先有舊清單再 sync 會被替換

## 2. view 綁定

- [x] 2.1 實作需求「Media panel renders the shared manifest」與「Tile hue is stable per media id」，依 design 決策「MediaPanelView 以 revision 監看重建」：view 增加 state_revision 監看、格線由 items 驅動（tile_icon 依 ClipType、tile_hue 依 id 穩定雜湊）、示範 tile 退出 runtime 路徑。驗證：新增 tile_hue 穩定性與範圍測試；cargo check -p fronda-app-shell-gpui --features desktop-app --bin fronda 通過

## 3. 全面驗證

- [x] 3.1 cargo test --workspace、cargo clippy --workspace --tests -- -D warnings 全過；手動驗證：跑 app 以 MCP create_folder／import_media 後 revision 遞增（畫面重建路徑由單元測試佐證）。驗證：指令輸出審閱

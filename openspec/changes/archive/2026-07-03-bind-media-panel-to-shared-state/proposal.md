## Why

timeline 畫面已渲染共享狀態，但 media panel 的素材格仍是寫死的六張示範 tile（media_demo_grid），與實際載入專案的 MediaManifest 完全無關：開啟專案或 MCP 匯入媒體後，面板內容不變。共享狀態、revision 變更訊號、from_core 對應模式都已就緒，media panel 是同一模式的直接延伸。

## What Changes

- media_panel_model 增加媒體項目模型與純對應函式：MediaItem（id、name、clip type）與 MediaPanelState::items_from_manifest(&MediaManifest)，依 entry type 決定顯示圖示、依 id 雜湊出穩定色相
- MediaPanelView 改為渲染共享 manifest 的真實 entries：以 hub revision 偵測變更重建項目清單（沿用 timeline/agent panel 的監看模式），空 manifest 顯示空格線；示範 tile 退場為測試用途
- 資料夾列表同步渲染 manifest.folders（僅顯示，不含資料夾導覽互動）

## Non-Goals

- 媒體匯入 UI（檔案對話框、拖放進面板）
- 縮圖產生（真實影格縮圖需要解碼管線；tile 維持色塊＋圖示）
- 資料夾導覽、搜尋、排序等互動
- Photos/Generated 分頁內容（維持現狀）

## Capabilities

### New Capabilities

- `media-panel-binding`: media panel 渲染共享 manifest 的真實媒體項目與資料夾，並隨 revision 更新

### Modified Capabilities

(none)

## Impact

- Affected specs: 新增 `media-panel-binding`
- Affected code:
  - Modified:
    - crates/app_shell_gpui/src/media_panel_model.rs
    - crates/app_shell_gpui/src/media_panel_view.rs
  - New: (none)
  - Removed: (none)

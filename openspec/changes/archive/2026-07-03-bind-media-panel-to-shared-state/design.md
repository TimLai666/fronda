## Context

MediaPanelState 目前只有分頁狀態；素材格由 media_panel_view 的 media_demo_grid() 寫死六張 demo_tile。共享狀態側 MediaManifest 有 entries（id、name、type: ClipType、source、duration）與 folders（id、name、parent_folder_id）。timeline_view 與 agent_panel_view 已建立「render 時比對 hub revision、變更即重建資料」的模式。

## Goals / Non-Goals

**Goals:**

- media panel Library 分頁渲染真實 manifest entries 與 folders，MCP 匯入／改名／刪除後畫面跟著更新
- 對應邏輯是純函式、可單元測試
- 空專案顯示空格線而非假資料

**Non-Goals:**

- 匯入 UI、真實縮圖、資料夾導覽互動、Photos/Generated 分頁

## Decisions

### media_panel_model 增加 MediaItem 與 items_from_manifest 純對應

新增 MediaItem { id, name, kind: core_model::ClipType }，MediaPanelState 增加 items: Vec<MediaItem> 與 folders: Vec<(String, String)>（id、name）。純函式 sync_from_manifest(&mut self, &MediaManifest) 重建兩個清單，保留分頁等 view 狀態。圖示與色相由 view 層從 MediaItem 導出：圖示依 ClipType（Video ▶、Audio ♪、Image ⬜、Text T），色相以 id 位元組和 mod 100 除以 100 取得穩定值（同一素材每次渲染同色）。替代方案是把圖示字元存進 model，但那是呈現細節、留在 view。

### MediaPanelView 以 revision 監看重建

MediaPanelView 增加 state_revision: u64，render 開頭比對 EditorStateHub::global().revision()，變更時鎖 executor 取 media_manifest 呼叫 sync_from_manifest 並 cx.notify()。media_demo_grid 與 demo tile 資料退出 runtime 路徑（格線改由 items 驅動；items 為空時渲染空捲動區）。

## Implementation Contract

- 行為：載入含 entries 的專案後，Library 分頁顯示每個 entry 一張 tile（名稱＝entry.name、圖示依 type）；MCP 呼叫 rename_media 後下一次 render 顯示新名稱；空專案顯示無 tile 的空格線。
- 介面／資料形狀：MediaItem { id: String, name: String, kind: ClipType }；MediaPanelState::sync_from_manifest(&mut self, &MediaManifest)；view 層 fn tile_icon(kind: &ClipType) -> &'static str、fn tile_hue(id: &str) -> f32（0.0..1.0）。
- 失敗模式：executor 鎖失敗時保留上次清單不 panic；未知 ClipType 變體（未來擴充）落到 Video 圖示。
- 驗收標準：
  - media_panel_model 新測試：sync_from_manifest 對應 entries 與 folders、重複呼叫冪等、清單被替換而非累加；tile_hue 對同 id 穩定且落在 0.0..1.0
  - cargo check -p fronda-app-shell-gpui --features desktop-app --bin fronda 通過；cargo test --workspace 與 clippy -D warnings 全過
  - 手動驗證：跑 app，MCP import_media 或 create_folder 後 media panel 於下一次重繪顯示新項目（以 revision 機制與單元測試佐證）
- 範圍界線：in scope＝model 對應、view 綁定與 revision 監看；out of scope＝匯入 UI、縮圖、互動、其他分頁。

## Risks / Trade-offs

- [entries 很多時每次 revision 全量重建清單] → 與 timeline 相同的量級假設，先全量、不夠再增量
- [demo tile 移除改變 scaffold 視覺] → 空專案本來就該是空的；Swift 版空專案的 Library 也是空格線

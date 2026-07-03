## Why

三個殘留 scaffold：ImportMedia 選單是 no-op（媒體只能靠 MCP 匯入）；home 畫面的 recent projects 是寫死示範卡片，開啟過的專案不會出現；media panel 與專案卡片的縮圖全是色塊，即使圖片素材與專案 thumbnail.png 都能直接顯示。所需基礎全部在樹內：import_media 工具、ClipType::from_extension、core_model::ProjectRegistry（可序列化）、gpui 的 img 元素直接支援檔案路徑，零新依賴。

## What Changes

- 媒體匯入：ImportMedia 選單開多選檔案對話框，每個檔案以 import_media 工具匯入共享 manifest（名稱取檔名、type 由 ClipType::from_extension 判定、不可辨識的副檔名略過並記 log）；media panel 隨 revision 自動出現新項目
- recent projects：ProjectRegistry 持久化於 Fronda 設定目錄的 projects.json；load_bundle 與 save_as 成功時登錄／更新該專案；home 畫面以 registry 的 sorted_entries 渲染真實專案卡（名稱、相對時間），點擊卡片走 open_project_at；寫死示範卡移除
- 縮圖：媒體格中 Image 類型的素材以 gpui img 直接顯示來源檔（External 絕對路徑或 Project 相對路徑經 project_root 解析）；專案卡顯示 bundle 內的 thumbnail.png（存在時）；影片與音訊素材維持型別色塊——影片首格縮圖需要解碼子系統（Swift 端為 AVFoundation），以 spec 明文記為架構邊界而非缺口

## Non-Goals

- 影片影格解碼（無解碼子系統；引入 ffmpeg 級依賴屬獨立的架構與套件選型決策）
- 匯入時複製檔案進 bundle 的 media/ 目錄（import_media 現行契約為引用來源路徑）
- 媒體時長探測（無解碼；沿用 import_media 工具預設）
- registry 的改名、移除、釘選等管理 UI

## Capabilities

### New Capabilities

- `media-import-ui`: 以檔案對話框把本機媒體匯入共享 manifest
- `recent-projects`: 持久化的最近專案清單驅動 home 畫面並可點擊開啟
- `real-thumbnails`: 圖片素材與專案卡以實際檔案渲染縮圖；影片影格縮圖記為解碼子系統邊界

### Modified Capabilities

(none)

## Impact

- Affected specs: 新增 `media-import-ui`、`recent-projects`、`real-thumbnails`
- Affected code:
  - Modified:
    - crates/app_shell_gpui/src/app_root.rs
    - crates/app_shell_gpui/src/editor_state_hub.rs
    - crates/app_shell_gpui/src/media_panel_model.rs
    - crates/app_shell_gpui/src/media_panel_view.rs
    - crates/app_shell_gpui/src/home_model.rs
    - crates/app_shell_gpui/src/home_view.rs
  - New:
    - crates/app_shell_gpui/src/project_registry_store.rs
  - Removed: (none)

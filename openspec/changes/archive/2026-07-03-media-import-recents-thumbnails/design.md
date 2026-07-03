## Context

import_media 工具接受 name/filePath/type/duration（duration 無解碼時走預設）。core_model::ProjectRegistry 可序列化，含 last_opened_date 與 sorted_entries()。home 畫面目前只渲染 New Project 卡，app_root 內有一組寫死的 RecentProject 示範資料。gpui img 元素接受 PathBuf 直接渲染圖檔。MediaManifestEntry 的 source 為 External{absolutePath} 或 Project{relativePath}（後者需 project_root 解析）。EditorStateHub 已知 project_root；mcp_service 已有平台設定目錄的路徑推導。

## Goals / Non-Goals

**Goals:**

- ImportMedia 對話框匯入本機檔案到共享 manifest，面板即時出現
- 開啟／另存過的專案持久化於 registry，home 以真實卡片呈現並可點開
- 圖片素材與專案 thumbnail.png 以實檔渲染縮圖，零新依賴

**Non-Goals:**

- 影片影格解碼（架構邊界，見 spec）、檔案複製進 bundle、時長探測、registry 管理 UI

## Decisions

### project_registry_store：registry 的檔案持久化

新增 project_registry_store 模組（無 gpui）：fronda_config_dir()（與 mcp_service 相同的平台推導，抽成此處的公用函式並讓 mcp_service 改用）、load() -> ProjectRegistry（檔案缺失或損毀回空 registry）、save(&ProjectRegistry)、record_opened(path)（load → register(now) → save）。檔案位置為設定目錄下 Fronda/projects.json，屬 Fronda 自有狀態、非相容契約檔。

### hub 在載入與另存成功時登錄 registry

load_bundle 與 save_as 的成功路徑呼叫 record_opened（失敗僅 eprintln，不影響主流程）。NewProject（未存檔）不登錄，save_as 後才有路徑可登錄。

### home 畫面渲染 registry 卡片

app_root 的 render_home 每次渲染呼叫 store::load().sorted_entries() 建卡（home 畫面渲染頻率低，直接讀檔可接受）；卡片顯示專案名（ProjectEntry::name()）、相對時間標籤（新增純函式 relative_time_label(then, now)：just now／N m ago／N h ago／N d ago，可測）；點擊卡片呼叫 open_project_at；寫死的 RecentProject 示範資料與欄位移除。卡片縮圖：entry 路徑下 thumbnail.png 存在時以 img 渲染，否則維持色塊。

### ImportMedia 對話框與工具落地

app_root 的 ImportMedia 分支開 prompt_for_paths（files: true, multiple: true），cx.spawn 等待後對每個路徑：ClipType::from_extension 判型（None 則 eprintln 略過），以 import_media 工具（name＝檔名、filePath＝絕對路徑、type）寫入共享 executor；revision 機制讓 media panel 自動更新。

### 圖片素材縮圖

MediaItem 增加 source_path: Option<PathBuf>；MediaPanelState::sync_from_manifest 增加 project_root 參數解析 Project 相對路徑（External 直接用絕對路徑），檔案不存在時為 None。view 的 tile：kind 為 Image 且 source_path 存在時以 img(path) 取代色塊（80x60、cover）；其餘型別維持色塊＋圖示。影片首格縮圖需要解碼子系統，於 spec 記為明確架構邊界。

## Implementation Contract

- 行為：ImportMedia 選檔後，選定的圖片／影片／音訊出現在 media panel（圖片 tile 顯示實際圖）且 MCP get_media 可見；開啟或另存專案後回到 home（或重啟 app）會看到該專案卡片，點擊即開啟；專案含 thumbnail.png 時卡片顯示它。不可辨識副檔名的檔案被略過且不中斷其他檔案匯入。
- 介面／資料形狀：project_registry_store::{fronda_config_dir, load, save, record_opened}；relative_time_label(then: DateTime<Utc>, now: DateTime<Utc>) -> String；MediaItem.source_path: Option<PathBuf>；MediaPanelState::sync_from_manifest(&MediaManifest, Option<&Path>)。
- 失敗模式：registry 檔損毀→空 registry 重建；登錄失敗不阻擋開啟；對話框取消無動作；來源檔缺失→tile 退回色塊。
- 驗收標準：
  - project_registry_store 測試：record_opened 兩個路徑後 load 的 sorted_entries 依 last_opened 排序、同路徑重複登錄不增項；損毀 JSON 回空 registry
  - relative_time_label 測試：分鐘／小時／天三檔與 just now
  - media_panel_model 測試：External 絕對路徑與 Project 相對路徑（給 project_root）的 source_path 解析、缺 root 的 Project 來源為 None
  - hub 測試：load_bundle 成功後 registry 含該路徑（以測試專用 store 路徑注入或環境隔離）
  - cargo test --workspace、clippy -D warnings、desktop-app check、app smoke 全過
- 範圍界線：in scope＝上述四塊；out of scope＝Non-Goals 全部。

## Risks / Trade-offs

- [home 每次渲染讀檔] → home 渲染頻率低；效能問題出現再快取
- [registry 全域檔案讓 hub 測試互相污染] → store 提供以路徑參數化的 load_from/save_to，global 便利函式只是預設路徑包裝；hub 測試注入 temp 路徑
- [import 引用原始路徑，檔案移動即離線] → 與 import_media 工具現行契約一致，離線呈現已有 RES 契約處理

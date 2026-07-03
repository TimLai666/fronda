## 1. registry 持久化

- [x] 1.1 實作需求「Opened projects persist in the recent-project registry」，依 design 決策「project_registry_store：registry 的檔案持久化」與「hub 在載入與另存成功時登錄 registry」：新增 project_registry_store（fronda_config_dir、load_from/save_to 與預設路徑包裝、record_opened_at）；mcp_service 的設定目錄推導改用共用函式；EditorStateHub 增 registry 路徑欄位（測試可注入），load_bundle/save_as 成功時登錄。驗證：新增測試——同路徑重複登錄不增項且更新 last_opened、損毀 JSON 回空 registry、hub load_bundle 後注入路徑的 registry 含該專案

## 2. home 畫面

- [x] 2.1 實作需求「Home screen lists recent projects」與「Project cards show the bundle thumbnail」，依 design 決策「home 畫面渲染 registry 卡片」：home_model 新增 relative_time_label 純函式；render_home 以 sorted_entries 建卡（名稱、相對時間、thumbnail.png 存在時 img 渲染）、點擊走 open_project_at；移除寫死的 RecentProject 示範資料。驗證：新增 relative_time_label 測試覆蓋 spec 例表四列；cargo check -p fronda-app-shell-gpui --features desktop-app --bin fronda 通過

## 3. 匯入與縮圖

- [x] 3.1 實作需求「Import Media dialog feeds the shared manifest」，依 design 決策「ImportMedia 對話框與工具落地」：app_root 的 ImportMedia 分支開多選檔案對話框，逐檔以 ClipType::from_extension 判型（None 略過並 eprintln）呼叫 import_media。驗證：新增 hub 整合測試——以 import_media 匯入 png 與 mp4 名稱後 get_media 可見且型別正確（對應 spec 的 Import an image and a video）；cargo check --features desktop-app 通過
- [x] 3.2 實作需求「Image media renders real thumbnails」，依 design 決策「圖片素材縮圖」：MediaItem 增 source_path，sync_from_manifest 增 project_root 參數解析 External/Project 來源（檔案不存在為 None）；media panel tile 對 Image 且有 source_path 者以 img 渲染，否則色塊。驗證：新增測試——External 絕對路徑（tempdir 實檔）解析成功、Project 相對路徑經 root 解析、缺 root 為 None、檔案不存在為 None；cargo check --features desktop-app 通過

## 4. 全面驗證

- [x] 4.1 cargo test --workspace、cargo clippy --workspace --tests -- -D warnings 全過；app smoke（啟動加 MCP initialize）。驗證：指令輸出審閱

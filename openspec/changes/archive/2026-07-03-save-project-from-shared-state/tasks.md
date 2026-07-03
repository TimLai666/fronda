## 1. project_io 窄儲存

- [x] 1.1 實作需求「Narrow save preserves unrelated package content」，依 design 決策「project_io 新增窄路徑 save_project_state」：新增 save_project_state(root, timeline, manifest)，只寫 project.json 與 media.json。驗證：新增 round-trip 測試——tempdir 預放 chat/session1.json，save_project_state 後 ProjectBundle::open 讀回 timeline fps 與 manifest 內容一致，且 chat/session1.json 內容原樣

## 2. hub 與選單接線

- [x] 2.1 實作需求「Shared state saves back to the open project」，依 design 決策「EditorStateHub::save 綁定目前專案根目錄」與「SaveProject 選單接線」：hub 增加 save()（無 root 回 Err；有 root 時鎖內 clone timeline/manifest、鎖外寫檔），app_root 的 SaveProject 分支呼叫 hub.save() 並以 eprintln 記錄失敗。驗證：新增測試——load_bundle → executor 執行 create_folder → save() → 重新 load_bundle 後 manifest 含 B-roll；未開專案 save() 回 Err；cargo check -p fronda-app-shell-gpui --features desktop-app --bin fronda 通過

## 3. 全面驗證

- [x] 3.1 cargo test --workspace、cargo clippy --workspace --tests -- -D warnings 全過。驗證：指令輸出審閱

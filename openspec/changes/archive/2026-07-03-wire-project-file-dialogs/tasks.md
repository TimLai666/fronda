## 1. hub save_as

- [x] 1.1 實作需求「Save As writes to a new root and switches the project root」的 hub 部分，依 design 決策「EditorStateHub 新增 save_as」：抽出鎖內 snapshot helper，新增 save_as(root)（寫到新目錄、成功才切換 project_root）。驗證：新增測試——未開專案時 save_as 到 tempdir 後該目錄有 project.json/media.json 且 project_root 指向它；隨後 executor create_folder 再 save()，新目錄的 media.json 含該資料夾（證明 save 目標已切換）

## 2. 對話框接線

- [x] 2.1 實作需求「Open Project shows a directory picker and loads the choice」與 SaveProjectAs 的對話框部分，依 design 決策「對話框以 cx.spawn 非同步接線」：OpenProject 分支開 prompt_for_paths（directories-only 單選）並於選定後呼叫 open_project_at；SaveProjectAs 分支開 prompt_for_new_path（起始於 project_root 或家目錄、建議名 Untitled.palmier）並呼叫 hub.save_as，Err 以 eprintln 記錄；取消一律無動作。驗證：cargo check -p fronda-app-shell-gpui --features desktop-app --bin fronda 通過；手動跑 app 按 Ctrl+O 出現對話框、取消後畫面不變（人工抽查項記錄於完成報告）

## 3. 全面驗證

- [x] 3.1 cargo test --workspace、cargo clippy --workspace --tests -- -D warnings 全過。驗證：指令輸出審閱

## 1. 載入管線

- [x] 1.1 實作需求「Project bundle loads into the shared editor state」，依 design 決策「EditorStateHub 增加 load_bundle 與專案根目錄記錄」：crates/app_shell_gpui/Cargo.toml 加 project_io 依賴，EditorStateHub 增加 load_bundle(path) -> Result<(), String> 與 project_root()，成功時載入 timeline/manifest（None 用 default）、記錄根目錄、revision 遞增，失敗回 Err 且狀態與 revision 不變。驗證：新增測試——tempdir 寫最小 project.json（fps 60）載入成功後 executor 的 fps 為 60、revision 遞增、project_root 記錄；載入不存在路徑回 Err 且 revision 不變

## 2. view 對應與渲染

- [x] 2.1 實作需求「Core timeline maps to the timeline view state」，依 design 決策「timeline_model 增加 core Timeline 到 TimelineState 的純對應函式」：新增 TimelineState::from_core(&core_model::Timeline, &MediaManifest)，依 spec 的 Mapping table 對應。驗證：新增測試逐列覆蓋 Mapping table 六列（track 種類兩列、clip 標籤兩列、total_frames 保底與取最大兩列）
- [x] 2.2 實作需求「Timeline view renders the shared state」，依 design 決策「TimelineView 以 revision 重建資料、保留 view 狀態」：TimelineView 增加 state_revision，render 時 revision 變更即以 from_core 重建 tracks/clips/fps/total_frames，保留 zoom_scale/scroll_x/scroll_y/playhead_frame，並 cx.notify()；with_default_tracks 退出 runtime 路徑。驗證：cargo check -p fronda-app-shell-gpui --features desktop-app --bin fronda 通過；跑 app 以 curl 呼叫 add_texts 或 create_folder 後確認 revision 遞增（timeline mutation 對應畫面資料重建路徑已由 from_core 測試覆蓋）

## 3. AppRoot 接線

- [x] 3.1 實作需求「New project resets the shared state before opening the editor」，依 design 決策「AppRoot 的 open_project_at 與 NewProject 經過 hub」：AppRoot 增加 open_project_at(path, cx)（load_bundle Ok 才 open_editor，Err 留原畫面並 eprintln），menu NewProject 分支改為先 hub.load_project(Timeline::default(), MediaManifest::default()) 再 open_editor。驗證：cargo check --features desktop-app 通過；手動跑 app 觸發 NewProject（Cmd/Ctrl+N）後 curl get_timeline 回傳空白 timeline

## 4. 全面驗證

- [x] 4.1 cargo test --workspace、cargo clippy --workspace --tests -- -D warnings、cargo fmt --all -- --check 全過；手動驗證：建含 project.json 的 .palmier 目錄，程式路徑呼叫 load_bundle 後 curl get_timeline 反映載入內容。驗證：指令輸出與 curl 回應內容審閱

## 1. adapter 與快取

- [x] 1.1 實作修改後需求「Image media renders real thumbnails」的 adapter 部分，依 design 決策「系統 ffmpeg 子行程 adapter（video_thumbnails 模組）」：新增 video_thumbnails 模組（ffmpeg_program、thumbnail_cache_dir、cache_path_for 以來源路徑加 mtime 雜湊、extract 快取命中短路且失敗一律 None）。驗證：單元測試——同來源同 mtime 鍵穩定、mtime 變動產新鍵、FRONDA_FFMPEG 指向不存在程式時 extract 回 None、預放快取檔後以壞 ffmpeg 路徑仍回 Some（命中不執行子行程）；整合測試——ffmpeg 存在時以 lavfi testsrc 產 1 秒影片並 extract 出非空 PNG，不存在時印 skip

## 2. 背景抽取與 tile 接圖

- [x] 2.1 依 design 決策「背景抽取與行程內結果表」：request_thumbnail 非阻塞（結果表、進行中集合、std::thread 抽取）；media panel 的 Video tile 有 source_path 時呼叫 request_thumbnail，Some 以 img 渲染、None 維持色塊。驗證：cargo check -p fronda-app-shell-gpui --features desktop-app --bin fronda 通過；手動抽查——匯入影片後 media panel 出現實際首格（人工項記錄於完成報告）

## 3. spec 邊界更新與全面驗證

- [x] 3.1 依 design 決策「spec 邊界更新」確認 delta spec 的 MODIFIED 需求已涵蓋 ffmpeg adapter 與退回行為（內容審閱）；cargo test --workspace、cargo clippy --workspace --tests -- -D warnings 全過；app smoke（啟動加 MCP initialize）。驗證：指令輸出審閱

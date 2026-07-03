## 1. 快取清理邏輯

- [x] 1.1 實作需求「Thumbnail cache evicts stale and excess entries」的純邏輯，依 design 決策「cache_path_for 拆出穩定前綴」「per-source 舊版本汰換（evict_stale_versions）」「啟動時總量上限修剪（prune_by_size）」：抽出 source_key／cache_prefix_for；新增 evict_stale_versions 與 prune_by_size 與 THUMBNAIL_CACHE_MAX_BYTES；extract 成功後呼叫 evict。驗證：新增測試——evict 移除同前綴舊版本且保留 kept、不碰不同前綴；prune 未超標回 0、超標依 mtime 由舊到新刪到低於上限（對應 spec 的 Prune order 例）、空目錄安全；cache_prefix_for 對同來源穩定

## 2. 啟動接線

- [x] 2.1 依 design 決策「啟動時總量上限修剪」：app_root open_main_window 以背景執行緒呼叫 prune_by_size(thumbnail_cache_dir(), THUMBNAIL_CACHE_MAX_BYTES)。驗證：cargo check -p fronda-app-shell-gpui --features desktop-app --bin fronda 通過

## 3. 全面驗證

- [x] 3.1 cargo test --workspace、cargo clippy --workspace --tests -- -D warnings 全過；app smoke（啟動加 MCP initialize）。驗證：指令輸出審閱

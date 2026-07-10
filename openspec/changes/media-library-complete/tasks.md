## 1. 狀態與純邏輯

- [x] 1.1 [P] MediaPanel 檢視狀態結構：search_query、view_mode、sort_key、type_filter、current_folder、selection: Vec<String>；純函式 visible_entries(manifest, state) -> Vec<條目>（過濾→資料夾範圍→排序），單元測試覆蓋每維度與組合 → `media_panel_view.rs` LibraryState/visible_entries（scope guard 限定檔案清單，純邏輯與測試同檔）；18 tests
- [x] 1.2 [P] 選取邏輯純函式：toggle（ctrl）、range（shift 以當前排序）、clear；測試 → select_click/select_toggle/select_range/clear_selection + anchor 語意；8 tests

## 2. UI

- [x] 2.1 搜尋框 TextField 接入（Edited → search_query → grid 過濾）+ 清除 X；moment 搜尋結果區（search_core 可用時）→ 名稱搜尋 + Files 區塊 + no-matches 完成；**moment/transcript 區塊 deferred**：Rust 端沒有 search index host（search_core lifecycle 未接入 app_shell），「索引可用時」條件不成立
- [x] 2.2 資料夾：folder tiles（folder_id 分組）、雙擊進入、breadcrumb、New Folder、改名 → 改名走既有 `rename_folder` 工具（**非 rename_media**——rename_media 只處理 media/timeline id）；New Folder 走 `create_folder` + 自動開啟 inline rename；rename 沿用 timeline tab TextField 模式（Enter 提交、Esc 取消、點外提交）
- [x] 2.3 View/Sort/Filter 選單（沿用既有 dropdown 樣式）→ View: Folders/Flat/Grouped；Sort: Name/Date Added/Duration/Type（鏡射 Swift SortMode 全集）；Filter: Video/Audio/Image + AI Generated + Clear
- [x] 2.4 多選視覺（選取框線）+ 批次刪除（remove media 既有工具）→ ctrl/cmd toggle、shift range（顯示排序）、Accent 框線；context bar「Delete (n)」逐一呼叫既有 `delete_media`；marquee 依提案為後續
- [x] 2.5 item-count + index-status 列；media_empty_state 接線（移除 dead_code allow）→ count 在 context bar，index status（executor `search_status()`，空字串隱藏）在 actions row；空狀態文案對齊 Swift（"No media yet"）

## 3. 驗證

- [x] 3.1 純邏輯測試 + 三 gate exit code 全綠 → 26 library tests；`cargo test --workspace` EXIT=0、`cargo test -p fronda-app-shell-gpui --features desktop-app` EXIT=0（280 passed）、`cargo check --bin fronda` EXIT=0（2026-07-10）
- [x] 3.2 對抗審查一輪；98-ui-parity-audit.md row 2 狀態更新 → 審查修正：escape 分層取消、rename 點外提交、rename 中禁止雙擊開資料夾、移除 min_w magic number

## 1. 狀態與純邏輯

- [ ] 1.1 [P] MediaPanel 檢視狀態結構：search_query、view_mode、sort_key、type_filter、current_folder、selection: Vec<String>；純函式 visible_entries(manifest, state) -> Vec<條目>（過濾→資料夾範圍→排序），單元測試覆蓋每維度與組合
- [ ] 1.2 [P] 選取邏輯純函式：toggle（ctrl）、range（shift 以當前排序）、clear；測試

## 2. UI

- [ ] 2.1 搜尋框 TextField 接入（Edited → search_query → grid 過濾）+ 清除 X；moment 搜尋結果區（search_core 可用時）
- [ ] 2.2 資料夾：folder tiles（folder_id 分組）、雙擊進入、breadcrumb、New Folder、改名（既有 rename_media/inline TextField 模式沿用 timeline tab rename）
- [ ] 2.3 View/Sort/Filter 選單（沿用既有 dropdown 樣式）
- [ ] 2.4 多選視覺（選取框線）+ 批次刪除（remove media 既有工具）
- [ ] 2.5 item-count + index-status 列；media_empty_state 接線（移除 dead_code allow）

## 3. 驗證

- [ ] 3.1 純邏輯測試 + 三 gate exit code 全綠
- [ ] 3.2 對抗審查一輪；98-ui-parity-audit.md row 2 狀態更新

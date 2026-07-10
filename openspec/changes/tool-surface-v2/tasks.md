## 1. 契約抄錄（先行，read-only）

- [x] 1.1 從 upstream/main@141c69b 抄錄完整 v2 契約到本 change 的 design.md：48 工具清單與每個的精確 input schema（ToolDefinitions.swift 全文對照）、mutation envelope 的確切 JSON 形狀（含哪些工具回它）、get_timeline v2 輸出（[start,end) frames、A/V 折疊規則、caption-group 摘要、gaps 表示）、get_transcript v2、新 SYSTEM_INSTRUCTION 全文；標注與 Rust 現面的逐工具 diff（新增/更名/吸收/退場/參數變更）與 Rust-native 擴展保留清單 — 完成：48 上游（45 本 change + 3 multicam 留位）+ 8 Rust 擴展 = 最終面 53；Rust 64 = 40 SAME + 1 RENAMED(sync_audio→sync_clips) + 15 ABSORBED + 8 KEPT；set_blend_mode/duplicate_timeline 實為上游吸收（修正 proposal 假設）

## 2. 新工具

- [x] 2.1 organize_media（path 定位資料夾；對照 Swift 語意含建立中間層級）——取代 create_folder/rename_folder/delete_folder/move_to_folder/rename_media/delete_media 的退場計畫一併在 design 標注 — 完成：`organize.rs` 純 path 解析（case-insensitive、exact-case 優先、ambiguity 錯誤、中間層級建立）+ `cmd_organize_media`（parse-before-mutate、cycle guard、last-timeline guard、folder cascade、clipsRemoved、nest warning、active-switch notes）；envelope/short-id 依 task 4 再套
- [x] 2.2 manage_tracks（reorder/mute/hide/syncLock/remove 多動作）— up-front index→id 解析、zone-clamped reorder、idempotent set、remove 保留他軌 linked partners、tracks 新順序輸出（label V 底往上 / A 上往下）+ notes；exec_mut undo 包覆
- [x] 2.3 close_project（ProjectNavigator seam 擴充）— trait 增 `close(name,id,path,ActiveProjectState)`→`ClosedProject`；save-first（失敗留開）、must-be-open、next-active adopt / no-project reset（清 project seams、換 rootless lister）；AppProjectNavigator 實作 + 測試

## 3. 吸收合併

- [x] 3.1 import_media 吸收 create_matte（source.matte）與 import_folder（source.path 目錄遞迴、鏡射子資料夾）；create_timeline 吸收 duplicate_timeline（`from` 參數，v2 payload {timelineId,name,active,note}）；被吸收工具退場。url/bytes 誠實回報 host 服務未接（follow-up）
- [x] 3.2 organize_media 六工具退場；UI 呼叫點全數遷移：media_panel_view（批次刪除、New Folder 唯一名+path、folder/asset rename、folder delete cascade）、timeline_view（tab rename/close）、media_import + preview_view（import_media source.path）、editor_state_hub 測試、mcp server 測試。工具數 64+3−10=57（4 檔斷言同步）

## 4. Envelopes 與讀取面

- [x] 4.1 mutation envelope 依抄錄格式套用到全部 clip 工具回傳
- [x] 4.2 get_timeline v2 重構（relationship-first）；get_transcript v2

## 5. Instructions 與收尾

- [x] 5.1 SYSTEM_INSTRUCTION 重寫：上游 v2 全文 + Rust 擴展工具段
- [x] 5.2 工具數斷言（4 檔）、snapshot、MCP 契約測試全面更新至最終數（48 + 擴展數，design 定案）
- [ ] 5.3 三 gate exit code 全綠；對抗審查一輪；AGENTS.md 工具面記載更新

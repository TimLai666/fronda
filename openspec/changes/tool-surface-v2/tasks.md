## 1. 契約抄錄（先行，read-only）

- [ ] 1.1 從 upstream/main@141c69b 抄錄完整 v2 契約到本 change 的 design.md：48 工具清單與每個的精確 input schema（ToolDefinitions.swift 全文對照）、mutation envelope 的確切 JSON 形狀（含哪些工具回它）、get_timeline v2 輸出（[start,end) frames、A/V 折疊規則、caption-group 摘要、gaps 表示）、get_transcript v2、新 SYSTEM_INSTRUCTION 全文；標注與 Rust 現面的逐工具 diff（新增/更名/吸收/退場/參數變更）與 Rust-native 擴展保留清單

## 2. 新工具

- [ ] 2.1 organize_media（path 定位資料夾；對照 Swift 語意含建立中間層級）——取代 create_folder/rename_folder/delete_folder/move_to_folder/rename_media/delete_media 的退場計畫一併在 design 標注
- [ ] 2.2 manage_tracks（reorder/mute/hide/syncLock/remove 多動作）
- [ ] 2.3 close_project（ProjectNavigator seam 擴充）

## 3. 吸收合併

- [ ] 3.1 import_media 吸收 create_matte（matte 參數路徑）；create_timeline 吸收 duplicate_timeline（sourceTimelineId 參數）；被吸收工具退場
- [ ] 3.2 organize_media 六工具退場；media_panel_view 等 UI 呼叫點遷移到新工具

## 4. Envelopes 與讀取面

- [ ] 4.1 mutation envelope 依抄錄格式套用到全部 clip 工具回傳
- [ ] 4.2 get_timeline v2 重構（relationship-first）；get_transcript v2

## 5. Instructions 與收尾

- [ ] 5.1 SYSTEM_INSTRUCTION 重寫：上游 v2 全文 + Rust 擴展工具段
- [ ] 5.2 工具數斷言（4 檔）、snapshot、MCP 契約測試全面更新至最終數（48 + 擴展數，design 定案）
- [ ] 5.3 三 gate exit code 全綠；對抗審查一輪；AGENTS.md 工具面記載更新

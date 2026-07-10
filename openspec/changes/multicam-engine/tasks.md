## 1. 資料模型

- [x] 1.1 MulticamSource 型別化（serde 鍵照 upstream #283 diff：git show 上游對應 Swift 檔逐欄）；ProjectFile.multicam_groups 從 Value 轉 Vec<MulticamSource>（帶容錯 alias 讓既有 opaque fixture round-trip 不破）；Clip.multicam_group_id 由 inert 轉讀寫

## 2. 引擎

- [x] 2.1 timeline_core::multicam：MulticamEngine 純邏輯移植（上游 MulticamEngine.swift + MulticamEngineTests 570 行逐案為準——TDD 直接移植其測試案例）：群組建立、sync offset 套用、change_cam 的 clip source 替換數學（保 trim/keyframes/timing）

## 3. 工具

- [x] 3.1 manage_multicam/change_cam/get_multicam 依 tool-surface-v2 design.md 保留位契約 + envelope/short-id 整合；validators wired；工具數 53→56 四檔斷言 + host split 更新
- [x] 3.2 move_clips 群組移動；manage_tracks multicam guard 真實化

## 4. 收尾

- [x] 4.1 三 gate exit code 全綠；對抗審查一輪；AGENTS.md/97-audit #283 row 更新

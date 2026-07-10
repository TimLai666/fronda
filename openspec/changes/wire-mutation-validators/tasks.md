## 1. 決策與接線

- [ ] 1.1 盤點 mutation.rs 全部 validators 與 executor 行內檢查的重疊/缺口矩陣（工具 × 規則），記錄於本 change；依矩陣確認方案 A（統一接線）可行性——validator 輸入型別與 executor 的 args 解析相容性
- [ ] 1.2 execute() dispatch 前接 tool→validator 映射（單行 match 或表驅動）；validator Err → 工具 Err 原文回傳；行內重複檢查刪除（保留 validator 沒有的執行期檢查如資產存在性）
- [ ] 1.3 e2e 測試：volume 1.5 / opacity -0.1 / speed 0 / trim 負值 / frame 超 ceiling 各一，經 executor.execute 拒絕；既有全部工具測試不得回歸

## 2. 收尾

- [ ] 2.1 三 gate exit code 全綠；AGENTS.md porting table #144 行更新為真 live 狀態

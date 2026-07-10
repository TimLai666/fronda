## 1. 已驗證 bug（TDD）

- [ ] 1.1 [P] #124 stranded audio：重現測試（linked V+A pair 被 overwrite-place 蓋掉 → audio 軌殘片 + 新 clip audio 排錯軌）→ 修 overwrite 路徑同步清 partner 軌範圍（對照 git show upstream PR #124 diff 的 Swift 修法語意）
- [ ] 1.2 [P] #263 ripple fixpoint：重現測試（anchor 軌 ripple 清 sync-locked 軌，被清 clips 的 partner 在 lock-off 軌 → 殘留 desync）→ compute_ripple_delete 的 partner 傳播改 fixpoint 掃全部 cleared 軌；既有 ripple 測試（含 200k fuzz 若在）不得回歸

## 2. UNVERIFIED 查證（測試先行，有 bug 才修）

- [ ] 2.1 [P] #139：render_core format_timecode 測 frame 1800 @ 29.97 drop-frame 應 00;01;00;02（與 10 分鐘邊界 17982 除數）；錯則修
- [ ] 2.2 [P] #264：mutation/tool args 以接近 i64::MAX 的 frame 值打 add/insert/move/split → 溢位 panic 或 wrap 則加共用 clamp（上游用 1e9 ceiling）
- [ ] 2.3 [P] #212：set_clip_properties speed 0.1 的接受度；下限過緊則放寬並測慢速 duration 換算

## 3. S 級 port

- [ ] 3.1 [P] #36：AnthropicConfig 支援自訂 base URL（ANTHROPIC_BASE_URL env 覆蓋，預設不變）；transport 測試
- [ ] 3.2 [P] #268：request builder 對 sonnet5 系模型加 output_config effort low（對照 git show upstream/main 對應 Swift 請求組裝檔的確切 JSON 形狀）；snapshot 測試

## 4. 收尾

- [ ] 4.1 三 gate exit code 全綠；97-audit 附錄各項標注結論

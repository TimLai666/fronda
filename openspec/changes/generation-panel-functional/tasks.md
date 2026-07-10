## 1. 資料與狀態

- [x] 1.1 [P] GenerationState 擴充：selected_model_id（依 type 預設 catalog 首個 plan-available）、per-type 參數（duration/aspect/resolution/quality/count/instrumental/gen_audio/voice）、references: Vec<String>（asset ids）、estimated_cost；模型切換時參數 caps 校正（無效值回 default）——純邏輯函式 + 單元測試
- [x] 1.2 [P] 成本估算純函式：generation_core 的價格資料 × 參數（video: 時長×解析度價格表；image: count×單價；audio: 模型價）——對照 Swift GenerationView 的 costEstimate 邏輯（git show upstream/main 對應檔），測試釘住數值
  - 來源記錄：upstream/main 的 CostEstimator 是 credits 制（server catalog）；本 repo 的 catalog 是 Fal 期 USD 價，故以 `9dfde8d^:Sources/PalmierPro/Generation/Fal/CostEstimator.swift`（USD 版，同結構同查找優先序）為準，實作在 `model_catalog::{video_cost,image_cost,audio_cost,format_usd}`

## 2. UI 佈線

- [x] 2.1 模型選單：讀 catalog() 過濾 selected_type；paid gating 標示（upgrade 徽章）；選取更新 state
- [x] 2.2 gear popover：caps 驅動控制項（既有 dropdown/segmented 模式沿用 preview 的 menu 樣式）；Esc/點外關閉
- [x] 2.3 成本列 + insufficient-credits 態（Generate disable + 訊息）
- [x] 2.4 參考 tiles：點擊開媒體資產選擇（列出 manifest 影像/影片資產的簡單 picker popover）、縮圖（video_thumbnails 既有 cache）、清除 X、cap 上限；audio 模式無 tiles
- [x] 2.5 voice picker（audio caps.voices）+ lyrics/style 欄位（TextField/TextArea 既有元件）
- [x] 2.6 Generate 提交：組 GenerationInput → executor 的 generate 路徑（cmd_generate_* 已存在——UI 呼叫 run_shared_tool 或直接 executor API，依 chat 面板先例選擇並記錄）；無 backend 顯示 unavailable；is_generating 綁真實狀態
  - 決策記錄：採 timeline_view 的共享 executor 直呼先例（同步 `exec.execute(tool, &args)`，但讀回結果而非 fire-and-forget）。`build_generation_input` 先組完整 GenerationInput（含 reference asset ids），`generation_tool_call` 從它導出 tool args（多餘參數今日 stub 忽略、留給未來 backend）。stub 回 `isError` + "requires a remote API" → `SubmitOutcome::Unavailable` → 面板顯示明確不可用訊息、不清空表單；`is_generating` 由 manifest 的 in-flight `generationStatus`（preparing/generating/downloading）導出，無面板自有假旗標。注意：stub 呼叫仍會登記一筆 placeholder manifest entry（與 agent chat 走的同一 tool 語意一致，可 undo/delete）

## 3. 驗證

- [x] 3.1 純邏輯測試（caps 校正、成本表、references cap）；三 gate exit code 全綠
- [x] 3.2 對抗審查一輪；與 Swift GenerationView 對照的視覺結構差異記錄到 98-ui-parity-audit.md（更新 row 1 狀態）

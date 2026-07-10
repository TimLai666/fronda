## 1. 目錄與資料

- [ ] 1.1 [P] 轉錄上游 upscale 目錄：git show 9dfde8d^ 下的 Fal upscale 模型檔（git ls-tree 找 Upscale/Topaz 相關 Swift 檔）→ generation_core::model_catalog ModelKind::Upscale 條目（id/display/倍率/價格照抄）；snapshot 測試釘住

## 2. 佈線

- [ ] 2.1 AiEditTabView 增 selected_media_asset_id（pub 欄位）；inspector_view 在切至 AiEdit tab 或 selection 變化時轉傳（既有 observer 機制內加一行）；無選中 → action rows disabled 樣式與 no-op
- [ ] 2.2 action_row 增 on_click 參數（Option 回呼）；Upscale → upscale_media（assetId + 所選模型）、Music → generate_music、Sound Effects → generate_audio；結果/unavailable → status 欄位顯示（generation_view SubmitOutcome 解析模式沿用）
- [ ] 2.3 Rerun：選中資產 generation_input 存在 → 依 kind 組 generate_* args 重放；無 → disabled。Edit/Create Video → 明確「需要生成後端」status（誠實，不假裝）

## 3. 驗證

- [ ] 3.1 純邏輯測試（rerun args 重建、disabled 判定、unavailable 解析）+ 三 gate exit code 全綠
- [ ] 3.2 98-ui-parity-audit.md row 11 更新（含殘餘）

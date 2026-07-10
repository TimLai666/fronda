## 1. Captions

- [ ] 1.1 [P] Caption 狀態載體：對照 search_core::CaptionConfig 既有欄位，補 UI 需要的（font/size/color/background/case/censor/position）——缺的加在 app 層 view state 或擴充 CaptionConfig（若擴充需保持 serde 相容），純測試
- [ ] 1.2 控制列 UI：選單與 scrub 欄（沿用 inspector 的 scrub row 模式）、Font picker（render_core::text 字族清單來源查證）
- [ ] 1.3 預覽框：樣本 caption 以既有 caption 背景/字體渲染近似（gpui 端簡化渲染即可，不必逐像素同 exporter）、中心導線、X/Y scrub 綁 position
- [ ] 1.4 Agent Mode 選單 + Generate gating（words 可用性 = executor timeline_words 或 TranscriptionProvider 存在性；無則註記）

## 2. Music

- [ ] 2.1 [P] Music view state（mode/model_id/duration/prompt）+ catalog music 條目過濾純函式與測試
- [ ] 2.2 UI：mode/model 選單、duration scrub、prompt TextField、來源範圍摘要（timeline in/out 或全長）
- [ ] 2.3 成本註記 + credit/backend gating + generating overlay（綁真實狀態）

## 3. 驗證

- [ ] 3.1 純邏輯測試 + 三 gate exit code 全綠
- [ ] 3.2 對抗審查一輪；98-ui-parity-audit.md rows 3/4 更新

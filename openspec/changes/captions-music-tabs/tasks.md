## 1. Captions

- [x] 1.1 [P] Caption 狀態載體：對照 search_core::CaptionConfig 既有欄位，補 UI 需要的（font/size/color/background/case/censor/position）——缺的加在 app 層 view state 或擴充 CaptionConfig（若擴充需保持 serde 相容），純測試（CaptionConfig 未擴充：censor/locale/case 既有欄位已足，style/position 依 Swift CaptionRequest 的拆分放 app 層 `CaptionsState`；`CaptionCase.config_value()` 對齊 serde 值）
- [x] 1.2 控制列 UI：選單與 scrub 欄（沿用 inspector 的 scrub row 模式）、Font picker（查證結果：render_core::text 無公開字族清單——`font_for` 為私有 match；`BUNDLED_FONT_FAMILIES` 鏡射其 6 個內嵌字族，僅列 bundled、不列系統字型以免匯出時默默 fallback）
- [x] 1.3 預覽框：樣本 caption 以既有 caption 背景/字體渲染近似（1080 參考畫布縮放、canvas aspect 外框）、中心導線（x/y==0.5 時顯示）、X/Y scrub 綁 position + 中心吸附
- [x] 1.4 Agent Mode 選單 + Generate gating（Generate 走 add_captions tool；無 TranscriptionProvider → tool 回 unavailable → 明確註記、不轉 overlay；Agent handoff 走 `set_agent_chat_handoff` seam，未接線時註記）

## 2. Music

- [x] 2.1 [P] Music view state（mode/model_id/duration/prompt）+ catalog music 條目過濾純函式與測試（`music_models` 過濾 AudioCategory::Music；Rust catalog 無 `inputs` 欄位 → text mode 對 music 模型恆可用，已註記）
- [x] 2.2 UI：mode/model 選單、duration scrub、prompt TextArea（2..5 行、IME）、來源範圍摘要（Rust 尚無 in/out 標記 → 全長 "Whole timeline · 0:00 – m:ss · Xs"）
- [x] 2.3 成本註記 + credit/backend gating + generating overlay（cost = model_catalog::audio_cost；credits 綁 GenerationState.credits_remaining；overlay 只在 Queued 且 manifest 有 in-flight 條目時亮）

## 3. 驗證

- [x] 3.1 純邏輯測試 + 三 gate exit code 全綠（media_panel_view 49 tests、cargo check --bin fronda、cargo test --workspace 均 exit 0）
- [x] 3.2 對抗審查一輪；98-ui-parity-audit.md rows 3/4 更新

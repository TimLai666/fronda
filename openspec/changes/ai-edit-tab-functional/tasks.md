## 1. 目錄與資料

- [x] 1.1 [P] 轉錄上游 upscale 目錄：git show 9dfde8d^ 下的 Fal upscale 模型檔（git ls-tree 找 Upscale/Topaz 相關 Swift 檔）→ generation_core::model_catalog ModelKind::Upscale 條目（id/display/倍率/價格照抄）；snapshot 測試釘住
  - 來源：`Sources/PalmierPro/Generation/Fal/UpscaleModelConfig.swift` @ 9dfde8d^（5 模型：bytedance/seedvr/topaz video + seedvr/topaz image）。以獨立 `UpscaleModelConfig` 型別轉錄（Swift 本來就是獨立型別；新增 ModelCaps variant 會打破 tool_exec.rs 的 exhaustive match，該檔由平行 stream 持有）。上游無「倍率」欄位（倍率藏在 buildFalInput payload，屬後端 plumbing，依本模組既有規則不轉錄）；實際欄位 id/displayName/speed/endpoint/pricePerSecond/p75DurationSeconds/supportedTypes 全數照抄，`upscale_cost` 對齊 CostEstimator.upscaleCost。snapshot 測試 `upscale_catalog_snapshot_field_for_field` 釘住全部欄位

## 2. 佈線

- [x] 2.1 AiEditTabView 增 selected_media_asset_id（pub 欄位）；inspector_view 在切至 AiEdit tab 或 selection 變化時轉傳（既有 observer 機制內加一行）；無選中 → action rows disabled 樣式與 no-op
  - 轉傳放在 InspectorView::render 的 guarded sync（沿用 text-tab entity sync 既有模式，只在變化時 notify，並清 status/picker）——app_root observer 已把 selection 寫進 inspector，render 涵蓋「切 tab」與「selection 變化」兩種時機且不動 app_root
- [x] 2.2 action_row 增 on_click 參數（Option 回呼）；Upscale → upscale_media（assetId + 所選模型）、Music → generate_music、Sound Effects → generate_audio；結果/unavailable → status 欄位顯示（generation_view SubmitOutcome 解析模式沿用）
  - action_row 收必要 on_click（所有列都有 handler，disabled 時不掛）。Upscale 由 picker 列直接觸發（Swift Menu 語意）並持久化 selected_upscale_model；picker 列顯示 displayName + speed · cost（Swift menu label）。Music/SFX 帶 referenceVideoAssetIds + duration（上游 videoAudioSeed 語意），不帶 model（Fal-era 目錄無 video-to-audio 條目，帶了會被 resolve_generation_model 拒絕）
- [x] 2.3 Rerun：選中資產 generation_input 存在 → 依 kind 組 generate_* args 重放；無 → disabled。Edit/Create Video → 明確「需要生成後端」status（誠實，不假裝）
  - rerun_tool_call：upscale 模型 id → upscale_media（Swift rerun 分支順序）；目錄模型 → caps 決定 generate_video/image/music/audio；目錄外模型 → 依 manifest entry kind fallback 並保留 model arg 讓 executor 誠實回報。Create Video 依 Swift 改為 image 資產才顯示（原 is_video 顯示是 parity bug）

## 3. 驗證

- [x] 3.1 純邏輯測試（rerun args 重建、disabled 判定、unavailable 解析）+ 三 gate exit code 全綠
  - generation_core：+3 tests（snapshot/filter/cost）＝189 passed。app_shell_gpui（--features desktop-app --lib）：+11 tests（gating、upscale/music/sfx args、rerun 分支、action_status 四路、backend-required 文案）
- [x] 3.2 98-ui-parity-audit.md row 11 更新（含殘餘）

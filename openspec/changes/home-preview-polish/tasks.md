## 1. Home

- [ ] 1.1 [P] 專案卡 hover 態 + hover 垃圾桶 + 刪除確認（context-menu-system 的確認模式若已 landed 則共用）+ file-missing 覆蓋（Path::exists 檢查於 render snapshot）
- [ ] 1.2 [P] Open Project → cx.prompt_for_paths（export_view/app_root 既有 prompt 模式）→ open_project 路徑

## 2. Preview

- [ ] 2.1 選單 ×4 接 timeline_core::project_presets + set_project_settings 工具（active 選項標示既有邏輯）
- [ ] 2.2 Capture Frame：preview_render 的 compose → PNG 寫入 media/（ProjectMatteWriter 的檔案寫入模式）→ manifest 註冊 + revision bump

## 3. Tour / Welcome / Toolbar

- [ ] 3.1 tour spotlight：查 anchors 座標來源（tour_overlay_view 現況）；可行則 overlay 遮罩挖洞 + 高亮框，不可行記錄阻擋原因於 tasks 附註
- [ ] 3.2 [P] Welcome 對照 Swift WelcomeOverlay.swift 補齊結構差異
- [ ] 3.3 [P] Toolbar Add-Text 鈕 → add_texts 工具（playhead frame、預設樣式）

## 4. 驗證

- [ ] 4.1 純邏輯測試 + 三 gate exit code 全綠
- [ ] 4.2 對抗審查一輪；98-ui-parity-audit.md rows 10/12/14/15/16 更新

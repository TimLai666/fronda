## 1. Home

- [x] 1.1 [P] 專案卡 hover 態 + hover 垃圾桶 + 刪除確認（context-menu-system 的確認模式若已 landed 則共用）+ file-missing 覆蓋（Path::exists 檢查於 render snapshot）
  - hover：邊框亮化（white@Opacity::MUTED）+ shadow_lg + hover 垃圾桶鈕；scale 效果 gpui hover style 不支援 transform，略（pixel-level 差異）
  - 垃圾桶採 arm-then-confirm（同 context_menu destructive_confirm 模式）：第一擊 arm 成 "Confirm Delete"、第二擊刪除；hover 離開自動 disarm。Swift 用 alert dialog——本 repo 無 modal alert 元件，沿用既有確認模式
  - file-missing：卡片 dim（opacity STRONG）+ 黑幕 "?"+"File missing" 覆蓋、點擊開啟被擋；右鍵選單對 missing 卡隱藏 Open/Reveal（Swift isAccessible 同款）
- [x] 1.2 [P] Open Project → cx.prompt_for_paths（export_view/app_root 既有 prompt 模式）→ open_project 路徑（sidebar 鈕改走 perform_menu_action(MenuAction::OpenProject)，與選單/快捷鍵同一路徑）

## 2. Preview

- [x] 2.1 選單 ×4 接 timeline_core::project_presets + set_project_settings 工具（active 選項標示既有邏輯）
  - Aspect/Frame Rate/Quality → set_project_settings（aspectRatio/fps/quality），Zoom → view-local canvas_zoom（Swift editor.canvasZoom 同款，1.0=Fit、±0.01 active 判定）；zoom 以 relative-size wrapper 套在 canvas 上（fit×zoom、超出裁切）
  - `settings_menu_rows` 純函式 + 6 個測試；badge 標籤 live（Fit badge 改為 zoom_badge_label）
- [x] 2.2 Capture Frame：preview_render 的 compose → PNG 寫入 media/（ProjectMatteWriter 的檔案寫入模式）→ manifest 註冊 + revision bump
  - 註冊走既有 import_media 工具（成功即 bump revision）；名稱 "Frame {frame}"（Swift 同款）、image duration 5s（Swift Defaults.imageDurationSeconds）
  - 限制：import_media 註冊為 External 絕對路徑（檔案仍在 media/ 內）；Project-relative 註冊需要 agent_contract 新 seam（本 change 凍結該 crate）。未存檔專案（無 package root）寫入 temp fronda-captures 資料夾。僅 Timeline tab 可截（Swift 另支援 video asset tab——Rust 尚無 per-asset player）

## 3. Tour / Welcome / Toolbar

- [x] 3.1 tour spotlight：查 anchors 座標來源（tour_overlay_view 現況）；可行則 overlay 遮罩挖洞 + 高亮框，不可行記錄阻擋原因於 tasks 附註
  - 調查結果：遮罩挖洞 + 高亮框已具備（spotlight_scrim，本次補 clamp + 純測試），但 **anchor bounds 無現成來源**。gpui 技術上可取（timeline_view 的 zero-size canvas prepaint 模式可回報元素 window bounds），但 Swift 的 anchor 目標（media panel 的 import/generate 鈕、generation panel、editor 各 pane、timeline ruler）全部位於本 change 凍結的檔案（media_panel_view.rs、editor_view.rs、timeline_view.rs），需要一個跨 view 的 anchor-bounds registry seam——留待獨立 change；在那之前 spotlight 步驟以置中 callout + 全幅 scrim 呈現
  - 本次落地：TourFlow 純步驟機（鏡射 Swift TourController.makeSteps 12 步、gated smart-search 除外）、Skip/Back/Next/Start-creating 全部接線、scrim 點擊僅 spotlight 步驟結束（Swift 同款）、預設 idle（不再開機即蓋版）、Welcome "Watch Tutorial" 為入口；5 個純測試
  - Outro 的 link rows（MCP Setup/Shortcuts/Docs/Settings）未做——目標視窗尚未全部存在，不做 inert 假列
- [x] 3.2 [P] Welcome 對照 Swift WelcomeOverlay.swift 補齊結構差異（520pt leading-aligned 卡片、title+subtitle 文案對齊、240px hero 區（漸層 fallback，無 bundled jpg 資產）、Skip / Watch Tutorial / Get started 膠囊鈕列；Watch Tutorial 開編輯器 + 啟動 tour——Swift 是下載 sample project 後啟動，SampleProjectService 網路 gated）
- [x] 3.3 [P] Toolbar Add-Text 鈕 → add_texts 工具（playhead frame、預設樣式）
  - ToolbarEvent::AddText → app_root 訂閱 → add_texts {content:"Text", startFrame:playhead, durationFrames:3s×fps}（Swift Defaults.textDurationSeconds）；serif 字體 gpui 無 generic family 解析，保留 bold "T"（pixel-level 差異）
  - 注意：工具語意為「找既有 text/video 軌或建軌」，與 Swift addTextClip 每次插最上方新軌不同——沿用工具契約

## 4. 驗證

- [x] 4.1 純邏輯測試 + 三 gate exit code 全綠 → 15 個本 change 測試（preview 6、tour 5、app_root 4）；`cargo test --workspace` EXIT=0、`cargo test -p fronda-app-shell-gpui --features desktop-app` EXIT=0（338 passed）、`cargo check -p fronda-app-shell-gpui --features desktop-app --bin fronda` EXIT=0（2026-07-10）
- [x] 4.2 對抗審查一輪；98-ui-parity-audit.md rows 10/12/14/15/16 更新

## 1. 資料安全與匯出

- [ ] 1.1 [P] #211 autosave：hub revision 變更後 debounce（Swift 間隔查上游）自動 save_project_state_with_siblings（有 root 時）；close_project/顯示 Home 前必存；無 root（未儲存專案）跳過；測試存檔觸發邏輯（注入時鐘或計數器）
- [ ] 1.2 [P] #138 HDR：Mp4Encoder HEVC Main10 路徑（pix_fmt yuv420p10le、colour primaries/transfer BT.2020/HLG 標記；libx265 缺時誠實回報）；ExportOptions + export UI 的 HDR 選項；小型 encode-decode 結構測試

## 2. 工具與渲染

- [ ] 2.1 [P] #176 duplicate_clips：上游契約（git show 其 PR diff/branch），完整保真（keyframes/effects/fades/links 重建 link group）、envelope/short-id、validators、工具數 56→57 四檔 + host split
- [x] 2.2 [P] #45 arrow/line：PORTED。查證結果：上游 main 沒有形狀光柵器（shape annotation 是 Rust-native #46），`Endpoints`/`Arrowhead` 目前無任何寫入端（add_shapes 工具尚未解析 endpoints）。座標空間**假設記錄**：端點採 shape bounding box 的正規化 0..1（start=(0,0) 左上、end=(1,1) 右下），與既有 rect/oval 的正規化慣例一致；無 endpoints 時預設水平置中線/箭頭。compositor `rasterize_line_or_arrow`（shaft = 到線段距離 ≤ 線寬/2；Arrow 於 end 加兩支箭羽，長度隨線寬）。3 黃金測試（水平預設線、對角線依端點、箭頭於尖端擴散且 Line 無擴散）。
- [x] 2.3 [P] #65 wght：PORTED。ab_glyph 0.2.32 **支援**變數字型軸（`VariableFont::set_variation(b"wght", v)`，`variable-fonts` 預設開啟；底層 ttf-parser 0.25）。已在 `render_text` 對 font 套 wght 軸（static 面回傳 false 為 no-op，仍走 font_for 的 Regular/Bold 檔）。同時把已打包的變數字族（Inter/Geist/GeistMono/DMSans/Caveat/PlayfairDisplay/SpaceGrotesk）接進 `font_for`（否則軸不可達＝假實作；Swift `BundledFonts` 也把全部字族列為可選）。測試：兩個皆 <600（同檔）的 wght 100 vs 590 於 Inter 產生不同筆畫覆蓋，證明是軸而非檔案切換。

## 3. UI 與雜項

- [x] 3.1 [P] #169 viewer guides：PORTED。基礎已在（`preview_guides.rs` #167：ViewerGuideState/7 種 guide/safe-zone/format-bar 數學，`viewer_guide_overlay` canvas 繪製）；本次補完缺的下拉選單 UI：`guide_menu_rows` 純函式 + Guides 按鈕 on_click `toggle_guide_menu` + `toggle_guide`（多選、mouse_down_out 關閉）+ 沿用 settings dropdown 樣式的選單面板（勾選反映 guide_state；與 settings 選單互斥不重疊）。View-local state（不寫入專案）。純函式測試。
- [x] 3.2 [P] #67：PORTED。專案卡右鍵選單加 Duplicate（accessible-gated——遺失的 package 不能複製）；`duplicate_project_at` 呼叫 `duplicate_project_package`（`project_io::project_duplicate::plan_duplicate` + 遞迴 `copy_dir_all` → "<name> (Copy).palmier"）→ `record_opened_at` 註冊到 recents → `home_cards_loaded_at = None` 刷新首頁卡片。duplicate_project **工具本身 host-gated**（無 fs），故 host 端執行 plan。**協調備註**：hub 目前**沒有** `save_now()`（autosave #211 屬資料 slice 之 1.1），故未在 show_home/close 接存檔——待資料 slice 落地後接（已記錄）。測試：菜單項存在/gated + fs 複製助手（暫存 .palmier 樹）。已知 follow-up：重複點 Duplicate 會覆寫同名 (Copy) package（plan 未做唯一化）。
- [ ] 3.3 [P] #284：aspect 標籤 helper（"16:9 (Landscape)" 類，上游 aspectRatioDisplayLabel 照抄）用於 list_models 與生成面板
- [x] 3.4 [P] #164：PORTED（可實作者）。對照 Swift `ShortcutsPane` 全表。**已補（3 個，動作皆已有 handler，非死綁定）**：`[`→Trim Start（Q 別名）、`]`→Trim End（W 別名）、⇧⌫→Ripple Delete（Swift 正式鍵；Rust 原僅 ⌥⌫）。menu.rs route 表（44→47）+ global_shortcuts.rs `!input` 綁定（`[`/`]` 復用既有 action，新增 `RippleDeleteSelection` action + `shift-backspace` 綁定）+ app_root on_action → perform_menu_action(RippleDelete)。
  **缺口清單（Swift 有、Rust 仍缺——底層功能未移植，刻意不加死綁定）**：
  - `V` 選取工具 / `C` 剃刀工具 — Rust 編輯器尚無 tool-mode 概念（DEFERRED）
  - `A` 選取本軌後續 / `⇧A` 選取全軌後續 — 尚無 select-forward 概念（DEFERRED）
  - `Esc` 取消選取並重置工具 — 尚無 deselect/tool-reset handler（現 Esc 僅關專案選單）（DEFERRED）
  **N/A（手勢非鍵盤綁定）**：⇧拖邊 Ripple Trim、⌘拖素材 Ripple Insert、⌥拖 Duplicate Clip、⇧拖尺規 Select Range、拖範圍邊 Adjust Range、⌥滾輪/Pinch/⌘滾輪 縮放捲動。
  **既有已符（無需補）**：Space/←/→/⇧←/⇧→、Q/W、Backspace、` `` `、I/O、所有 ⌘ 選單鍵（New/Open/Save/SaveAs/Import/Export/Undo/Redo/Cut/Copy/Paste/SelectAll/Split/FullScreen）。

## 4. 收尾

- [ ] 4.1 三 gate exit code 全綠；對抗審查一輪；AGENTS.md/97-audit 各項標注 PORTED

## 1. 資料安全與匯出

- [x] 1.1 [P] #211 autosave：`editor_state_hub.rs` `autosave_should_fire`（純決策，注入計數器可測）+ `autosave_if_dirty`（coalesce：有 root 且 revision 前進才存，rapid edits 併為一次；無 root 跳過）+ `save_now`（Home-transition/timer 具名 wrapper）+ `mark_saved_revision`（save/save_as/load_bundle 後基準）。對照 Swift `VideoProject.scheduleProjectCheckpointAutosave`（無時間間隔，coalesce 到下個 tick）。close_project 既有存檔不動；app_root timer/Home-transition 呼叫 `autosave_if_dirty()`/`save_now()` 是 1-line follow-up（app_root 屬別 slice，未改）。6 tests。
- [x] 1.2 [P] #138 HDR：`video_export.rs` 加 `VideoCodec::H265Hdr`（HEVC + `YUV420P10LE` + `set_colorspace(BT2020NCL)` + 直寫 `color_primaries=BT2020` / `color_trc=ARIB_STD_B67`(HLG)；libx265 缺→明確 error，NEVER SDR fallback）；`export_model.rs` 加 `hdr` 欄位 + `set_hdr` + `effective_video_codec`（H265+hdr→H265Hdr）；`export_view.rs` HDR toggle（H.265 才顯示）走 `effective_video_codec`；HDR 經 `VideoCodec` 沿 `audio_export::export_project_with_audio` 既有簽章直達（未改 audio_export）。結構測試 `hdr_export_is_10bit_bt2020_hlg_or_errors`（成功→re-decode 斷言 10-bit+BT.2020+HLG；libx265 缺→斷言明確 error 且不留檔）+ 2 model 測試。

## 2. 工具與渲染

- [x] 2.1 [P] #176 duplicate_clips：上游 PR #176 契約照抄（description verbatim）。`tool_exec.rs` `cmd_duplicate_clips`（單行 dispatch arm，走 `exec_enveloped`→C-4 mutation envelope）：完整保真 clone（fresh id、keyframes/effects/fades/speed/opacity/volume/transform/crop 全保留）、linked partner 自動複製（`partner_moves_for_move_of` 相對位移）、複製集內以 fresh link group 重連（≥2 成員共用 / 單一 unlink）、目的地 overwrite（`clear_region`）、multicam 脫離。`mutation.rs` `validate_duplicate_clips` wired 進 `validate_args`（entries 非空、toFrame≥0+frame ceiling、toTrack≥0）。short-id：`clipId` 已在 SCALAR_ID_KEYS 且 `expand_input_ids` 遞迴進 entries，nested 前綴展開 + 輸出 shorten 已涵蓋（測試確認，無需改 id_short）。工具數 56→57 四檔（tools.rs header+3 assert、spec_tool_snapshots、spec_mcp_contract、mcp server.rs）+ host split（shared 51→52 / mcp 55→56 / in-app 52→53）。四 Swift 測試移植 + short-id + validator，共 11 tests。
- [x] 2.2 [P] #45 arrow/line：PORTED。查證結果：上游 main 沒有形狀光柵器（shape annotation 是 Rust-native #46），`Endpoints`/`Arrowhead` 目前無任何寫入端（add_shapes 工具尚未解析 endpoints）。座標空間**假設記錄**：端點採 shape bounding box 的正規化 0..1（start=(0,0) 左上、end=(1,1) 右下），與既有 rect/oval 的正規化慣例一致；無 endpoints 時預設水平置中線/箭頭。compositor `rasterize_line_or_arrow`（shaft = 到線段距離 ≤ 線寬/2；Arrow 於 end 加兩支箭羽，長度隨線寬）。3 黃金測試（水平預設線、對角線依端點、箭頭於尖端擴散且 Line 無擴散）。
- [x] 2.3 [P] #65 wght：PORTED。ab_glyph 0.2.32 **支援**變數字型軸（`VariableFont::set_variation(b"wght", v)`，`variable-fonts` 預設開啟；底層 ttf-parser 0.25）。已在 `render_text` 對 font 套 wght 軸（static 面回傳 false 為 no-op，仍走 font_for 的 Regular/Bold 檔）。同時把已打包的變數字族（Inter/Geist/GeistMono/DMSans/Caveat/PlayfairDisplay/SpaceGrotesk）接進 `font_for`（否則軸不可達＝假實作；Swift `BundledFonts` 也把全部字族列為可選）。測試：兩個皆 <600（同檔）的 wght 100 vs 590 於 Inter 產生不同筆畫覆蓋，證明是軸而非檔案切換。

## 3. UI 與雜項

- [x] 3.1 [P] #169 viewer guides：PORTED。基礎已在（`preview_guides.rs` #167：ViewerGuideState/7 種 guide/safe-zone/format-bar 數學，`viewer_guide_overlay` canvas 繪製）；本次補完缺的下拉選單 UI：`guide_menu_rows` 純函式 + Guides 按鈕 on_click `toggle_guide_menu` + `toggle_guide`（多選、mouse_down_out 關閉）+ 沿用 settings dropdown 樣式的選單面板（勾選反映 guide_state；與 settings 選單互斥不重疊）。View-local state（不寫入專案）。純函式測試。
- [x] 3.2 [P] #67：PORTED。專案卡右鍵選單加 Duplicate（accessible-gated——遺失的 package 不能複製）；`duplicate_project_at` 呼叫 `duplicate_project_package`（`project_io::project_duplicate::plan_duplicate` + 遞迴 `copy_dir_all` → "<name> (Copy).palmier"）→ `record_opened_at` 註冊到 recents → `home_cards_loaded_at = None` 刷新首頁卡片。duplicate_project **工具本身 host-gated**（無 fs），故 host 端執行 plan。**協調備註**：hub 目前**沒有** `save_now()`（autosave #211 屬資料 slice 之 1.1），故未在 show_home/close 接存檔——待資料 slice 落地後接（已記錄）。測試：菜單項存在/gated + fs 複製助手（暫存 .palmier 樹）。已知 follow-up：重複點 Duplicate 會覆寫同名 (Copy) package（plan 未做唯一化）。
- [x] 3.3 [P] #284：`generation_core::model_catalog::aspect_ratio_display_label`（+ `aspect_ratio_display_token`）verbatim 照抄 Swift `ImageModelConfig.aspectRatioDisplayLabel`（colon-form 原樣、underscore enum → "Landscape 16:9"/"Square HD"）；golden 測試取自上游 PR #284 刪掉的 `ImageModelConfigTests`。`tool_exec.rs::cmd_list_models` 於 Video/Image entry 加 `aspectRatioLabels`（與 `aspectRatios` 平行，additive 相容）。生成面板 picker wiring 是 trivial follow-up（`generation_view.rs` 屬別 slice，未改）。3 tests（helper golden + list_models e2e + token 邊界含在 golden 內）。
- [x] 3.4 [P] #164：PORTED（可實作者）。對照 Swift `ShortcutsPane` 全表。**已補（3 個，動作皆已有 handler，非死綁定）**：`[`→Trim Start（Q 別名）、`]`→Trim End（W 別名）、⇧⌫→Ripple Delete（Swift 正式鍵；Rust 原僅 ⌥⌫）。menu.rs route 表（44→47）+ global_shortcuts.rs `!input` 綁定（`[`/`]` 復用既有 action，新增 `RippleDeleteSelection` action + `shift-backspace` 綁定）+ app_root on_action → perform_menu_action(RippleDelete)。
  **缺口清單（Swift 有、Rust 仍缺——底層功能未移植，刻意不加死綁定）**：
  - `V` 選取工具 / `C` 剃刀工具 — Rust 編輯器尚無 tool-mode 概念（DEFERRED）
  - `A` 選取本軌後續 / `⇧A` 選取全軌後續 — 尚無 select-forward 概念（DEFERRED）
  - `Esc` 取消選取並重置工具 — 尚無 deselect/tool-reset handler（現 Esc 僅關專案選單）（DEFERRED）
  **N/A（手勢非鍵盤綁定）**：⇧拖邊 Ripple Trim、⌘拖素材 Ripple Insert、⌥拖 Duplicate Clip、⇧拖尺規 Select Range、拖範圍邊 Adjust Range、⌥滾輪/Pinch/⌘滾輪 縮放捲動。
  **既有已符（無需補）**：Space/←/→/⇧←/⇧→、Q/W、Backspace、` `` `、I/O、所有 ⌘ 選單鍵（New/Open/Save/SaveAs/Import/Export/Undo/Redo/Cut/Copy/Paste/SelectAll/Split/FullScreen）。
## 4. 收尾

- [ ] 4.1 三 gate exit code 全綠；對抗審查一輪；AGENTS.md/97-audit 各項標注 PORTED

> Cross-slice follow-ups CLOSED (coordinator, 2026-07-11): #211 save_now()
> wired into app_root show_home + a 20s periodic autosave_if_dirty tick in
> open_editor; #284 aspect_ratio_display_label wired into the generation
> panel AspectRatio picker labels.

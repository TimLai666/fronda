## 1. 資料安全與匯出

- [x] 1.1 [P] #211 autosave：`editor_state_hub.rs` `autosave_should_fire`（純決策，注入計數器可測）+ `autosave_if_dirty`（coalesce：有 root 且 revision 前進才存，rapid edits 併為一次；無 root 跳過）+ `save_now`（Home-transition/timer 具名 wrapper）+ `mark_saved_revision`（save/save_as/load_bundle 後基準）。對照 Swift `VideoProject.scheduleProjectCheckpointAutosave`（無時間間隔，coalesce 到下個 tick）。close_project 既有存檔不動；app_root timer/Home-transition 呼叫 `autosave_if_dirty()`/`save_now()` 是 1-line follow-up（app_root 屬別 slice，未改）。6 tests。
- [x] 1.2 [P] #138 HDR：`video_export.rs` 加 `VideoCodec::H265Hdr`（HEVC + `YUV420P10LE` + `set_colorspace(BT2020NCL)` + 直寫 `color_primaries=BT2020` / `color_trc=ARIB_STD_B67`(HLG)；libx265 缺→明確 error，NEVER SDR fallback）；`export_model.rs` 加 `hdr` 欄位 + `set_hdr` + `effective_video_codec`（H265+hdr→H265Hdr）；`export_view.rs` HDR toggle（H.265 才顯示）走 `effective_video_codec`；HDR 經 `VideoCodec` 沿 `audio_export::export_project_with_audio` 既有簽章直達（未改 audio_export）。結構測試 `hdr_export_is_10bit_bt2020_hlg_or_errors`（成功→re-decode 斷言 10-bit+BT.2020+HLG；libx265 缺→斷言明確 error 且不留檔）+ 2 model 測試。

## 2. 工具與渲染

- [x] 2.1 [P] #176 duplicate_clips：上游 PR #176 契約照抄（description verbatim）。`tool_exec.rs` `cmd_duplicate_clips`（單行 dispatch arm，走 `exec_enveloped`→C-4 mutation envelope）：完整保真 clone（fresh id、keyframes/effects/fades/speed/opacity/volume/transform/crop 全保留）、linked partner 自動複製（`partner_moves_for_move_of` 相對位移）、複製集內以 fresh link group 重連（≥2 成員共用 / 單一 unlink）、目的地 overwrite（`clear_region`）、multicam 脫離。`mutation.rs` `validate_duplicate_clips` wired 進 `validate_args`（entries 非空、toFrame≥0+frame ceiling、toTrack≥0）。short-id：`clipId` 已在 SCALAR_ID_KEYS 且 `expand_input_ids` 遞迴進 entries，nested 前綴展開 + 輸出 shorten 已涵蓋（測試確認，無需改 id_short）。工具數 56→57 四檔（tools.rs header+3 assert、spec_tool_snapshots、spec_mcp_contract、mcp server.rs）+ host split（shared 51→52 / mcp 55→56 / in-app 52→53）。四 Swift 測試移植 + short-id + validator，共 11 tests。
- [ ] 2.2 [P] #45 arrow/line：先查 Swift 光柵化的端點座標空間（ShapeStyle/annotation 渲染碼），據此在 compositor rasterize_shape 補 arrow/line（線寬/箭頭幾何照 Swift）；黃金測試
- [ ] 2.3 [P] #65 wght：ab_glyph 變數字型軸支援查證；可行則 render_core::text 依 TextStyle.font_weight 套軸；不可行記錄阻擋與替代（多 weight 字檔）

## 3. UI 與雜項

- [ ] 3.1 [P] #169 viewer guides：preview 選單 Guides 項（SMPTE 安全區/中心線/格式參考——Swift ViewerGuides 對照）canvas overlay 繪製
- [ ] 3.2 [P] #67：專案卡右鍵選單加 Duplicate（既有 duplicate_project 工具 + registry 刷新）
- [x] 3.3 [P] #284：`generation_core::model_catalog::aspect_ratio_display_label`（+ `aspect_ratio_display_token`）verbatim 照抄 Swift `ImageModelConfig.aspectRatioDisplayLabel`（colon-form 原樣、underscore enum → "Landscape 16:9"/"Square HD"）；golden 測試取自上游 PR #284 刪掉的 `ImageModelConfigTests`。`tool_exec.rs::cmd_list_models` 於 Video/Image entry 加 `aspectRatioLabels`（與 `aspectRatios` 平行，additive 相容）。生成面板 picker wiring 是 trivial follow-up（`generation_view.rs` 屬別 slice，未改）。3 tests（helper golden + list_models e2e + token 邊界含在 golden 內）。
- [ ] 3.4 [P] #164：對照 Swift 快捷鍵全表列缺口，補進 menu/global_shortcuts（維持 !input predicate 慣例）；缺口清單記錄

## 4. 收尾

- [ ] 4.1 三 gate exit code 全綠；對抗審查一輪；AGENTS.md/97-audit 各項標注 PORTED

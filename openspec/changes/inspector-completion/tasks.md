## 1. 共用元件

- [x] 1.1 [P] field_components.rs：ColorField（色票格 + hex TextField，解析驗證純函式與測試）、FontPickerField（字族選單——清單來源 render_core::text 的字族常數查證）；lib.rs 註冊
  - 查證結果：render_core::text 無公開字族常數（只有 `font_for` 的 match）；canonical 清單改為 `FONT_FAMILIES`（field_components.rs），內容 = `Resources/Fonts/` 的 13 個 bundled 家族（Swift `BundledFonts.families` 排序語意），測試 pin 住 renderer 支援的家族都在清單內。系統字型清單為平台特定，延後。
  - ColorField 為跨平台 swatch + hex 輸入（無 NSColorPanel 等價物）；Enter 提交、非法輸入回復現值。

## 2. Binding 修正

- [x] 2.1 scrub_values 派生自選中 clip：`derive_scrub_values`（純函式 + 測試）——volume 走 dB（kf track 直接取樣、否則 db_from_linear）、position 為 top-left 畫布像素、scale/opacity 百分比，全部 keyframe-resolved at playhead（timeline_core::resolved_*_at / sample_keyframe_track）；無選取時列灰化（enabled=false，值 muted、不可 scrub）
  - 選取輸入為 InspectorView 公開欄位（selected_clip_ids / selected_media_asset_id / playhead_frame），沿用既有 has_clip_selected 慣例；app_root/timeline 的接線屬平行 stream（本 change 範圍禁改該二檔）。
- [x] 2.2 scrub 寫回：查名結果——工具面只有 `set_clip_properties`（volume/opacity/speed/transform 含 flip）與 `update_text`（fontSize 等），**無** set_clip_transform、無 fade 工具、無靜態 crop 工具。`scrub_commit_args`（純函式 + 測試）對映各欄位；drop-on-panel 提交（release 在面板外不提交，已知限制）。每區 reset 按鈕：Levels（volume→1）、Transform（transform+opacity 重設 + position/scale/rotation keyframe 清空，多筆 undo 步進為已知偏差）、Playback（speed→1）
  - Fade In/Out 列：綁真值顯示，寫回 DEFERRED（需要 agent_contract 增加 fade 屬性——本 change 不可改工具 schema）。
- [x] 2.3 Crop 列（toggle + aspect 選單）與 Flip 列（H/V toggles）綁真值：Flip 寫回 transform.flipHorizontal/Vertical；Crop toggle 驅動 `crop_editing_active`（公開欄位，preview overlay 接線屬平行 stream）；aspect 選單 8 個 Swift 預設（view state）——套用預設的 crop COMMIT 需靜態 crop 工具，DEFERRED（agent_contract）

## 3. Text 分頁

- [x] 3.1 TextStyle 讀取 + 寫回查證：`update_text` 支援 content/fontName/fontSize/fontWeight/color/alignment/transform；background/border 走 `set_clip_properties`（整個 TextFill 取代語意——commit 時帶回現有 color/padding/cornerRadius）。**shadow 無任何工具路徑**（parse_text_fill 只認 background/border）→ Shadow 列綁真值顯示但編輯 DEFERRED（需 agent_contract 增 shadow fill；直接 executor mutation 會繞過 undo/revision，不採用）
- [x] 3.2 UI：Content（text_area，IME，clip 切換時同步、Edited 即寫回 update_text）、Font（FontPickerField）、Size/Opacity（scrub）、Color（ColorField）、Alignment segmented（L/C/R）、Background/Border（ColorField + toggle）、Shadow（顯示綁定）、Position X/Y 列——照 Swift TextTab 分區（Content/Typography/Appearance/Layout）
  - 已知偏差：content 每鍵一筆 undo（無 undo grouping）；同 clip 期間外部改 content 不回灌輸入框（避免打斷輸入）。

## 4. Source 區

- [x] 4.1 真實 File 資料（manifest entry type/dims/duration + `video_export::source_path` 解析路徑 + fs metadata 檔案大小，ByteCountFormatter 風格格式化）、AI badge（generation_input.is_some）、Generated 參數區（generation_core::model_catalog 顯示名，fallback raw id）、Prompt + copy（cx.write_to_clipboard + 1.4s copied 回饋）
  - References strip 依 proposal Non-Goals 不做。

## 5. 驗證

- [x] 5.1 純邏輯測試 + 三 gate exit code 全綠：20 個新測試（field_components 5 + inspector_view 15）；`cargo test --workspace` EXIT=0、`cargo test -p fronda-app-shell-gpui --features desktop-app` EXIT=0（274 passed）、`cargo check -p fronda-app-shell-gpui --features desktop-app --bin fronda` EXIT=0
- [x] 5.2 對抗審查一輪（修正：fill enabled 狀態在 toggle 後不更新、alignment/crop glyph 換可渲染字元）；98-ui-parity-audit.md rows 5/8/9/13 更新
  - 互動行為（scrub 拖曳、選單、IME 輸入）僅 compile + 純測試驗證，無 gpui 互動測試（repo 既有慣例）；端到端需 app-shell selection 接線後人工驗證。

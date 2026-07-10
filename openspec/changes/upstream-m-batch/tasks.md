## 1. 資料安全與匯出

- [ ] 1.1 [P] #211 autosave：hub revision 變更後 debounce（Swift 間隔查上游）自動 save_project_state_with_siblings（有 root 時）；close_project/顯示 Home 前必存；無 root（未儲存專案）跳過；測試存檔觸發邏輯（注入時鐘或計數器）
- [ ] 1.2 [P] #138 HDR：Mp4Encoder HEVC Main10 路徑（pix_fmt yuv420p10le、colour primaries/transfer BT.2020/HLG 標記；libx265 缺時誠實回報）；ExportOptions + export UI 的 HDR 選項；小型 encode-decode 結構測試

## 2. 工具與渲染

- [ ] 2.1 [P] #176 duplicate_clips：上游契約（git show 其 PR diff/branch），完整保真（keyframes/effects/fades/links 重建 link group）、envelope/short-id、validators、工具數 56→57 四檔 + host split
- [ ] 2.2 [P] #45 arrow/line：先查 Swift 光柵化的端點座標空間（ShapeStyle/annotation 渲染碼），據此在 compositor rasterize_shape 補 arrow/line（線寬/箭頭幾何照 Swift）；黃金測試
- [ ] 2.3 [P] #65 wght：ab_glyph 變數字型軸支援查證；可行則 render_core::text 依 TextStyle.font_weight 套軸；不可行記錄阻擋與替代（多 weight 字檔）

## 3. UI 與雜項

- [ ] 3.1 [P] #169 viewer guides：preview 選單 Guides 項（SMPTE 安全區/中心線/格式參考——Swift ViewerGuides 對照）canvas overlay 繪製
- [ ] 3.2 [P] #67：專案卡右鍵選單加 Duplicate（既有 duplicate_project 工具 + registry 刷新）
- [ ] 3.3 [P] #284：aspect 標籤 helper（"16:9 (Landscape)" 類，上游 aspectRatioDisplayLabel 照抄）用於 list_models 與生成面板
- [ ] 3.4 [P] #164：對照 Swift 快捷鍵全表列缺口，補進 menu/global_shortcuts（維持 !input predicate 慣例）；缺口清單記錄

## 4. 收尾

- [ ] 4.1 三 gate exit code 全綠；對抗審查一輪；AGENTS.md/97-audit 各項標注 PORTED

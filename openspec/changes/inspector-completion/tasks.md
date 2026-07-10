## 1. 共用元件

- [ ] 1.1 [P] field_components.rs：ColorField（色票格 + hex TextField，解析驗證純函式與測試）、FontPickerField（字族選單——清單來源 render_core::text 的字族常數查證）；lib.rs 註冊

## 2. Binding 修正

- [ ] 2.1 scrub_values 派生自選中 clip：hub snapshot 的 clip transform/volume/speed/opacity（keyframe-resolved at playhead 用 timeline_core::resolved_*_at）；無選取時列灰化；純派生函式與測試
- [ ] 2.2 scrub 寫回：既有 set_clip_properties/set_clip_transform 工具路徑（查名）；每區 reset 按鈕（回預設值工具呼叫）
- [ ] 2.3 Crop 列（toggle + aspect 選單 + 數值）與 Flip 列（H/V toggles）綁真值

## 3. Text 分頁

- [ ] 3.1 TextStyle 讀取：選中 Text clip 的 style 欄位對映 UI 狀態；寫回經 update_text 工具（textStyle 參數支援度查證，不足則 executor API）
- [ ] 3.2 UI：Content（text_area）、Font/Size/Opacity/Color/Alignment/Background/Shadow/Stroke/Position 列（Swift TextTab.swift 逐區對照）

## 4. Source 區

- [ ] 4.1 真實 File 資料（manifest entry 的 dims/duration/source path；檔案大小 fs 查詢）、AI badge（generation_input.is_some）、Generated 參數區、Prompt + copy（剪貼簿既有模式）

## 5. 驗證

- [ ] 5.1 純邏輯測試 + 三 gate exit code 全綠
- [ ] 5.2 對抗審查一輪；98-ui-parity-audit.md rows 5/8/9/13 更新

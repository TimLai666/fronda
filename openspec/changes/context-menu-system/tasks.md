## 1. 元件

- [ ] 1.1 調查 gpui-ce 內建選單基元（anchored/deferred/overlay 系統與 right-click 事件——MouseButton::Right 的 on_mouse_down）；記錄選型
- [ ] 1.2 context_menu.rs：ContextMenu 元件（開啟座標、項目清單 {label, destructive, action closure}、分隔線；Esc/點外關閉——沿用 picker 的 dismiss 模式）；純狀態測試（開/關/項目觸發）

## 2. 接入

- [ ] 2.1 專案卡：右鍵 → Open/Reveal（PlatformAdapter reveal 既有介面——查 consumer 現況）/Remove from Recents（registry API）/Delete Project（確認步驟 + 刪除目錄）
- [ ] 2.2 媒體資產 tile：Rename（inline TextField，timeline tab rename 模式）/Delete/Reveal
- [ ] 2.3 資料夾 tile：Rename/Delete（內容移上層——executor 資料夾 API 查證）

## 3. 驗證

- [ ] 3.1 三 gate exit code 全綠 + 純邏輯測試
- [ ] 3.2 對抗審查一輪；98-ui-parity-audit.md row 7 更新

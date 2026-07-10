## 1. 元件

- [x] 1.1 調查 gpui-ce 內建選單基元（anchored/deferred/overlay 系統與 right-click 事件——MouseButton::Right 的 on_mouse_down）；記錄選型
  - 調查結果（gpui-ce f9a8c62）：無現成 menu widget（那在 Zed 的 `ui` crate，不在 gpui-ce），但基元齊全——`anchored()`（視窗座標定位 + `snap_to_window_with_margin` 防溢出）、`deferred().with_priority()`（畫在所有祖先之上）、`on_mouse_down(MouseButton::Right, ..)`（右鍵觸發）、`on_mouse_down_out`（capture-phase 點外偵測）、`.occlude()`（擋住底下 hover/click）。`on_click` 只回應左鍵（div.rs 的 `MouseButton::Left` match），右鍵開選單不會誤觸卡片開啟。
  - 選型：自建輕量元件 `context_menu.rs` = 純狀態機（`ContextMenuState<T>` open/close/activate，含 in-menu 確認步驟）+ `render_context_menu()`（deferred+anchored popover）。Esc 關閉走 host 的 key handler（AppRoot.handle_key_down 早退），點外走 `on_mouse_down_out`。
- [x] 1.2 context_menu.rs：ContextMenu 元件（開啟座標、項目清單 {label, destructive, action closure}、分隔線；Esc/點外關閉——沿用 picker 的 dismiss 模式）；純狀態測試（開/關/項目觸發）
  - 11 個純邏輯測試（open/close/activate/確認武裝/reopen 重置/label 切換）。危險項用既有 `theme::Status::ERROR`。action 走 entry-index → host `activate()` dispatch，item id 為 `&'static str`。

## 2. 接入

- [x] 2.1 專案卡：右鍵 → Open/Reveal（PlatformAdapter reveal 既有介面——查 consumer 現況）/Remove from Recents（registry API）/Delete Project（確認步驟 + 刪除目錄）
  - Reveal 用既有 free function `platform_adapter::reveal_in_file_manager`（export_view.rs 已是 consumer，audit 的「無 consumer」註記已過時）。Remove from Recents 用 `ProjectRegistry::remove(id)` + `project_registry_store::save_to`（API 已存在，project_registry_store.rs 未改）。Delete Project 為 in-menu 確認（第一次點武裝成 "Confirm Delete"，第二次執行）；只刪 `.palmier` 副檔名目錄（registry 損壞防呆），之後移除 recents 條目。
- [ ] 2.2 媒體資產 tile：Rename（inline TextField，timeline tab rename 模式）/Delete/Reveal
  - 未接：media_panel_view.rs 由平行工作流持有，等 media-library merge 後由整合者接線。接線方式：MediaPanelView 持 `ContextMenuState<AssetTarget{asset_id}>`；tile 加 `on_mouse_down(MouseButton::Right, ..)` 呼叫 `open_at(e.position.x/y.as_f32(), target)`；view 根尾端 `.when_some(state.open_menu().cloned(), |el, open| el.child(render_context_menu(point(px(open.x), px(open.y)), entries, open.confirming, cx, on_activate, on_dismiss)))`；Rename → 既有 inline rename 路徑（timeline tab 的 text_field 模式），Delete → executor delete_media，Reveal → manifest 解析出的本地路徑丟 `platform_adapter::reveal_in_file_manager`；Esc 在 MediaPanelView key handler 早退關閉。
- [ ] 2.3 資料夾 tile：Rename/Delete（內容移上層——executor 資料夾 API 查證）
  - 未接（同上，media_panel_view.rs 平行持有）。同一元件；`ContextMenuState<FolderTarget{folder_id}>`，或與 2.2 共用一個 enum target 的單一 state。Delete 走 executor 的資料夾刪除（內容移回上層）語意，接線前先查證 executor API。

## 3. 驗證

- [x] 3.1 三 gate exit code 全綠 + 純邏輯測試
  - 2026-07-10（1.1/1.2/2.1 範圍）：`cargo test --workspace` EXIT=0、`cargo test -p fronda-app-shell-gpui --features desktop-app` EXIT=0（265 passed，含 11 個 context_menu 測試）、`cargo check -p fronda-app-shell-gpui --features desktop-app --bin fronda` EXIT=0。2.2/2.3 接線後需重跑。
- [ ] 3.2 對抗審查一輪；98-ui-parity-audit.md row 7 更新

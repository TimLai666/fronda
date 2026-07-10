## 1. 元件

- [x] 1.1 調查 gpui-ce 內建選單基元（anchored/deferred/overlay 系統與 right-click 事件——MouseButton::Right 的 on_mouse_down）；記錄選型
  - 調查結果（gpui-ce f9a8c62）：無現成 menu widget（那在 Zed 的 `ui` crate，不在 gpui-ce），但基元齊全——`anchored()`（視窗座標定位 + `snap_to_window_with_margin` 防溢出）、`deferred().with_priority()`（畫在所有祖先之上）、`on_mouse_down(MouseButton::Right, ..)`（右鍵觸發）、`on_mouse_down_out`（capture-phase 點外偵測）、`.occlude()`（擋住底下 hover/click）。`on_click` 只回應左鍵（div.rs 的 `MouseButton::Left` match），右鍵開選單不會誤觸卡片開啟。
  - 選型：自建輕量元件 `context_menu.rs` = 純狀態機（`ContextMenuState<T>` open/close/activate，含 in-menu 確認步驟）+ `render_context_menu()`（deferred+anchored popover）。Esc 關閉走 host 的 key handler（AppRoot.handle_key_down 早退），點外走 `on_mouse_down_out`。
- [x] 1.2 context_menu.rs：ContextMenu 元件（開啟座標、項目清單 {label, destructive, action closure}、分隔線；Esc/點外關閉——沿用 picker 的 dismiss 模式）；純狀態測試（開/關/項目觸發）
  - 11 個純邏輯測試（open/close/activate/確認武裝/reopen 重置/label 切換）。危險項用既有 `theme::Status::ERROR`。action 走 entry-index → host `activate()` dispatch，item id 為 `&'static str`。

## 2. 接入

- [x] 2.1 專案卡：右鍵 → Open/Reveal（PlatformAdapter reveal 既有介面——查 consumer 現況）/Remove from Recents（registry API）/Delete Project（確認步驟 + 刪除目錄）
  - Reveal 用既有 free function `platform_adapter::reveal_in_file_manager`（export_view.rs 已是 consumer，audit 的「無 consumer」註記已過時）。Remove from Recents 用 `ProjectRegistry::remove(id)` + `project_registry_store::save_to`（API 已存在，project_registry_store.rs 未改）。Delete Project 為 in-menu 確認（第一次點武裝成 "Confirm Delete"，第二次執行）；只刪 `.palmier` 副檔名目錄（registry 損壞防呆），之後移除 recents 條目。
- [x] 2.2 媒體資產 tile：Rename（inline TextField，timeline tab rename 模式）/Delete/Reveal
  - 已接（2026-07-10 整合）：單一 `ContextMenuState<LibraryMenuTarget>`（Asset/Folder enum，2.2/2.3 共用）；asset tile `on_mouse_down(Right)` 開選單（Rename / Reveal in File Manager / Delete）。Rename → 新增 `asset_rename_field` inline TextField（folder rename 同款：Enter 提交 `rename_media`、空名取消、Esc bubble 到 panel key handler、click-away 提交；編輯中 tile 卸下選取/拖曳 handler）。Reveal → `state.items` 已解析的本地路徑（僅存在的檔案）丟 `platform_adapter::reveal_in_file_manager`，無本地檔時選項省略（project-card missing 模式；activate 時重解析，消失則安全降級 no-op）。Delete → 右鍵目標在選取內刪整個選取（Swift contextTargetIds），否則只刪該資產；`delete_media` 只移除 manifest 條目（不動磁碟）故用 plain destructive、不走 arm-confirm。sync 時 prune：編輯/選單目標被 MCP 刪除即關閉。4 個純測試（entry id 順序、無檔省略 Reveal、delete 非 confirm）。
- [x] 2.3 資料夾 tile：Rename/Delete（內容移上層——executor 資料夾 API 查證）
  - 已接：folder tile 右鍵 → Open / Rename / Delete（Swift FolderTileView.contextMenuItems 同構）。Rename 走既有 `begin_folder_rename`；開始新 rename 時先 commit 進行中的 rename（Swift focus-loss commit）。**executor 語意查證（tool_exec.rs cmd_delete_folder）：直屬資產移到 library 根（`folder_id = None`，非移到上層資料夾）；子資料夾不重新掛載——其 `parent_folder_id` 懸空，Folders 視圖無法到達（Grouped 視圖仍可見）**。與 Swift 的差異屬 executor 層既有行為，選單照實接線並在 code 註記。

## 3. 驗證

- [x] 3.1 三 gate exit code 全綠 + 純邏輯測試
  - 2026-07-10（1.1/1.2/2.1 範圍）：`cargo test --workspace` EXIT=0、`cargo test -p fronda-app-shell-gpui --features desktop-app` EXIT=0（265 passed，含 11 個 context_menu 測試）、`cargo check -p fronda-app-shell-gpui --features desktop-app --bin fronda` EXIT=0。2.2/2.3 接線後需重跑。
  - 2026-07-10 重跑（2.2/2.3 接線後）：`cargo test --workspace` EXIT=0、`cargo test -p fronda-app-shell-gpui --features desktop-app` EXIT=0（387 passed，含 4 個 tile-menu 測試）、`cargo check -p fronda-app-shell-gpui --features desktop-app --bin fronda` EXIT=0。
- [x] 3.2 對抗審查一輪；98-ui-parity-audit.md row 7 更新
  - 審查發現並修正：開始新 inline rename 未 commit 進行中的 rename（會靜默丟編輯）→ begin_* 先 commit。確認安全性：activate 時 entries 以當下狀態重建，與顯示不同步時降級為 no-op（絕不誤執行）；右鍵不觸發左鍵路徑（`on_click` 僅回應左鍵）；grid click-away 為 Left-only 不吃右鍵；menu 目標被 MCP 刪除時 sync prune 關閉選單。已知殘留：delete_folder 的子資料夾懸空（見 2.3，executor 層）；點進 rename TextField 重定游標會觸發 grid click-away commit（folder rename 既有行為，asset 同構）。row 7 已更新為 DONE。

## 1. 調查與基礎

- [x] 1.1 查證 gpui-ce 外部檔案拖放 API（~/.cargo/git/checkouts/gpui-ce-*/crates/gpui：搜 ExternalPaths / on_drop / DragMoveEvent 的 external 分支與官方範例），記錄可用面與限制（Windows/Linux/macOS 差異）到 change 附註；無外部 drop 支援時記錄阻擋並縮範圍至面板間拖放
- [x] 1.2 drag_payload 擴充：AssetDrag{asset_id, media_type} payload 型別 + 單元測試（既有 moment-segment parse 模式）

## 2. 實作

- [x] 2.1 媒體面板：外部檔案 drop → import 流程（依 1.1 的 API）；hover 高亮（AppTheme 樣式）
- [x] 2.2 資產 tile on_drag（AssetDrag payload + 拖曳預覽）；timeline 軌道 drop target：hover 顯示插入線（既有 snap 指示模式）、release 呼叫 add_clips 於指標 frame（trackIndex + startFrame 參數——查 add_clips 支援度，不足則 executor API）→ 實際採 insert_clips（見附註「放置工具決策」）
- [x] 2.3 generation ref tiles drop target：型別/上限檢查沿用 click-to-pick 的驗證函式

## 3. 驗證

- [x] 3.1 純邏輯測試（payload、放置 frame 計算）+ 三 gate exit code 全綠 → 6 新測試（timeline_core drag_payload 4、generation_view drop_rejection 2）；`cargo test --workspace` EXIT=0、`cargo test -p fronda-app-shell-gpui --features desktop-app` EXIT=0（326 passed）、`cargo check -p fronda-app-shell-gpui --features desktop-app --bin fronda` EXIT=0（2026-07-10）
- [x] 3.2 對抗審查一輪；98-ui-parity-audit.md row 6 更新

## 附註（1.1 調查結果，gpui-ce f9a8c62）

### 外部檔案拖放：三平台皆支援，元素端 API 統一

OS 檔案拖進視窗由各平台後端轉成 `PlatformInput::FileDrop(FileDropEvent)`，
core（`gpui/src/window.rs:4660-4699`）再把它翻譯成「內部 drag」：Entered 時建立
`AnyDrag { value: Arc<ExternalPaths>, .. }` 並合成左鍵 MouseMove，Submit 合成
MouseUp。因此元素端只要 `.on_drop::<ExternalPaths>()` + `.drag_over::<ExternalPaths>()`，
三平台行為一致：

- Windows：`gpui_windows/src/window.rs:1105-1191` — 完整 IDropTarget
  （DragEnter/DragOver/DragLeave/Drop → Entered(paths)/Pending/Exited/Submit）。
- macOS：`gpui_macos/src/window.rs:2766/2792` — NSDraggingDestination。
- Linux：Wayland `gpui_linux/src/linux/wayland/client.rs:2429/2482`（data_device）、
  X11 `gpui_linux/src/linux/x11/client.rs:894/928`（XDND）。

限制/陷阱（實測原始碼，非猜測）：

1. `can_drop` 回 false 會「吞掉」整個 drag（`div.rs:2517` 先 `active_drag.take()`
   再跑 predicate，false 時不歸還、不 stop_propagation）——驗證一律放在 `on_drop`
   內做，本 change 不使用 `can_drop`。
2. `on_drag_move::<T>` 在 Capture phase 對「任何」該型別的 drag 每次滑鼠移動都會觸發
   （不限 hitbox 內；`div.rs:315-339`），需自行用 `e.bounds.contains()` 篩選。
3. drop 派發是 bubble phase 內層優先 + `stop_propagation`（`div.rs:2506-2537`），
   且以 TypeId 配對——內外層不同 payload 型別互不遮蔽，無 Swift `.onDrop` 遮蔽問題。
4. MouseUp 後未被接走的 drag 由 window 統一清除（`window.rs:4769-4773`），
   timeline 的 hover 指示器據此在 render 時清 stale 狀態。
5. 外部拖曳的 preview 由平台畫檔案 icon（`ExternalPaths` 的 Render 是 Empty）。

### 放置工具決策（2.2）

Swift 基準（`TimelineView.swift performDragOperation`）：預設 drop =
`addClips(trackIndex:startFrame:)`（指標 frame 覆蓋放置 + linked audio），
Cmd = ripple insert。Rust 工具面：

- `add_clips` 無 startFrame 參數，且 `cmd_add_clips` 實際呼叫
  `place_clips(track, 0, ..)` —— 固定在 frame 0 覆蓋放置（與工具描述
  「end of the timeline」不符，這是既有 divergence，本 change 不動 tool 層）。
  直接用它會清掉目標軌 frame 0 起的內容，不可接受。
- `insert_clips` 收 trackIndex + frame（schema 沒列 trackIndex 但 executor 必填），
  ripple insert：不毀內容、單一 undo step、linked A/V（CLP-007/008/RPL-010）、
  fps 警告（resolve_placement）。空白處/時間軸尾端 drop 時行為與覆蓋放置相同。

採用：**有相容軌 → `insert_clips`（ripple）；無相容軌 → `add_clips` auto-create**
（自動建軌 + 首 clip settings 偵測，放 frame 0）。與 Swift 預設（覆蓋放置）的差異：
drop 在既有 clip 之前時後續內容會右移而非被覆蓋。要完全對齊需給 `add_clips` 加
`startFrame`（agent tool schema 變更，需獨立 spec change）——列為 follow-up。

### 已知縮減範圍（follow-up）

- 拖多選（Swift 多行 payload）：v1 只拖單一資產。
- 外部檔案拖到 timeline 直接成 clip（Swift 支援 .fileURL）：v1 只有媒體面板收外部檔案。
- 資料夾 tile 收資產 drop（移入資料夾）與資料夾整包匯入：不在 spec 三需求內。
- import 進當前資料夾（Swift `importFinderItems(into: currentFolderId)`）：
  `import_media` tool 無 folderId 參數，v1 匯入到根。
- assigned First/Last tile 的替換式 drop：v1 只掛空 tile。

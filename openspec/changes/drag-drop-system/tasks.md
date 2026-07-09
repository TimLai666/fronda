## 1. 調查與基礎

- [ ] 1.1 查證 gpui-ce 外部檔案拖放 API（~/.cargo/git/checkouts/gpui-ce-*/crates/gpui：搜 ExternalPaths / on_drop / DragMoveEvent 的 external 分支與官方範例），記錄可用面與限制（Windows/Linux/macOS 差異）到 change 附註；無外部 drop 支援時記錄阻擋並縮範圍至面板間拖放
- [ ] 1.2 drag_payload 擴充：AssetDrag{asset_id, media_type} payload 型別 + 單元測試（既有 moment-segment parse 模式）

## 2. 實作

- [ ] 2.1 媒體面板：外部檔案 drop → import 流程（依 1.1 的 API）；hover 高亮（AppTheme 樣式）
- [ ] 2.2 資產 tile on_drag（AssetDrag payload + 拖曳預覽）；timeline 軌道 drop target：hover 顯示插入線（既有 snap 指示模式）、release 呼叫 add_clips 於指標 frame（trackIndex + startFrame 參數——查 add_clips 支援度，不足則 executor API）
- [ ] 2.3 generation ref tiles drop target：型別/上限檢查沿用 click-to-pick 的驗證函式

## 3. 驗證

- [ ] 3.1 純邏輯測試（payload、放置 frame 計算）+ 三 gate exit code 全綠
- [ ] 3.2 對抗審查一輪；98-ui-parity-audit.md row 6 更新

## 1. timeline_model 擴充

- [x] 1.1 實作修改後需求「Clip selection is view state」與「Same-track clip drag with snapping commits via the shared executor」的純邏輯，依 design 決策「多選與空白清除（timeline_model 擴充選取語意）」「跨軌拖曳（ClipDrag 增加目標軌）」：新增 select_all()；ClipDrag 增 origin_track_index/proposed_track_index、update_clip_drag_track(content_y)（僅同類型軌可落）、take_clip_drag 改回 (id, toTrack, toFrame) 且同軌零位移回 None。驗證：新增測試——select_all、y 對應軌與同類型限制（video 拖到 audio 軌維持原軌）、跨軌 take 回傳新 toTrack、同軌零位移回 None
- [x] 1.2 實作需求「Trim handles on clip edges」的純邏輯，依 design 決策「trim 手把（新增 TrimDrag session）」：TrimDrag/TrimEdge 與 begin/update/take_trim_drag，Start 邊夾在 0 到 end-1、End 邊夾在 start+1 以上，零變化回 None。驗證：新增測試覆蓋兩邊夾界（含 spec 的 Clamp prevents zero-length clips 例）與零變化

## 2. view 手勢與選單

- [x] 2.1 實作修改後選取與拖曳的 UI 部分：剪輯 on_mouse_down 讀修飾鍵（Shift 或 Cmd/Ctrl 走 toggle_select）並 stop_propagation；畫布空白 on_mouse_down 清選取；on_drag_move 同時更新 x 提案與 update_clip_drag_track（y 相對畫布）；拖曳中剪輯畫在 proposed_track_index 對應軌；on_drop 以 toTrack 與 toFrame 呼叫 move_clips。驗證：cargo check -p fronda-app-shell-gpui --features desktop-app --bin fronda 通過
- [x] 2.2 實作 trim 手把 UI：剪輯左右 6px 熱區 on_mouse_down（stop_propagation）加 on_drag 掛 TrimDragToken；畫布 on_drag_move 與 on_drop 對 TrimDragToken 更新與落地（End 邊 durationFrames、Start 邊 durationFrames + move_clips）；拖曳中以 proposed_frame 畫預覽邊界。驗證：cargo check --features desktop-app 通過
- [x] 2.3 實作修改後需求「Edit menu actions operate on the shared state」的新增動作，依 design 決策「選單 Trim 與 RippleDelete（timeline_view 方法）」：TimelineView 增 select_all(cx)、trim_selected_to_playhead(edge, cx)、ripple_delete_selected(cx)（依軌分組呼叫 ripple_delete_ranges、成功清選取）；app_root 的 SelectAll/TrimStartToPlayhead/TrimEndToPlayhead/RippleDelete 分支接線。驗證：新增 hub 整合測試——set_clip_properties durationFrames 使剪輯結尾變為目標 frame 且 undo 還原、ripple_delete_ranges 使後續剪輯前移（spec 的 Ripple delete closes the gap 例）；cargo check --features desktop-app 通過

## 3. 全面驗證

- [x] 3.1 cargo test --workspace、cargo clippy --workspace --tests -- -D warnings、cargo fmt --all -- --check 全過；app smoke（啟動加 MCP get_timeline）。驗證：指令輸出審閱

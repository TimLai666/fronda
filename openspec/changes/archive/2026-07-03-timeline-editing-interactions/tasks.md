## 1. timeline_model 互動邏輯

- [x] 1.1 實作需求「Playhead scrubbing on the ruler」「Clip selection is view state」與拖曳提案邏輯（需求「Same-track clip drag with snapping commits via the shared executor」的純邏輯部分），依 design 決策「timeline_model 承載互動狀態與純邏輯」：TimelineState 增加 selected_clip_ids、ClipDrag、select_only/toggle_select/clear_selection、scrub_to_content_x、begin_clip_drag/update_clip_drag/take_clip_drag、track_index_of_clip；Cargo.toml 加 timeline_core 依賴。驗證：新增測試覆蓋 scrub 夾 0、選取轉移、proposed 夾 0、snap 命中設 snap_x_frame 且採目標 frame、零位移 take 回 None、track_index_of_clip

## 2. view 手勢接線

- [x] 2.1 實作需求「Same-track clip drag with snapping commits via the shared executor」的 UI 部分，依 design 決策「座標換算以 content-origin 快取解決」與「剪輯拖曳走 gpui drag 三件組、放開以 on_drop 落地」：ruler 的 on_mouse_down scrub 播放頭；剪輯 on_mouse_down 選取＋begin、on_drag 掛 token；畫布 on_drag_move 更新提案並畫 snap 線、on_drop 以 move_clips 落地（零位移不發）；選取剪輯高亮、拖曳中以 proposed_start 畫位置。驗證：cargo check -p fronda-app-shell-gpui --features desktop-app --bin fronda 通過；手動抽查拖曳＋snap＋Ctrl+Z（人工項記錄於完成報告）

## 3. 選單編輯動作

- [x] 3.1 實作需求「Edit menu actions operate on the shared state」，依 design 決策「選單編輯動作經共享 executor」：app_root 的 Undo/Redo 直呼共享 executor 的 undo/redo（Err 靜默）；Delete 與 SplitAtPlayhead 透過 timeline_view.update 呼叫 delete_selected/split_selected_at_playhead（內呼 remove_clips/split_clip、成功清選取、Err 靜默）。驗證：新增整合測試——共享 executor move_clips 後執行 undo 還原 start_frame；cargo check --features desktop-app 通過

## 4. 全面驗證

- [x] 4.1 cargo test --workspace、cargo clippy --workspace --tests -- -D warnings、cargo fmt --all -- --check 全過；跑 app smoke（啟動、MCP get_timeline 正常）。驗證：指令輸出審閱

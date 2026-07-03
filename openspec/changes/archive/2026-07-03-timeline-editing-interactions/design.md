## Context

TimelineView 由共享狀態驅動渲染（revision 重建、view 狀態保留），但無任何輸入處理。可用基礎：agent_contract 工具 move_clips（clipIds/toTrack/toFrame）、remove_clips（clipIds）、split_clip（clipId/frame）、undo、redo，皆 undo-tracked 且成功後遞增 revision；timeline_core::snapping 提供 SnapTarget/SnapState/find_snap（含 sticky 行為與 THRESHOLD_PIXELS）；app_root 已示範 gpui 拖曳三件組（on_mouse_down 起 session、on_drag 掛 token 與空白 preview、容器 on_drag_move::<Token> 更新）；DragMoveEvent 帶 bounds 可做視窗座標到元素座標換算；TimelineState 已有 snap_x_frame 黃色對齊線欄位與 x/frame 換算。

## Goals / Non-Goals

**Goals:**

- 播放頭可點可拖（ruler 區域），剪輯可點選取（高亮）、可同軌水平拖曳且會 snap，放開即透過共享 executor 落地（可 Undo）
- Undo/Redo/Delete/SplitAtPlayhead 選單與快捷鍵經共享 executor 生效，MCP 與 UI 共用同一份 undo 歷史
- 互動邏輯（拖曳換算、snap、選取）為 timeline_model 純函式，可單元測試

**Non-Goals:**

- 跨軌拖曳、trim 手把、多選 rubber-band、複製貼上、TrimStart/End、SelectAll、播放 transport、媒體拖放

## Decisions

### timeline_model 承載互動狀態與純邏輯

TimelineState 增加 selected_clip_ids: Vec<String> 與 clip_drag: Option<ClipDrag>；ClipDrag { clip_id, grab_offset_frames, proposed_start, snap_state }。純方法：
- select_only(id)／toggle_select(id)／clear_selection()
- scrub_to_content_x(x)：playhead_frame = max(0, frame_for_x(scroll_x + x))
- begin_clip_drag(clip_id, pointer_frame)：記 grab offset，起始 proposed_start＝原 start
- update_clip_drag(pointer_frame)：proposed = pointer − grab，夾 0 為下限；以其餘剪輯邊界＋播放頭建 SnapTarget（probe offsets ＝ [0, duration]），find_snap 命中時採 snap 值並設 snap_x_frame，否則清除
- take_clip_drag() -> Option<(clip_id, to_frame)>：結束並清 snap 線；proposed 與原 start 相同時回 None（不發 no-op mutation）
- track_index_of_clip(id)：由 ClipSlot.track_id 對應 tracks 序號，供 move_clips 的 toTrack

snapping 依 timeline_core 現有函式，不重造；app_shell_gpui Cargo.toml 新增 timeline_core 依賴。

### 座標換算以 content-origin 快取解決

gpui mouse 事件是視窗座標。在 ruler 內容 div 與剪輯畫布共用的左緣放一個零尺寸 gpui canvas 元素，prepaint 時把 bounds.origin.x 寫回 view 的 content_origin_x 欄位（每幀更新，捲動與面板調整自動跟上）。on_mouse_down 的內容座標＝event.position.x − content_origin_x。拖曳中的 on_drag_move 直接用 DragMoveEvent.bounds 換算。

### 剪輯拖曳走 gpui drag 三件組、放開以 on_drop 落地

剪輯 div：on_mouse_down 選取＋begin_clip_drag；on_drag(ClipDragToken, 空白 preview)。剪輯畫布容器：on_drag_move::<ClipDragToken> 呼叫 update_clip_drag 並 notify（拖曳中以 proposed_start 畫該剪輯與黃色 snap 線）；on_drop::<ClipDragToken> 呼叫 take_clip_drag，有位移時鎖共享 executor 執行 move_clips { clipIds:[id], toTrack: track_index_of_clip, toFrame }，失敗 eprintln；成功由 revision 機制重建畫面。替代方案是自行追蹤 mouse up，gpui drag 系統已處理捕捉與結束，不重造。

### 選單編輯動作經共享 executor

app_root：Undo/Redo 分支鎖 executor 執行 "undo"/"redo"（Err 靜默——空 undo stack 屬正常）；Delete 與 SplitAtPlayhead 需要 view 的選取與播放頭，透過 timeline_view Entity 的 update 呼叫 TimelineView::delete_selected(cx)／split_selected_at_playhead(cx)，其內鎖 executor 呼叫 remove_clips／split_clip 後清選取。選取狀態留在 view（不寫回 core Timeline.selected_clip_ids），因為那是 agent 契約欄位、與 UI 選取語意的同步屬後續 change。

## Implementation Contract

- 行為：點 ruler 任一點播放頭跳至該 frame（負值夾 0）；點剪輯該剪輯高亮、再點其他剪輯高亮轉移；水平拖曳剪輯放開後，MCP get_timeline 反映新 start_frame，Cmd/Ctrl+Z 還原、Cmd/Ctrl+Shift+Z 重做；拖曳接近其他剪輯邊界或播放頭（THRESHOLD_PIXELS 內）時吸附並顯示黃色對齊線；Delete 刪除選取剪輯；SplitAtPlayhead 把選取剪輯於播放頭切成兩段（播放頭在剪輯範圍外時該工具回錯誤、UI 靜默不變）。
- 介面／資料形狀：如 Decisions 所列 TimelineState 方法簽名；TimelineView::delete_selected(&mut self, cx)、split_selected_at_playhead(&mut self, cx)；AppRoot Undo/Redo 直呼共享 executor。
- 失敗模式：executor 鎖失敗或工具回 Err（如 undo stack 空、split 超界）→ UI 不變、不 panic；take_clip_drag 對零位移回 None 不發 mutation。
- 驗收標準：
  - timeline_model 新測試：scrub 夾 0 與換算、選取轉移、拖曳 proposed 夾 0、snap 命中採用目標 frame 且設 snap_x_frame、零位移 take 回 None、track_index_of_clip 正確
  - 整合測試（app_shell_gpui，無 gpui）：對共享 executor 執行 move_clips 後 undo 還原 start_frame（證明選單 Undo 走的路徑有效）
  - cargo test --workspace、clippy -D warnings、desktop-app check 全過
  - 手動抽查：跑 app 拖曳剪輯、Ctrl+Z 還原（人工項，記錄於完成報告）
- 範圍界線：in scope＝上述互動與選單四動作；out of scope＝Non-Goals 全部。

## Risks / Trade-offs

- [gpui drag 在 headless 無法自動化] → 互動邏輯全部下沉 timeline_model 純測試；gpui 層只剩座標換算與轉呼叫
- [UI 選取與 core selected_clip_ids 脫鉤] → 明確記為 view-only 選取；agent 的 select 工具語意同步屬後續 change
- [拖曳中每次 mouse move 全量 notify] → 與現有 resize drag 相同成本模式，可接受
- [content_origin_x 每幀 prepaint 寫回 view] → 零尺寸 canvas 只做一次指標寫入，成本可忽略

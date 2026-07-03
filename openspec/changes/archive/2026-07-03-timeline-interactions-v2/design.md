## Context

v1 已建立：TimelineState 的選取（select_only/toggle_select/clear_selection 皆已存在）、ClipDrag（同軌、snap、take 回 (clip_id, to_frame)）、content-origin 座標換算、gpui drag 三件組、run_shared_tool/commit 管道、revision 重繪。工具面：move_clips 支援 toTrack；set_clip_properties 的 trimStartFrame/trimEndFrame 是素材源 trim 偏移、不改 timeline 幾何；改變剪輯的時間軸長度用 durationFrames（結尾邊），起點邊需 durationFrames + move_clips 兩步（各自 undo-tracked，起點 trim 需兩次 undo）；ripple_delete_ranges 需要 trackIndex 與 ranges[{start,end}]。TrackRow 有 height，軌道由上而下排列，y 座標可對應軌 index。

## Goals / Non-Goals

**Goals:**

- Shift/Cmd 點擊多選、SelectAll、點空白清選取
- 跨軌拖曳（限同類型軌），拖曳中畫在目標軌
- 剪輯邊緣 trim 手把（左右各 6px 熱區）與 TrimStart/EndToPlayhead、RippleDelete 選單
- 全部 undo-tracked、MCP 可見、邏輯下沉 timeline_model 純測試

**Non-Goals:**

- transport、匯入、縮圖、rubber-band、拖曳自動捲動、多剪輯同時拖曳

## Decisions

### 多選與空白清除（timeline_model 擴充選取語意）

點擊剪輯：無修飾鍵 select_only、Shift 或 Cmd/Ctrl 時 toggle_select（方法已存在，僅 view 接修飾鍵）。新增 select_all()（全部 clip id）。剪輯畫布空白處 on_mouse_down 清除選取（剪輯的 mouse_down 已 stop 掉冒泡則不衝突；gpui 事件順序為子先於父，剪輯 handler 內呼叫 cx.stop_propagation）。

### 跨軌拖曳（ClipDrag 增加目標軌）

ClipDrag 增加 origin_track_index 與 proposed_track_index。新增 update_clip_drag_track(content_y)：以 tracks 的累計高度把 y 對應軌 index，僅當目標軌 kind 與來源軌相同才更新 proposed_track_index，否則維持原值。take_clip_drag 改回傳 Option<(String, usize, i64)>（id、toTrack、toFrame），零位移且同軌時回 None。view 的 on_drag_move 同時换算 x 與 y（DragMoveEvent.bounds 為剪輯畫布，y 相對畫布頂緣加 scroll_y）；拖曳中的剪輯以 proposed_track_index 對應軌的 y 區間繪製。

### trim 手把（新增 TrimDrag session）

TimelineState 新增 trim_drag: Option<TrimDrag>；TrimDrag { clip_id, edge: TrimEdge::Start|End, original_start, original_duration, proposed_frame }。begin_trim_drag(clip_id, edge, pointer_frame)、update_trim_drag(pointer_frame)（Start 邊夾在 [0, end-1]、End 邊夾在 [start+1, ∞)）、take_trim_drag() 回 Option<(clip_id, edge, frame)>，無變化回 None。落地：edge End（邊界 F）→ set_clip_properties { clipIds:[id], properties:{ durationFrames: F - start } }；edge Start（邊界 F）→ 先 set_clip_properties durationFrames = end - F，再 move_clips 到 F（兩個 undo 步驟，記為明確取捨）。view：剪輯左右 6px 子元素各自 on_mouse_down（stop_propagation 避免觸發移動拖曳）＋on_drag 掛 TrimDragToken；畫布 on_drag_move::<TrimDragToken> 與 on_drop::<TrimDragToken> 對應更新與落地；拖曳中以 proposed_frame 畫該剪輯的預覽邊界。trim 不做 snap（Swift 基準的 trim 亦以自由拖曳為主，snap 留給移動）。

### 選單 Trim 與 RippleDelete（timeline_view 方法）

trim_selected_to_playhead(edge, cx)：對每個選取剪輯以播放頭為邊界套用與手把相同的落地規則；播放頭不在 (start, end) 開區間內的剪輯直接略過。ripple_delete_selected(cx)：把選取剪輯依軌分組，對每軌以剪輯的 [start, start+duration) 集合呼叫 ripple_delete_ranges（trackIndex＋ranges），成功後清選取。app_root 的 TrimStartToPlayhead/TrimEndToPlayhead/RippleDelete/SelectAll 分支透過 timeline_view.update 轉呼叫。

## Implementation Contract

- 行為：Shift/Cmd 點擊第二個剪輯後兩者皆高亮；SelectAll 後全部高亮；點空白全部取消。拖曳剪輯到同類型另一軌放開，MCP get_timeline 顯示剪輯換軌且 undo 可還原。拖曳剪輯右緣向左放開後 duration 縮短（durationFrames 生效）、左緣向右放開後起點與長度同步改變（durationFrames + move_clips），皆可 undo（起點 trim 為兩步 undo）。選取剪輯後 TrimStartToPlayhead 使剪輯起點變為播放頭（播放頭在範圍外則不變）。RippleDelete 刪除選取剪輯且同軌後續剪輯前移補位。
- 介面／資料形狀：select_all()；ClipDrag 增 origin_track_index/proposed_track_index，update_clip_drag_track(content_y: f32)，take_clip_drag() -> Option<(String, usize, i64)>；TrimDrag/TrimEdge 與 begin/update/take_trim_drag 如 Decisions；TimelineView::trim_selected_to_playhead(TrimEdge, cx)、ripple_delete_selected(cx)、select_all(cx)。
- 失敗模式：工具 Err 一律 UI 不變（沿用 run_shared_tool）；trim 夾界保證不產生零長或負長剪輯提案；跨軌拖曳到不同類型軌時停留原軌。
- 驗收標準：
  - timeline_model 新測試：select_all、多選 toggle、y→軌對應與同類型限制、跨軌 take 回傳 toTrack、trim 夾界（Start 不越過 end-1、End 不低於 start+1）、trim 零變化回 None
  - editor_state_hub 整合測試：set_clip_properties durationFrames 後剪輯結尾變為目標 frame 且 undo 還原；ripple_delete_ranges 後後續剪輯前移
  - cargo test --workspace、clippy -D warnings、desktop-app check 全過；app smoke（啟動＋MCP 回應）
- 範圍界線：in scope＝上述互動與四個選單動作；out of scope＝Non-Goals 全部。

## Risks / Trade-offs

- [起點 trim 需兩個 mutation，Undo 需按兩次] → 記為 v2 明確取捨；合併為單一 undo 需要 composite 工具，屬 MCP 契約變更、不在本 change 內做
- [gpui 事件冒泡順序若與假設不符，空白清選取會誤觸] → 剪輯與手把 handler 一律 stop_propagation，行為以人工抽查確認
- [跨軌拖曳中 y 對應在軌高度被調整時漂移] → 對應函式每次以當前 tracks 高度計算，無快取

## Why

timeline 互動 v1 只有單選與同軌拖曳，選單的 SelectAll、TrimStartToPlayhead、TrimEndToPlayhead、RippleDelete 仍是 no-op，剪輯無法跨軌移動也無法用邊緣手把調整長度。所需的 mutation 工具全部已存在（move_clips 的 toTrack、set_clip_properties 的 trimStartFrame/trimEndFrame、ripple_delete_ranges），v1 也已建立拖曳、選取、snap、revision 重繪的完整管道，v2 是同一模式的直接擴充。

## What Changes

- 多選：Shift/Cmd(Ctrl)+點擊加入選取（toggle_select 已存在），SelectAll 選取全部剪輯，點空白畫布清除選取
- 跨軌拖曳：拖曳中以指標 y 對應目標軌（同類型軌才可落），放開時 move_clips 帶目標 toTrack；拖曳中剪輯畫在目標軌位置
- trim 手把：剪輯左右邊緣 6px 熱區水平拖曳，放開時以 set_clip_properties 調整 trimStartFrame/trimEndFrame（undo-tracked）；拖曳中預覽新邊界
- TrimStartToPlayhead/TrimEndToPlayhead：對選取剪輯以播放頭位置設 trim（播放頭在剪輯範圍外時工具回錯誤、UI 靜默）
- RippleDelete：刪除選取剪輯所在軌上對應範圍並前移後續剪輯；同時修正 ripple_delete_ranges 工具的半成品行為（原本只刪不移，未套用 timeline_core 的 ripple shift 數學）
- 全部經共享 executor，undo 歷史與 MCP 共用

## Non-Goals

- 播放 transport（change B）、媒體匯入與縮圖（change C）
- rubber-band 框選、拖曳中自動捲動、多剪輯一起拖曳（多選僅作用於 Delete/Trim/RippleDelete/Split）

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `timeline-interactions`: 選取由單選擴為多選（含 SelectAll、空白處清除）；拖曳由同軌擴為跨軌（同類型軌）；新增 trim 手把與 TrimStart/End/RippleDelete 選單行為

## Impact

- Affected specs: 修改 `timeline-interactions`
- Affected code:
  - Modified:
    - crates/agent_contract/src/tool_exec.rs
    - crates/app_shell_gpui/src/timeline_model.rs
    - crates/app_shell_gpui/src/timeline_view.rs
    - crates/app_shell_gpui/src/app_root.rs
  - New: (none)
  - Removed: (none)

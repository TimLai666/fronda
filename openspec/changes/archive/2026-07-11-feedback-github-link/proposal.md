## Why

`send-feedback-tool` 移植了工具 + `FeedbackSender` seam，但實際傳送被 Convex/自有後端 host-gate 卡住（該 change 的 task 3.1）。產品決策已定（2026-07-11）：**不建回饋後端**，改把「送出回饋」連結到 Fronda 的 GitHub issues。App 選單的 Send Feedback 目前是空 handler，agent `send_feedback` 無後端時回「unavailable」錯誤——兩者都與新方向不一致。

## What Changes

- `platform_adapter`：新增純函式 `open_url_argv(url, os)`（windows→`explorer`、macos→`open`、linux→`xdg-open`）+ best-effort `open_url(url)`（對稱於既有 `reveal_argv`/`reveal_in_file_manager`）。
- App **Send Feedback** 選單動作：從空 handler 改為 `open_url(FEEDBACK_ISSUES_URL)`，開啟 GitHub 新 issue 頁面。
- `agent_contract`：新增單一真相來源 `pub const FEEDBACK_ISSUES_URL`（= `https://github.com/TimLai666/fronda/issues/new`），工具與選單共用。
- `cmd_send_feedback`：無 `FeedbackSender` 時，從回 "unavailable" 錯誤改為**成功回傳 GitHub issues 指引**（不動 dedup/cap 狀態，因為沒有實際送出）。seam 保留：若 host 裝了 sender 仍走原路徑。
- 工具數、名稱、schema **不變**（56 工具）；僅 `send_feedback` 的無後端回傳語意更貼近實情。

## Non-Goals

- 不建任何回饋後端（Convex/自有 API）——這正是被此決策取消的方向。
- 不移除 `send_feedback` 工具或 `FeedbackSender` seam（保留向下相容；host 仍可裝 sender）。
- 不改工具描述文字（"to the Fronda team" 仍成立——GitHub issues 即團隊管道；回傳文字已明確導向）。

## Capabilities

### New Capabilities

- `feedback-destination`：把產品回饋導向 Fronda 的 GitHub issues——App 選單開啟頁面、agent 工具回傳該 URL 作為指引，無需後端。

## Impact

- Affected code:
  - Modified: `crates/app_shell_gpui/src/{platform_adapter.rs,app_root.rs}`, `crates/agent_contract/src/{tool_exec.rs,lib.rs}`
- 無 on-disk 契約變更、無新依賴、無工具面數量/名稱變更。`FEEDBACK_ISSUES_URL` 為新公開常數（非破壞）。取代 `send-feedback-tool` 的 task 3.1（Convex 後端取消）。

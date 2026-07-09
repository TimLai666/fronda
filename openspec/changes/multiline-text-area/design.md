## Context

單行 `TextField`（crates/app_shell_gpui/src/text_field.rs）已依 gpui 官方 input 範例完成：`EntityInputHandler`（IME 組字）、自訂 Element 在 paint 註冊 `ElementInputHandler`、`FrondaTextField` key context 的 action bindings、游標/選取/剪貼簿。generation prompt 與 feedback message 是多行語意，仍在 key_char 路徑上（無 IME）。全域單鍵快捷鍵已遷移至 `!input` predicate bindings（crates/app_shell_gpui/src/global_shortcuts.rs），任何文字輸入元素以 key context 含 `input` 標記自身。實作由 worktree agent 平行進行中，本 design 記錄整合時必須成立的契約。

## Goals / Non-Goals

**Goals**
- 多行 TextArea：軟換行、`\n` 硬換行、Enter=插入換行、IME 組字、視覺行 up/down 游標移動、選取、剪貼簿（貼上保留換行）、依內容自動長高（有 max 行數上限）
- generation prompt 與 feedback message 遷移，移除其 key_char 處理
- key context 含 `input`，讓 `!input` 全域快捷鍵 predicate 正確避讓

**Non-Goals**
- 捲動（超過 max 行數後的 viewport 捲動留待後續）
- chat composer 遷移（已在單行 TextField 上，多行顯示是獨立決策）
- undo/redo 於欄位內
- grapheme cluster 邊界（沿用 TextField 的 char 邊界決策，避免新依賴）

## Decisions

1. **文字塑形用 gpui 的多行 wrap API（`shape_text` / `WrappedLine`）而非逐行 `shape_line`**：wrap 寬度取自元素 bounds，硬換行切段後每段交給 wrap；游標與選取幾何經 wrapped-line 的 index↔position 映射。若 gpui-ce 的 API 面與預期不符，退階為「僅硬換行、無軟換行」並在報告中明確標注（不得默默降級）。
2. **up/down 以視覺行移動**：以 wrapped-line 幾何求「上/下一行相同 x 的最近 index」。若幾何 API 不足，退階為硬換行行間移動，同樣必須明確標注。
3. **Enter 綁 `InsertNewline` action（無 Submit）**：多行欄位的 Enter 永遠是換行；submit 由 host view 的按鈕負責。key bindings 若與 `FrondaTextField` 集合不相容則註冊獨立 `FrondaTextArea` context 與 `bind_text_area_keys(cx)`，boot 於 crates/app_shell_gpui/src/main.rs 呼叫。
4. **`Edited` 事件鏡射 host model**：與 TextField 相同模式——host `cx.subscribe` 後把 `text()` 鏡射回 `GenerationState.prompt` / `FeedbackViewModel.message`，維持既有 can_submit 邏輯不變。
5. **key context 對齊**：TextArea 的根 div key context 必須含 `input`（如 "FrondaTextArea input"），與 global_shortcuts 的 `!input` predicate 協作；遷移後 host 容器的暫時性 `key_context("input")` 若已無 key_char 路徑即移除。

## Implementation Contract

- `TextArea::new(cx, placeholder)`、`text()`、`set_text()`（重置選取與 marked range、不發 `Edited`）、`is_empty()`、`TextAreaEvent::Edited`
- 貼上保留 `\n`（TextField 是把 `\n` 換空白——TextArea 不沿用）
- 未綁定且無修飾的按鍵不得吞掉 escape/tab（host 導航），其餘行為與 binding 系統一致
- 驗證 gate：cargo test --workspace EXIT=0、cargo test -p fronda-app-shell-gpui --features desktop-app EXIT=0、cargo check -p fronda-app-shell-gpui --features desktop-app --bin fronda EXIT=0

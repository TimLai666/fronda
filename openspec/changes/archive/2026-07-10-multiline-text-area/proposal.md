## Why

generation panel 的 prompt 與 feedback 表單的 message 欄位仍走 key_char 打字路徑：無法輸入中文（IME 組字不經過 key_char）、無游標移動、無選取與剪貼簿。單行的 `TextField`（crates/app_shell_gpui/src/text_field.rs）已具備完整平台文字輸入，但這兩個欄位語意是多行 textarea，需要多行版元件。

## What Changes

- 新增多行 `TextArea` gpui 元件：軟換行（wrap 至元素寬度）、`\n` 硬換行、Enter 插入換行（非 submit）、IME 組字（marked-text 底線）、游標跨視覺行移動（up/down）、shift 選取、滑鼠選取、剪貼簿（貼上保留換行）、`Edited` 事件、依內容行數自動長高（min/max 行數上限）
- 元件的 key context 必須含 `input` 標記，與 global_shortcuts 的 `!input` predicate 對齊（焦點在欄位內時全域單鍵快捷鍵失效）
- generation panel prompt 遷移至 `TextArea`（經 `Edited` 鏡射回 `GenerationState.prompt`），移除其 key_char 鍵盤處理
- feedback 表單 message 欄位遷移至 `TextArea`（鏡射 `FeedbackViewModel.message`），保留 Tab 聚焦 email `TextField` 的行為，移除其 key_char 鍵盤處理
- 兩個 view 遷移完成後，其根 div 的暫時性 `key_context("input")` 標記與 focused guard 依實際需要收斂（欄位自帶 context 後，容器標記僅在仍有 key_char 路徑時保留）

## Capabilities

### New Capabilities

- `text-area`: 多行文字輸入元件——軟換行、IME 組字、游標與選取、剪貼簿、自動高度，供 gpui view 以 Entity 內嵌並訂閱 Edited 事件

### Modified Capabilities

(none)

## Impact

- Affected specs: text-area（新增）
- Affected code:
  - New: crates/app_shell_gpui/src/text_area.rs
  - Modified: crates/app_shell_gpui/src/lib.rs, crates/app_shell_gpui/src/generation_view.rs, crates/app_shell_gpui/src/feedback_view.rs, crates/app_shell_gpui/src/main.rs
  - Removed: (none)

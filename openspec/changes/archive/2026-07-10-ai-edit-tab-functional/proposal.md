## Why

UI parity audit row 11 驗證確認：AI Edit 分頁的三個 click handler 全是展開切換，action 按鈕（Upscale/Edit/Rerun/Create Video/Music/Sound Effects）沒有任何 dispatch，Upscale 模型清單是硬編碼三行（Topaz/Frame Interpolation/Magnific——非真實目錄）。可接的真工具已存在：upscale_media、generate_music、generate_audio、denoise_audio。

## What Changes

- Upscale 目錄：從上游 9dfde8d^ 的 Fal-era upscale 模型清單（5 個）轉錄進 generation_core::model_catalog（ModelKind::Upscale caps），Upscale picker 改讀目錄
- action_row 增加 on_click 佈線：Upscale → upscale_media（選中資產）、Music → generate_music、Sound Effects → generate_audio（video 參考語意依上游 args）——無後端時顯示工具回傳的明確 unavailable 狀態（generation panel 的 SubmitOutcome 模式）
- Edit/Rerun/Create Video：Rerun 以選中資產的 generation_input 重放（generate_* 同參數）；Edit/Create Video 顯示明確「需要生成後端」狀態（video-to-video 上游也是後端功能）
- selected asset 傳遞：AiEditTabView 增 selected_media_asset_id（inspector selection 觀察已存在——inspector 轉傳）；無選中時 action rows disabled
- 狀態列：每 action 的結果/錯誤顯示（status 欄位）

## Non-Goals

- 真實生成後端（gated）
- AI Enhance 的 replace-source/trimmed-portion 實際套用（依賴後端輸出）

## Capabilities

### New Capabilities

- `ai-edit-tab`: AI Edit 分頁真實 dispatch——真 upscale 目錄、工具佈線、選中資產綁定、誠實的後端 gating 狀態

### Modified Capabilities

(none)

## Impact

- Affected specs: ai-edit-tab（新增）
- Affected code:
  - New: (none)
  - Modified: crates/app_shell_gpui/src/ai_edit_tab_view.rs, crates/generation_core/src/model_catalog.rs, crates/app_shell_gpui/src/inspector_view.rs
  - Removed: (none)

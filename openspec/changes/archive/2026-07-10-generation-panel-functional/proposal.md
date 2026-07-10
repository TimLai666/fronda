## Why

UI parity audit（specs/rust-rewrite/98-ui-parity-audit.md row 1）：generation panel 是視覺殼——模型列硬編碼、gear 設定鈕 no-op、參考圖 tiles 靜態、無成本估算、Generate 按鈕只切換假的 is_generating。這是「看起來對但按了沒反應」的最大單一面。model-catalog-wiring 已把 19 個真實模型接進 generation_core，本 change 讓面板真的用它。

## What Changes

- 模型選單改讀 generation_core::catalog()（依 selected_type 過濾、顯示 display_name、paid gating 標示），取代硬編碼三行
- gear 設定 popover：依模型 caps 顯示 Duration/AspectRatio/Resolution/Quality/Count（video）、Count（image）、Instrumental/Generate-audio（audio）——狀態存 GenerationState，資料來源 ModelCaps
- 成本估算列：generation_core 既有 format_cost 邏輯 × 所選參數；insufficient-credits 態（credits_remaining 比較）
- 參考圖 tiles：接受媒體資產指派（先做點擊選取媒體庫資產的路徑；拖放屬 drag-drop-system change）、顯示縮圖（thumbnail cache 既有）、清除 X、數量上限
- Generate 按鈕真實提交路徑：組 GenerationInput（prompt/model/參數/references）→ 經 GenerationBackend seam 送出（無 backend 時顯示明確不可用狀態，不再假轉圈）；is_generating 綁真實任務狀態
- voice picker（audio 模型的 voices caps）、lyrics/style 欄位（audio）

## Non-Goals

- 拖放進 tiles（drag-drop-system change 的範圍）
- @mention 參考自動完成（獨立小 change）
- edit-video (video-to-video) 源帶（依賴選取系統，後續）
- 真實後端傳輸（GenerationBackend host 實作是 gated 決策）

## Capabilities

### New Capabilities

- `generation-panel`: 功能完整的生成面板——真實模型目錄、caps 驅動的設定、成本估算、參考資產、真實提交路徑

### Modified Capabilities

(none)

## Impact

- Affected specs: generation-panel（新增）
- Affected code:
  - New: (none)
  - Modified: crates/app_shell_gpui/src/generation_view.rs, crates/generation_core/src/model_catalog.rs, crates/app_shell_gpui/src/editor_state_hub.rs
  - Removed: (none)

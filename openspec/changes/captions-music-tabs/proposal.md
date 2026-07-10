## Why

UI parity audit rows 3+4：Captions 與 Music 分頁全是靜態標籤。Swift 的 CaptionTab 有完整控制面（來源/語言/字體/尺寸/顏色/背景/大小寫/髒話過濾、即時預覽框、Agent Mode 選單、Generate 按鈕與轉錄 overlay）；MusicTab 有輸入模式選單、時長、模型選單、prompt 欄、成本與 credit gating。兩者共享媒體面板且性質類似，合為一個 change。

## What Changes

- Captions 分頁：Source/Language 選單（transcription_language 既有欄位）、Font picker（render_core::text 的字族清單）、Size/Color/Background(+toggle)/Case/Censor 控制、caption 預覽框（中心導線 + X/Y placement scrub）、Agent Mode 選單（remove filler/fix names/add emoji/translate 子選單——經 chat 提示或 caption 工具）、Generate 按鈕（words 可用性 gating + 轉錄中 overlay + 錯誤註記）；CaptionConfig（search_core 既有）為狀態載體
- Music 分頁：Input mode 選單（Video↔Text to Music）、Duration scrub（text 模式）、來源範圍摘要（timeline 選取）、Model 選單（generation_core catalog 的 audio/music 條目）、prompt TextField、成本/驗證註記、credit gating、generating overlay、Agent Mode 選單（timeline/mood 子選單）

## Non-Goals

- 真實轉錄執行（TranscriptionProvider host gated；Generate 在無 provider 時顯示明確不可用）
- 音樂生成後端（GenerationBackend gated）

## Capabilities

### New Capabilities

- `captions-tab`: 功能完整的字幕分頁——樣式控制、預覽、生成 gating
- `music-tab`: 功能完整的音樂分頁——模式/模型/提示、成本與 gating

### Modified Capabilities

(none)

## Impact

- Affected specs: captions-tab、music-tab（新增）
- Affected code:
  - New: (none)
  - Modified: crates/app_shell_gpui/src/media_panel_view.rs
  - Removed: (none)

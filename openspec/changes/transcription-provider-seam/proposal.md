## Why

get_transcript、remove_words、字幕產生都吃 timeline words，目前唯一來源是 set_timeline_words 注入邊界（空值時工具回 "No transcribable speech"）——沒有任何路徑真正執行語音轉錄。轉錄模型是 host 依賴（whisper 系或平台 STT），但「對哪些 clip 轉錄、詞級時間戳如何落到 timeline、語言設定如何傳遞」的協調邏輯是純的，可先實作到 seam 邊界。

## What Changes

- `TranscriptionProvider` host seam trait：`transcribe(source, language: Option<&str>) -> Result<Vec<WordStamp>, String>`（詞、起訖秒，source time）
- 純協調邏輯：對 timeline 的音訊承載 clips 逐一轉錄、把 source-time 詞戳映射到 project frames（沿用 silence_detector 的 source_offset_seconds = trim_start_frame/fps 慣例與 speed 換算）、寫入 executor 的 timeline words 儲存
- 語言設定來自 Timeline.transcription_language（#40 已移植）
- transcribe 觸發點：agent 工具（如 transcribe_timeline）或 host UI 按鈕——依上游工具面決定（上游若無此工具則僅 host API，不擴充工具面）
- whisper.cpp / 平台 STT 的 host 實作為 gated

## Non-Goals

- 不選定語音模型與執行期（host 決策）
- 不改 caption 產生邏輯（search_core 既有 phrases_from_words 不動）

## Capabilities

### New Capabilities

- `transcription-seam`: 轉錄提供者 seam 與詞戳到 timeline words 的純映射協調

### Modified Capabilities

(none)

## Impact

- Affected specs: transcription-seam（新增）
- Affected code:
  - New: (none)
  - Modified: crates/agent_contract/src/tool_exec.rs, crates/timeline_core/src/word_cut.rs
  - Removed: (none)

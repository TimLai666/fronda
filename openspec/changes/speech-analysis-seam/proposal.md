## Why

上游 #261 的 remove_silence 以 Silero VAD 偵測語音區段（dead air = 非語音），並做講者識別。Rust 版目前用 RMS 自適應門檻誠實近似（audit 2026-07-05b 記錄），對音樂床、環境音重的素材會誤判。VAD 模型本身是 host 依賴，但「語音區段 → dead-air 反轉」的邏輯是純的，可以先建 seam 讓日後只差接模型。

## What Changes

- `audio_core` 新增純函式：`speech_spans_to_dead_air(spans, clip_duration, min_silence, edge_padding) -> Vec<(f64, f64)>`——把語音區段反轉為可刪除的非語音區段（含邊界 padding 與最短長度過濾）
- `agent_contract` 新增 `SpeechAnalyzer` host seam trait：`analyze(source, sample_rate) -> Option<Vec<SpeechSpan>>`（None = 不可用）；ToolExecutor 掛 optional analyzer
- `detect_track_dead_air` 優先走 analyzer 提供的語音區段（反轉為 dead air），analyzer 缺席或回 None 時退回現行 RMS 自適應路徑（行為不變）
- Silero VAD（或平台語音 API）的 host 實作與講者識別（speakers 註冊表寫入）為 gated，不在本 change 範圍

## Non-Goals

- 不引入 ONNX/模型執行期依賴（host adapter 的獨立決策）
- 不做講者識別與 ProjectFile.speakers 的寫入（透傳已完成，識別是模型工作）

## Capabilities

### New Capabilities

- `speech-analysis-seam`: 語音區段分析的 host seam 與純反轉邏輯，讓 remove_silence 在有 VAD 時精準刪除非語音、無 VAD 時維持 RMS 近似

### Modified Capabilities

(none)

## Impact

- Affected specs: speech-analysis-seam（新增）
- Affected code:
  - New: (none)
  - Modified: crates/audio_core/src/silence_detector.rs, crates/agent_contract/src/tool_exec.rs
  - Removed: (none)

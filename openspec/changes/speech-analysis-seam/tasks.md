## 1. 純邏輯

- [x] 1.1 crates/audio_core/src/silence_detector.rs：新增 speech_spans_to_dead_air(spans: &[(f64,f64)], clip_duration: f64, min_silence_seconds: f64, edge_padding_seconds: f64) -> Vec<(f64,f64)>——先合併重疊/相接的語音區段再取補集，補集各段起點加 padding、終點減 padding，過濾短於 min 的段；單元測試覆蓋表格例（頭尾缺口、短缺口過濾、全語音、全靜音、重疊 spans）

## 2. Seam 與接線

- [x] 2.1 crates/agent_contract/src/tool_exec.rs：SpeechSpan{start_seconds, end_seconds} 與 SpeechAnalyzer trait（analyze(&MediaSource, sample_rate: u32) -> Option<Vec<SpeechSpan>>）；executor 欄位 speech_analyzer: Option<Arc<dyn SpeechAnalyzer>> + setter
- [x] 2.2 detect_track_dead_air：每 clip 先問 analyzer——有 spans 則 speech_spans_to_dead_air 產生 source ranges（再走既有 source_ranges_to_project_frames），None 則走現行 rms_envelope 路徑；mock analyzer 測試兩条路徑與 fallback
- [x] 2.3 cargo test --workspace EXIT=0 驗證；remove_silence 既有測試（單軌、雙軌 sync-locked、無 dead air 錯誤）不得回歸

## 3. Host 模型（gated）

- [ ] 3.1 Silero VAD host adapter（ONNX 執行期或平台語音 API 的選型決策）與講者識別為獨立後續 change；本 change 完成至 seam 邊界

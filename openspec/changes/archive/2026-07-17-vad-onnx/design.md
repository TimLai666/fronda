## Decisions

Silero VAD v4/v5 ONNX（16k 單聲道、固定窗 512 樣本、state 傳遞——以模型實際輸入簽名為準，下載官方 silero-vad ONNX 入 assets/models/）。`vad.rs`：`SileroVad::new(model_bytes)`、`analyze(pcm_16k_mono) -> Vec<SpeechSpan>`（機率→窗標記→合併成 span：閾值/最短語音/最短靜音 padding 依 silero 官方 utils 預設，常數註明）。resample：既有 ffmpeg decode 已可指定 16k。掛載：hub 建 executor 時 `set_speech_analyzer`（feature gated）；`SpeechAnalyzer::analyze` 內 decode（沿用 ClipAudioSource）→ VAD。錯誤（ort session 失敗）→ None＋log。測試：合成正弦/靜音 PCM 過真模型（feature on 時）驗 span 邊界合理；feature off 編譯路徑照常。CI 註記：download-binaries 需網路。

## Implementation Contract

- feature on：語音+靜音合成音訊得到非空 spans 且靜音段不在 span 內；全靜音→空。feature off：workspace 行為不變。
- `cargo test -p fronda-app-shell-gpui --features vad` 綠；default 兩相不回歸。

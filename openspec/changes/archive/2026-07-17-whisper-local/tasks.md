## 1. 實作

- [x] 1.1 依 design 實作 transcribe-local feature（先查明 seam 現形並回報；whisper-rs 建置驗證；模型路徑偏好鍵；映射與掛載）；先紅後綠。驗證：feature 組合綠、default 不回歸、desktop check。→ whisper-rs 0.16.0（whisper-rs-sys 0.15.0 / whisper.cpp 1.8.3，需 cmake——本機經 brew 補裝 4.4.0）；`transcribe.rs` WhisperTranscriber 實作 TranscriptionProvider seam，word 級（token_timestamps + split_on_word + max_len 1），模型路徑每次呼叫重讀 preferences.json `whisperModelPath`（缺→honest Err），context 以路徑為 key 快取；掛載＝editor_state_hub::install_matte_writer 的 vad 前例（#[cfg(feature)]）。驗證：feature on 364 綠（+7）、default 357 綠、desktop 與 desktop+transcribe-local check 綠、clippy 綠；真模型推論測試以 FRONDA_WHISPER_MODEL 為 gate（本機無模型檔，未 runtime 驗證）。
- [x] 1.2 workspace 全綠；AGENTS.md transcription 條目更新。驗證：內容審查。

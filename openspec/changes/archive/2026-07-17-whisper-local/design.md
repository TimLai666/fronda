## Decisions

先讀既有 transcription seam 形（archived change `transcription-seam` 與 search_core 的 word/transcript 形——seam trait 在哪個 crate、簽名為何，以現碼為準）。whisper-rs 鎖最新穩定版；`transcribe.rs`：模型路徑來源＝`preferences.json` 的 `whisperModelPath` 鍵（缺→None→seam 回不可用）；decode 16k mono（沿用 ffmpeg decode 慣例）→ whisper full → segments/word timestamps 映射 seam 的 word 形（whisper 的 token 時間戳 heuristic 以 whisper-rs 的 word-level 支援為準，做不到 word 級就 segment 級並如實回報）。錯誤→log＋不可用。測試：feature on 用極短合成/靜音 wav 驗「跑得動、空結果不 panic」（無模型時 skip——CI 無模型檔）；feature off 不回歸。

## Implementation Contract

- feature off：workspace 行為不變。feature on＋無模型：honest 不可用。掛載點與 seam 對齊現碼。
- `cargo test -p fronda-app-shell-gpui --features transcribe-local` 綠（無模型環境跳過推論測試）；default 兩相不回歸。

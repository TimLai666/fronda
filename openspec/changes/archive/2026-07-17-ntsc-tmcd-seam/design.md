## Decisions

seam 簽名 `timecode_frame_duration(&self, source: &MediaSource) -> Option<(i64, i64)>`（num/den，例 NTSC (1001, 30000)）。`manifest_tc_seconds` 改為可注入精確值的形（呼叫點把 seam 結果傳入或 executor 內查詢——依現行 closure 結構選最小侵入）；優先序：seam 精確值 > DF 1001/1000 推導 > frame/quanta。ffmpeg 端：找 tmcd/data stream（codec id tmcd 或 stream disposition），取 time_base；找不到回 None。無 tmcd fixture 可入庫——ffmpeg 路徑驗 None path＋純換算全測，真實 NTSC 素材手動抽查列 follow-up。

## Implementation Contract

- 換算單元測試：seam (1001,30000) 時 tc 秒 = frame×1001/30000（NDF NTSC 精確）；DF 無 seam 維持 1001/1000；皆無維持 frame/quanta。sync 與 multicam 兩路一致。
- `cargo test -p fronda-agent-contract`、`-p fronda-app-shell-gpui` 全綠。

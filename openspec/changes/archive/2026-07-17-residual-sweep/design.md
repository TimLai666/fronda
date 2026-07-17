## Decisions

(1) ProjectSeams struct 加 transcription provider 欄位（比照既有 audio_source/matte_writer 欄位模式），hub 的 install 點與 navigator open 路徑同步——查現行 ProjectSeams 用法後最小侵入。(2) clippy 逐項：redundant closure/map_or/assign-op/sort_by_key/strip-prefix，零行為變更；有疑慮跳過並列出。(3) overlay Layout chips：沿用 D7 的 layout chips 建構（program 分支），gate 改含 overlay 目標（Swift 語意），e2e 沿用既有 harness。

## Implementation Contract

- 各 crate 測試綠、clippy 警告降至僅依賴噪音；overlay layout e2e 過 executor。兩相全綠、desktop check。

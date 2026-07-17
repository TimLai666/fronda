## 1. 實作

- [x] 1.1 依 design「下限套在 find_sync_offset_windowed 的 retain 階段」實作 min-overlap 下限與 `MIN_OVERLAP_HOPS`，實作 spec「Audio sync correlation enforces a minimum overlap」；先寫紅測試（2 秒訊號 → None；thin-edge lag 上限斷言）再實作；audio_core 與 agent_contract 既有測試訊號依 design「測試訊號加長」調整。驗證：`cargo test -p fronda-audio-core` 與 `cargo test -p fronda-agent-contract` 全綠。
- [x] 1.2 `cargo test --workspace` 全綠；97-upstream-pr-audit.md 的 #269 DEFERRED 列更新（min-overlap floor 已 DONE，其餘引擎強化仍 DEFERRED）；AGENTS.md audit bullet 同步。驗證：內容審查。

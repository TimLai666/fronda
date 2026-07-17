## 1. 實作

- [x] 1.1 依 design 實作 spec「Audio sync uses seeded search with global fallback」：correlator 種子窗（centerLagHops+信心 fallback，語意照 `git show 02cf7acd`）、capture-date 種子接入 cmd_sync_clips（欄位來源讀 Swift 確認）、NTSC frameDuration 缺口查證（等價則回報無事）；先紅後綠。驗證：`cargo test -p fronda-audio-core` 與 `-p fronda-agent-contract` 全綠。
- [x] 1.2 移除 update_text flat 相容鍵（拒絕＋訊息一致），inspector nested 測試不回歸。驗證：`cargo test -p fronda-agent-contract` 與 `-p fronda-app-shell-gpui` 全綠。
- [x] 1.3 `cargo test --workspace` 全綠、desktop check；AGENTS.md #269 列與 text-style follow-up 註記更新。驗證：內容審查。

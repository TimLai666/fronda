## 1. 實作

- [x] 1.1 依 design 實作 spec「generate_audio supports source-based categories per Swift v0.6.10」：schema/描述逐字 `0e53593b`、id_short sourceMediaRef、generation_core catalog/payload 擴充（含 pre-existing sfx/inputs 缺口，回報中標明歸因）、cmd_generate_audio gating 與 list_models 欄位；先 transplant 上游測試意圖（合成 catalog entry）再實作。驗證：`cargo test -p fronda-agent-contract` 與 `cargo test -p fronda-generation-core` 全綠、工具數斷言不動。
- [x] 1.2 `cargo test --workspace` 全綠、desktop check 通過；AGENTS.md porting table #294 列更新（rest 完成、workflow/UI 維持 DEFERRED）。驗證：內容審查。

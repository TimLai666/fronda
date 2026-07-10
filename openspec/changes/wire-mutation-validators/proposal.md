## Problem

upstream-critical-fixes 過程確認：crates/agent_contract/src/mutation.rs 的驗證層（validate_add_clips、validate_set_clip_properties 等 20+ 函式）不在 live path 上——tool_exec.rs 從不呼叫它們。#144 的 volume/opacity 0..1 檢查（porting table 標 DONE）實際 dormant；#264 的 frame ceiling 因此被迫在兩層各實作一次。這是假保護：MCP/agent 呼叫只受 executor 內散落的 ad-hoc 檢查保護。

## Root Cause

mutation.rs 是移植時建立的平行驗證庫，executor 的 cmd_* 各自長出了不完整的行內驗證，兩者從未接線。

## Proposed Solution

決策後擇一：(A) executor dispatch 前統一呼叫對應 validator（單一保護層，刪行內重複）；(B) 承認 executor 行內驗證為權威，把 mutation.rs 缺的檢查（#144 等）搬進 executor 後刪除 mutation.rs。傾向 (A)：validators 已有測試且 schema 對齊，接線點在 execute() 的 dispatch match 前做 tool→validator 映射。無論何者：#144 檢查必須上 live path，並以 e2e 測試（executor.execute 拒絕 volume 1.5）釘住。

## Non-Goals

- 新增驗證規則（僅接線既有的）

## Success Criteria

executor.execute 對每個有 validator 的工具在無效輸入時回 Err（抽樣 e2e 測試至少覆蓋 #144 的 volume/opacity/speed/trim 與 #264 ceiling）；重複的行內檢查移除或指向共用函式；cargo test --workspace EXIT=0

## Impact

- Affected code:
  - Modified: crates/agent_contract/src/tool_exec.rs, crates/agent_contract/src/mutation.rs
  - New: (none)
  - Removed: (none)

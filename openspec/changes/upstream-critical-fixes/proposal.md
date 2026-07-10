## Problem

2026-07-10 全量上游掃描（97-audit 附錄）驗證了兩個存在於 Rust 的資料損壞級 bug 與三個未查證的疑似 bug：(1) #124 覆蓋放置 linked V+A 時只清 video 軌範圍，audio 殘片留下且新 clip 的 audio 被排到多餘軌；(2) #263 內嵌修復——ripple delete 只從 anchor 軌的 clips 傳播 linked partners，被清除的 sync-locked 軌上 clips 的 partners 若在 lock-off 軌上會殘留（desync）；(3) UNVERIFIED：#139 drop-frame timecode 分鐘邊界除數（frame 1800 @29.97DF 應為 00;01;00;02）、#264 frame 參數溢位（i64 加法接近極值）、#212 speed 下限。另有兩個 S 級 port：#36 自訂 Anthropic base URL（現硬編碼）、#268 sonnet5 請求缺 effort 欄位。

## Root Cause

(1) overwrite 清除邏輯不含 linked partner 軌；(2) partner 傳播非 fixpoint、不掃 cleared 軌；(3) 移植時未帶上游後續修復或未加防護。

## Proposed Solution

逐項：TDD 重現 → 修復。#124 於 timeline_core/agent_contract 的 overwrite-place 路徑清 partner 軌同範圍；#263 於 compute_ripple_delete/apply 做跨 cleared 軌的 partner fixpoint；三個 UNVERIFIED 先寫測試查證（frame 1800 DF、i64 極值 args、speed 0.1），有 bug 修無 bug 記錄；#36 AnthropicConfig 加 base_url（env/設定）；#268 request builder 加 effort。

## Non-Goals

- #263 工具面 v2（使用者決策待定）
- multicam 引擎（獨立 change）

## Success Criteria

每項有失敗測試先行後轉綠；#124/#263 的重現測試釘住修復；UNVERIFIED 三項各有查證測試與結論記錄；cargo test --workspace EXIT=0

## Impact

- Affected code:
  - Modified: crates/timeline_core/src/workflow.rs, crates/timeline_core/src/edit.rs, crates/agent_contract/src/tool_exec.rs, crates/agent_contract/src/mutation.rs, crates/render_core/src/xml_export.rs, crates/app_shell_gpui/src/anthropic_transport.rs, crates/agent_contract/src/prompt_caching.rs
  - New: (none)
  - Removed: (none)

## Summary

兩件收尾：(1) #269 引擎剩餘半邊——centerLag/capture-date 種子窗與 NTSC 精確 frameDuration（min-overlap floor 已落地）；(2) update_text 的 4 個 flat 相容鍵移除（inspector 已遷移 nested style，相容鍵可退場，回到 #330 嚴格契約）。

## Non-Goals

- sync UI 與 sync_offset_frames metadata（既有 deferred）

## Impact

- Affected specs: `upstream-v0610-compat`（MODIFIED：sync 種子窗 requirement 補充）
- Affected code: crates/audio_core/src/audio_sync_correlator.rs、crates/agent_contract/src/{tool_exec.rs,mutation.rs,tools.rs}

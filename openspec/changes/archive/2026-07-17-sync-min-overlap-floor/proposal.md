## Summary

移植上游 #269 的 correlator min-overlap 下限（2026-07-17 audit 標記的最高優先 follow-up bug）：Rust `AudioSyncCorrelator` 目前對任何重疊長度的 lag 都給分，1–2 個 RMS frame 的薄邊重疊可以產生完美 Pearson 相關而奪冠，造成假同步結果。Swift pre-#269 就有 16-hop 下限，#269 提高為 `max(16 hops, 3 秒)`。

## Motivation

audit（97-upstream-pr-audit.md 2026-07-17 節 #269 列）判定：「Rust correlator 無 min-overlap 下限（cross_correlate 接受 1-frame 重疊）— 正是 PR 修的 thin-edge 假匹配問題，Rust 比修前 Swift 更脆弱」。這是 sync_clips 工具的正確性 bug，符合「Rust 相關 bug fix 自動移植」規則。#269 引擎半邊的其餘強化（centerLag/capture-date 種子窗、NTSC 精確 frameDuration）維持 DEFERRED。

## Proposed Solution

`find_sync_offset_windowed` 在 peak 搜尋前過濾重疊長度低於 `max(16, round(3.0 * sample_rate / frame_size))` hops 的 lag（對齊 Swift `SyncDefaults.minOverlapSeconds = 3` 與 `AudioSyncCorrelator.minOverlap = 16`）。`cross_correlate` 本體不動（保持純粹、既有直接呼叫者不受影響）。訊號短到無法產生達標重疊時回 None（誠實失敗，Swift 同）。既有測試訊號（1–2 秒）加長至 8 秒等級以通過下限，e2e mock 音訊同步加長。

## Non-Goals

- #269 引擎其餘強化（種子窗、NTSC frameDuration）— 維持 DEFERRED
- sync UI 與 sync_offset_frames metadata（既有 deferred 項）

## Impact

- Affected specs: `upstream-v0610-compat`（MODIFIED：補一條 requirement）
- Affected code:
  - Modified: crates/audio_core/src/audio_sync_correlator.rs（下限 + 測試）
  - Modified: crates/agent_contract/src/tool_exec.rs（sync e2e mock 音訊加長）

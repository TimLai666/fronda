## Context

`AudioSyncCorrelator::cross_correlate` 對 lag 範圍 `-(m-1)..=(n-1)` 全部計分；重疊長度 len=1 時 Pearson 為 0（norm 0），但 len=2 起即可 ±1.0 滿分——薄邊 lag 可勝過真實對齊的中段 lag。Swift `02cf7acd`：`guard n >= minOverlapHops else { continue }`，`minOverlapHops = max(16, Int((3.0 / hop).rounded()))`。

## Goals / Non-Goals

**Goals:** find_sync_offset 系列不再讓重疊低於下限的 lag 參與 peak；下限值與 Swift 相同。
**Non-Goals:** cross_correlate 簽名/行為變更；#269 其餘引擎強化。

## Decisions

### 下限套在 find_sync_offset_windowed 的 retain 階段

`cross_correlate` 維持原樣（純函式、多處直接測試）。`find_sync_offset_windowed` 在 `find_peak` 前以 lag 幾何計算重疊長度 `len = min(m - max(0,-lag), n - max(0,lag))`，retain `len >= min_overlap_hops`；`min_overlap_hops = MIN_OVERLAP_HOPS(16).max((3.0 * sample_rate / frame_size as f64).round() as usize)`（等價 Swift `3.0 / hop`）。全部 lag 被濾掉 → find_peak None → 回 None。

### 測試訊號加長

audio_core 既有 find_sync_offset 測試（1–2 秒）與 agent_contract 的 `MockSyncAudio`/`MockClickAudio` 等 sync e2e mock（1 秒）加長到 ≥8 秒；位移樣本數不變（斷言的 offset 不變）。若 cmd_sync_clips 依 clip duration 窗切 PCM，manifest duration 同步加長並換算受影響斷言。

## Implementation Contract

- 兩段各 2 秒的訊號（最大重疊 < 3s）→ find_sync_offset 回 None（新回歸測試，對舊碼紅）。
- 8 秒 noise 對（已知位移）→ 偏移斷言與現行相同（下限不破壞正常同步）。
- 回傳的 `peak_lag_frames` 保證 `|lag| <= hops - min_overlap_hops`（薄邊 lag 不可能奪冠）。
- `cargo test -p fronda-audio-core`、`cargo test -p fronda-agent-contract` 全綠。

## Risks / Trade-offs

- [短素材（<3s 重疊）從此無法 audio-sync] → Swift 相同行為；sync_clips 對 None 已有誠實錯誤路徑。

## Migration Plan

無資料變更；revert 即回滾。

## Open Questions

（無）

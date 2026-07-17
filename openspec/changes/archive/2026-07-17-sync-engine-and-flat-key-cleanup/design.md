## Context

上游 `02cf7cd`(#269)：correlator 支援 centerLagHops 種子窗（先窄搜、信心不足再全域）、capture-date 種子（manifest 拍攝時間差 → 初始 lag 猜測）、NTSC frameDuration 用精確有理數。Rust：min-overlap 已入，find_sync_offset_windowed 無種子窗；tc_seconds=frame/quanta 已是有理式（NTSC 影響點在 cmd_sync_clips 的 timecode 換算與 correlator 的 frame 換算——實作時讀 Swift 確認缺口實際位置，audit 說 Rust 是「更精確形」，若已等價則如實回報無事可做）。flat 鍵：tool_exec cmd_update_text 保留 fontName/fontSize/color/alignment（nested 優先）。

## Goals / Non-Goals

**Goals:** 種子窗語意 parity（信心門檻/fallback 順序照 Swift）；NTSC 缺口查證後補齊或記錄等價；update_text 拒絕全部 flat 鍵（錯誤訊息與其他 flat 鍵一致）。**Non-Goals:** 見 proposal。

## Implementation Contract

- 種子窗：合成訊號測試（種子命中快路徑、種子錯誤時 fallback 全域仍正確）；capture-date 種子從 manifest source_timecode/拍攝欄位（讀 Swift 確認欄位來源）。
- flat 鍵：update_text 帶 fontName/fontSize/color/alignment → 驗證錯誤；inspector 既有 nested 測試不回歸。
- `cargo test -p fronda-audio-core`、`-p fronda-agent-contract`、`-p fronda-app-shell-gpui` 全綠。

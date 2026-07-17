## Context

上游 `0e53593b`。Rust：cmd_generate_audio 只認 videoSource 分支＋honest backend error（#288 span 驗證已在）；model_catalog AudioCategory 只有 Tts|Music、AudioCaps 缺 inputs/target_languages；payload 缺 source_url/target_language；SCALAR_ID_KEYS 缺 sourceMediaRef。audit 覆核註記：list_models 的 inputs 是 pre-#294 既有鍵（Rust 缺是 pre-existing gap，一併補但不歸因 #294）；AudioCaps minSeconds/maxSeconds 亦 pre-existing。

## Goals / Non-Goals

**Goals:** schema/gating/輸出欄位逐字對齊上游；合成 catalog entry 測試 cleanup/dubbing 路徑；backend 缺席 honest error 不變。**Non-Goals:** 見 proposal。

## Decisions

- schema：generate_audio ＋ sourceMediaRef/targetLanguage，描述逐字 0e53593b；id_short SCALAR_ID_KEYS ＋ sourceMediaRef。
- catalog：AudioCategory ＋ Cleanup/Dubbing（rawValue 對齊 Swift）＋既有 sfx 缺口一併補；AudioCaps ＋ inputs/target_languages/default_target_language；acceptsSource/usesSourceURL/validate 語意 parity（text-input-only prompt 檢查）。payload ＋ source_url/target_language。
- gating（cmd_generate_audio）：cleanup/dubbing 需 sourceMediaRef；來源為 video 且 has_audio==Some(false) → 拒絕（訊息逐字）；usesSourceURL 類別 duration=來源 entry.duration；dubbing 的 targetLanguage 預設/驗證照 Swift；靜態 catalog 無 cleanup/dubbing entry → 測試注入合成 entry（catalog 建構已有注入慣例則沿用，否則加 test-only 建構子）。
- list_models audio 條目 ＋ inputs/minSeconds/maxSeconds/targetLanguages（僅在 caps 有值時輸出，鏡射 Swift emission）。

## Implementation Contract

- e2e：dubbing 合成 entry ＋ sourceMediaRef → payload 含 source_url/target_language、duration=來源；靜音影片來源 → 逐字錯誤；cleanup 無 source → 錯誤；backend 缺席 → 既有 honest error；list_models 新欄位形狀；sourceMediaRef 短 id 展開。
- `cargo test -p fronda-agent-contract` 與 `-p fronda-generation-core` 全綠；工具數不變。

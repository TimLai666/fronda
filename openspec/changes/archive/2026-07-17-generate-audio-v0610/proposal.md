## Summary

完成 #294 的契約層全量（on-disk targetLanguage 已落地）：generate_audio 工具 schema（sourceMediaRef 通用來源、targetLanguage）、cleanup/dubbing 類別 gating（acceptsSource/usesSourceURL、靜音影片拒絕、duration=來源長度）、list_models audio 條目補 inputs/minSeconds/maxSeconds/targetLanguages、id_short 收 sourceMediaRef、generation_core catalog/payload 擴充。backend 缺席時維持既有 honest-error（#251/#288 判例）；cleanup/dubbing model entries 屬伺服器端 catalog，Rust 靜態 catalog 中 dormant，測試用合成 entry。

## Motivation

2026-07-17 audit：契約層 Rust 全缺且可移植；audit 覆核確認 SKIP/DEFERRED 不成立（工具契約是 preserved compat surface）。

## Non-Goals

- 實際 elevenlabs 工作流／上傳／generation panel UI（遠端 backend 與 catalog 缺席，維持 DEFERRED）
- AVFoundation 音軌抽取前處理（Swift-only）

## Impact

- Affected specs: `upstream-v0610-compat`（ADDED：generate_audio source/dubbing 契約 requirement）
- Affected code: crates/agent_contract/src/{tools.rs,tool_exec.rs,id_short.rs,mutation.rs}；crates/generation_core/src/{model_catalog.rs,generation_payload.rs}

## Summary

決策 D6 執行。無法由程式碼側選定商業關係（palmier Convex 需帳號/授權；付費 vendor 需信用憑證），故採唯一可由程式碼完成的忠實路徑：**可設定端點的通用 HTTP 生成後端**——把 GenerationBackend seam 從「僅 recovery」擴為「submit + poll」，generate_video/image/audio 在有安裝後端時真的送出請求並建立 in-flight 資產（複用 #216 recovery 機制完成），無後端時維持既有 honest error（零回歸）。附一份可自建的「Fronda Generation Protocol v1」規格與一個 reqwest 具體 adapter（endpoint/token 取自 env/prefs），沿用 anthropic_transport 既有模式（trait＋mock 單元測試；live 需設定端點,不自動測）。

## Motivation

三個 generate 工具目前一律回 honest error（"requires a remote API"）。#294 已把 generate_audio 的驗證/gating 全部備妥,只差送出。#216 已備妥 recovery（plan_generation_recovery/apply_generation_outcome/GenerationOutcome）。D6 缺的就是 submit seam 與具體 transport。BYO-endpoint 讓「自建相容服務」成為設定變更而非程式碼變更,且對沒有端點的使用者完全無感。

## Non-Goals

- 選定/接上任何特定付費 vendor 或 palmier Convex（商業決策,非程式碼可定）
- generation panel 的即時進度 UI（既有 deferral；本次只到工具契約＋in-flight 資產）
- 串流/webhook 回呼（poll 即可；host 的 recovery tick 完成資產）

## Impact

- Affected specs: 新 `specs/rust-rewrite/98-generation-protocol.md`（Protocol v1）；`upstream-v0610-compat` 無關
- Affected code:
  - crates/generation_core（GenerationRequest/GenerationSubmission 純型別）
  - crates/agent_contract/src/tool_exec.rs（GenerationBackend::submit、cmd_generate_* 送出＋建 generating 資產、mock 擴充）
  - crates/app_shell_gpui（http_generation_backend.rs 新、hub 安裝、prefs/env config）

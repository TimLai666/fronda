## Summary

第二個真實 image provider:Pollinations(免 key)。`GET {base}/prompt/{url-encoded prompt}` 回真 AI 生成 JPEG,無需認證。加進 gateway provider registry(kind=Image),證明(1)多 provider 每 kind 切換、(2)**真 AI 媒體端到端回得來——且免任何憑證可自驗**(補上 Gemini 留給使用者 key 的驗證缺口)。

## Motivation

phase 2 的 Gemini image 需使用者 Google key 才能驗真輸出;Pollinations 免 key,讓「真 AI 圖經 gateway 存/serve/抓回」可由我完整實測。同時第二個真實 vendor 驗證 registry 的多 provider 切換(架構核心主張)。

## 誠實邊界

- Pollinations 是**示範/測試用免 key provider**(第三方免費服務,可用性非我方保證);base URL 可設,要換服務是改設定。
- 預設 image provider 維持 `stub`(離線、確定性);pollinations 為 opt-in(`provider:"pollinations"`),與 gemini 一致。

## Non-Goals

- Pollinations 進階參數全表(先 prompt + 可選 width/height/seed/model)
- 改預設 provider

## Impact

- Affected code: crates/generation_gateway(pollinations provider + 註冊 + tests)
- Affected specs: 98-generation-protocol.md 參考 gateway 段補列 pollinations

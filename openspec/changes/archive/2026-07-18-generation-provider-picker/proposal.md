## Summary

Fronda phase 2b:provider/model picker。`GenerationRequest` 加 `provider: Option<String>`,build_submit_body 帶上;generate 工具接受並透傳 provider;client 加 `fetch_providers()` 讀 `GET /v1/providers` catalog;generation panel 加 provider + model 下拉(讀 catalog,選定寫入送出的 request)。可對 stub catalog 開發驗證,Gemini 落地後自動出現在清單。

## Non-Goals

- gateway 側真 provider——change `gemini-image-provider`(並行)
- catalog 快取失效/輪詢——先每次開 panel 抓一次

## Impact

- Affected code: crates/generation_core(GenerationRequest.provider)、crates/agent_contract(generate 工具 provider 透傳、build 端)、crates/app_shell_gpui(http_generation_backend fetch_providers、generation_view picker)
- Affected specs: 無 delta(protocol v1.1 已定義 provider/catalog)

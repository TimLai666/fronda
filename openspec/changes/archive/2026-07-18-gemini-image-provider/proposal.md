## Summary

Gateway phase 2a:第一個真實 provider(Gemini image)+ 生成結果服務。gateway 新增結果儲存 + `GET /v1/results/{id}`(serve 真實媒體位元組),讓 resultUrl 是可抓取的真 URL(取代 stub://)。`GeminiImageProvider` 用 Google Gemini `generateContent` REST(BYO key,model/base 可設)產生真圖 → 存 → serve。mock-Gemini 測試 key-free 證明整條「生成→存→serve→抓」管線回真位元組;真 Gemini(真圖)由 gated live 測試 + 使用者 key 驗證。

## Motivation

使用者選 phase 2:接第一個真實 provider 驗證真媒體回得來。stub 只回假 URL;此 change 讓 gateway 真的產出並 serve 可抓取的媒體,並接上第一個真 AI provider。

## 誠實邊界

- gateway 是 **server daemon**,其 config(BYO key、model)用 env/config 檔——非 Fronda GUI 一部分,server 用 env 是慣例。
- **真 Gemini 呼叫需使用者的 Google API key**(BYO)——我不能開帳號/刷卡。故:adapter 完整建好、mock-Gemini 全測、gated live 測試就緒;真圖回傳由使用者設 key 後驗。
- **媒體管線本身 key-free 驗到底**:mock Gemini 回真 PNG 位元組 → gateway 存 → `/v1/results` serve → 抓回同位元組(整合測試證明)。

## Non-Goals

- Gemini video(Veo,async operation poll)/audio——後續;此 change 只做 image。
- Fronda 端 picker——change `generation-provider-picker`(並行)。

## Impact

- Affected code: crates/generation_gateway(result store + serve endpoint、gemini provider、註冊、tests)
- Affected specs: 98-generation-protocol.md(加 `/v1/results/{id}` + provider 註記)

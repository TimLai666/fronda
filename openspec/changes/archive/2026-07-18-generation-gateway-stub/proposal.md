## Summary

自建一個相容 Fronda Generation Protocol 的**生成閘道服務**（新 workspace crate `crates/generation_gateway`,binary `fronda-gen-gateway`）。前端說 Protocol v1（Fronda 的 HttpGenerationBackend 直接指過來),後端以 provider registry 對每種功能(video/image/audio)路由到多個 provider,每個 provider bring-your-own-key。**Phase 1 先做本地 stub**:stub providers 讓整條 Fronda→gateway→result 迴圈在零外部金鑰下端到端跑通(順帶第一次真正驗證 D6 client 的 live round-trip)。Gemini 等真實 provider adapter 走同一 trait,列為 phase 2。

## Motivation

D6 已把 Fronda 側做到「指向任何相容端點只需兩個環境變數」。使用者選擇自建相容服務、多 provider、BYO key。此 change 建那個端點,先以 stub 打通全鏈路與 provider 架構,真實 provider 後續逐一接。

## 決策與假設(顯式,可調)

- **在 fronda repo 內新 crate**(非獨立 repo):共用/鏡射 Protocol 型別、與 Fronda 一起版本化與本地測試;日後要拆成獨立部署很便宜。
- **HTTP 框架 axum + tokio**(tokio 生態標準,async):新依賴**侷限在這個新 crate**,不碰跨平台的 gpui/core 等既有 crate。
- **多 provider 選擇**:request 加 optional `provider`;gateway 依 (kind, provider) 路由,provider 缺省時用該 kind 的預設;新增 `GET /v1/providers` catalog 讓 Fronda 之後能填 picker。此為 Protocol v1.1(加性,不破 v1)。
- **Phase 1 只出 stub providers**;真實 provider(Gemini/fal/Replicate/ElevenLabs…)= phase 2,各是一個 provider adapter + 一組 BYO-key 設定。

## Non-Goals

- 真實 provider adapter(Gemini 等)——phase 2
- Fronda 端 model/provider picker UI——phase 2(catalog 端點先備好)
- 部署/容器化/雲端——本地跑先

## Impact

- Affected specs: `specs/rust-rewrite/98-generation-protocol.md` → v1.1(加 provider + catalog)
- Affected code:
  - New: crates/generation_gateway(整個 crate:server、registry、stub providers、config、tests)
  - Modified: Cargo.toml(workspace members)

## 1. 實作

- [x] 1.1 依 design「結果儲存 + serve」加 ResultStore + `GET /v1/results/{id}`,實作 spec「Gateway serves generated media and hosts a real image provider」的 serve 半;JobStore result_urls 指向 serve URL。單元+整合(put/get/404/fetch bytes)。驗證：`cargo test -p fronda-gen-gateway` 綠。
- [x] 1.2 依 design「GeminiImageProvider」實作 gemini image adapter(generateContent、inlineData 解析、tokio::spawn、有 key 才註冊),mock-Gemini 整合測試(key-free 回真 bytes)+ gated live(FRONDA_GEMINI_API_KEY)。驗證：`cargo test -p fronda-gen-gateway` 全綠、`cargo build -p fronda-gen-gateway`。

## 2. 收尾

- [x] 2.1 98-generation-protocol.md 加 `/v1/results/{id}` + provider 註記;`cargo test --workspace` 全綠。驗證：內容審查。

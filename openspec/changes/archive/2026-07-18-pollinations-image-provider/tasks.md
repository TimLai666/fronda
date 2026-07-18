## 1. 實作

- [x] 1.1 依 design「PollinationsImageProvider（鏡射 GeminiImageProvider 結構）」實作免 key image provider(prompt url-encode、可選 query、response bytes→ResultStore、一律註冊),實作 spec「No-key image provider (Pollinations)」;單元 + mock 整合(key-free,byte-equal)+ live-gated(FRONDA_GEN_LIVE_POLLINATIONS)。驗證：`cargo test -p fronda-gen-gateway` 全綠、`cargo build -p fronda-gen-gateway`。

## 2. 收尾

- [x] 2.1 98-generation-protocol.md 參考 gateway 段列 pollinations;`cargo test --workspace` 全綠。維護者實跑一次 live pollinations 確認真圖(記錄於 VERIFY)。驗證：內容審查 + live 筆記。

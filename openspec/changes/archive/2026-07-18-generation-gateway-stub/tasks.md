## 1. Gateway crate

- [x] 1.1 依 design「crate 結構」建 `crates/generation_gateway`(protocol/provider/registry/stub/jobs/server/config/main),實作 spec「Self-hosted generation gateway with pluggable providers」;Cargo.toml 加 workspace member;新依賴只在此 crate。單元+整合測試(registry 路由、bearer、stub 狀態機、完整 HTTP 迴圈、/v1/providers、401)。TDD 先紅後綠。驗證：`cargo test -p fronda-gen-gateway` 全綠、`cargo build -p fronda-gen-gateway` 出 binary。

## 2. Protocol 與端到端

- [x] 2.1 依 design「protocol v1.1（加性）」把 `specs/rust-rewrite/98-generation-protocol.md` 升 v1.1(加 provider 欄位 + `GET /v1/providers` catalog,註明 v1 相容);手動端到端驗證(起 gateway、設兩個環境變數、Fronda generate→generating→poll→completed 帶 stub url),記錄於 VERIFY 筆記。驗證：內容審查 + 手動驗證筆記。

## 3. 收尾

- [x] 3.1 `cargo test --workspace` 全綠、desktop check 通過;AGENTS.md 記 gateway 存在與 phase 2(真 provider)待辦;99-decisions D6 補「gateway 自建路線 phase 1 stub 完成」。驗證：內容審查。

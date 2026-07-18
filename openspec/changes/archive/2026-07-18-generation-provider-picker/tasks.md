## 1. plumbing

- [x] 1.1 依 design「provider 透傳（generation_core + agent_contract）」,generation_core GenerationRequest 加 provider + build_submit_body 帶上 + generate 工具透傳,實作 spec「Fronda selects a generation provider and model」的透傳半。先紅後綠。驗證：`cargo test -p fronda-generation-core -p fronda-agent-contract` 全綠、工具數斷言不動。
- [x] 1.2 依 design「catalog fetch（client）」,HttpGenerationBackend 加 fetch_providers + 純解析測試。驗證：`cargo test -p fronda-app-shell-gpui` 綠。

## 2. picker UI

- [x] 2.1 依 design「picker UI（generation_view）」,generation_view 加 provider/model 下拉(讀 catalog、選定寫 request、無端點不回歸),結構測試。驗證：兩相綠、desktop check。

## 3. 收尾

- [x] 3.1 `cargo test --workspace` 全綠;AGENTS.md/98-spec 註記 picker 就位。驗證：內容審查。

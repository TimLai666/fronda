## 1. Seam 與工具（agent_contract + generation_core）

- [x] 1.1 [P] 依 design「seam 擴充（agent_contract + generation_core）」,generation_core GenerationRequest/GenerationSubmission 純型別（serde）＋GenerationBackend::submit 擴充＋MockGenerationBackend 補 submit＋單元測試。驗證：`cargo test -p fronda-generation-core -p fronda-agent-contract` 綠。
- [x] 1.2 依 design「cmd_generate_* 送出（複用 #216,不偽造成功）」,實作 spec「Generation tools submit through a configurable backend」;cmd_generate_video/image/audio 有後端時 submit＋建 generating 資產、無後端維持 honest error;e2e（mock 送出後 manifest 多 entry、plan_generation_recovery 找得到、resume_job Success 後落 URLs;無後端 honest error 不回歸）。驗證：`cargo test -p fronda-agent-contract` 綠、工具數斷言不動。

## 2. Adapter 與接線（app_shell）

- [x] 2.1 依 design「http adapter（app_shell）+ protocol v1」,實作 spec「Configurable HTTP generation backend」;http_generation_backend.rs（reqwest blocking rustls、from_config env/prefs、submit/poll、request/response 純函式＋fixture 測試）＋hub 安裝＋specs/rust-rewrite/98-generation-protocol.md。驗證：`cargo test -p fronda-app-shell-gpui` 兩相綠、desktop check。

## 3. 收尾

- [x] 3.1 workspace 全綠、desktop check;AGENTS.md porting table/#294 條目更新（generation submit 已通,vendor 仍 D6-external）;99-decisions D6 標 EXECUTED（BYO-endpoint）。驗證：內容審查。

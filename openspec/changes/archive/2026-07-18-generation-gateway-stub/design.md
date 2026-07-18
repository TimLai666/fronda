## Context

Protocol v1（`specs/rust-rewrite/98-generation-protocol.md`）:`POST /v1/generate`(body {kind,model,prompt,durationSeconds?,sourceUrl?,targetLanguage?,params?} → {jobId,status})、`GET /v1/jobs/{id}`(→ {status,resultUrls?,error?})、bearer auth。Fronda 的 `HttpGenerationBackend` 是 client。此 change 建 server 端。

## Decisions

### Crate 結構

`crates/generation_gateway`(binary `fronda-gen-gateway` + lib):
- `protocol.rs`:Protocol v1.1 的 request/response 型別(serde,鏡射 spec;與 Fronda 端結構相容)。
- `provider.rs`:`trait GenerationProvider: Send + Sync { fn name(&self)->&str; fn kind(&self)->ProviderKind; fn submit(&self, req:&GenerateRequest)->Result<ProviderJob,String>; fn poll(&self, job_id:&str)->Result<ProviderStatus,String> }`;`ProviderKind` = Video|Image|Audio;`ProviderStatus` = Queued|Running|Succeeded{urls}|Failed{reason}。
- `registry.rs`:`ProviderRegistry`(HashMap<(kind,name), Arc<dyn GenerationProvider>> + 每 kind 預設 provider name);`route(kind, provider:Option<&str>)` 解析 → provider 或明確錯誤(未知 provider / 該 kind 無 provider)。
- `stub.rs`:`StubProvider{kind}`——submit 產生 job id、poll 依內部 job store 由 queued→succeeded 回一個 placeholder result url(如 `stub://{kind}/{jobid}.bin` 或可設定的 base);模擬 async:第一次 poll running、第二次 succeeded(讓 client 的 poll 迴圈真的跑到)。BYO-key 欄位保留但 stub 忽略。
- `jobs.rs`:in-memory job store(Arc<Mutex<HashMap<jobid, JobRecord>>>;JobRecord 記 provider、狀態、poll 次數、result)。
- `server.rs`:axum router——`POST /v1/generate`、`GET /v1/jobs/{id}`、`GET /v1/providers`;bearer middleware(比對設定 token,常數時間比較,401);handler 委派 registry+store。
- `config.rs`:`GatewayConfig`(bind addr、auth token、每 provider 的 BYO-key map、每 kind 預設 provider);從 env(`FRONDA_GEN_GATEWAY_ADDR`/`_TOKEN`,provider keys 如 `FRONDA_GEN_KEY_<PROVIDER>`)或 config 檔;stub 模式即使無 key 也能起。
- `main.rs`:讀 config、建 registry(註冊 stub providers)、起 axum server。

### Protocol v1.1(加性)

- generate request 加 optional `provider`(字串);缺省 → 該 kind 預設。
- 新 `GET /v1/providers` → `{"video":[{"name":"stub","models":["stub-video"]}], "image":[…], "audio":[…]}`(bearer)。讓 Fronda 之後能列 provider/model。
- v1 client(無 provider 欄位)完全相容:缺 provider 走預設。

### 測試

- 單元:registry 路由(命中/未知 provider/該 kind 無 provider)、bearer 中介(對/錯/缺 token)、stub 狀態機(submit→poll running→poll succeeded)、request/response serde。
- 整合:`tokio::test` 起 server 於隨機 port,用 reqwest 跑完整 submit→poll(running)→poll(succeeded) 迴圈 + `/v1/providers` + 401 路徑。這證明 Protocol v1.1 server 正確。
- 端到端(Fronda client → gateway):文件化手動驗證(起 gateway、設 `FRONDA_GENERATION_URL=http://127.0.0.1:<port>` + `FRONDA_GENERATION_TOKEN`,Fronda 內 generate → generating 資產 → recovery poll → completed 帶 stub url)。這關掉 D6 client 的 live-round-trip 缺口。

## Implementation Contract

- `cargo test -p fronda-gen-gateway` 全綠;`cargo build -p fronda-gen-gateway` 產出 binary。
- gateway 整合測試證明:generate 回 jobId、poll 由 running 到 succeeded 帶 result url、未知 provider/錯 token 明確錯誤、`/v1/providers` 列 stub。
- `cargo test --workspace` 全綠(新 crate 不破既有);既有 fronda-app-shell-gpui 的 D6 測試不動。
- 新依賴(axum/tokio/reqwest-dev)只在新 crate 的 Cargo.toml。

## Risks / Trade-offs

- [新增 axum/tokio 依賴] → 侷限新 crate,不影響跨平台 UI/core crate 的編譯與體積。
- [Protocol v1.1 加 provider] → 加性、v1 client 相容;spec 明記版本。
- [stub 的 placeholder url 非真媒體] → phase 1 的重點是打通鏈路與架構;真 provider phase 2 回真 url。

## Context

gateway 現有 GenerationProvider trait(sync submit/poll)、ProviderRegistry、JobStore、stub providers、axum server。Fronda 的 media 層抓 resultUrls(#135 cachedRemoteURL 路徑)——gateway 需 serve 真 URL。

## Decisions

### 結果儲存 + serve

- `results.rs`:`ResultStore`(Arc<Mutex<HashMap<id,(bytes,content_type)>>>);put→id、get→(bytes,ct)。
- `GET /v1/results/{id}`(bearer):回 bytes + `Content-Type`;缺→404。JobStore 的 succeeded result_urls 指向 `{public_base}/v1/results/{id}`(public_base 從 config,預設 bind addr)。

### GeminiImageProvider

- 實作 GenerationProvider(kind=Image);config:api_key(BYO)、model(預設 `gemini-2.5-flash-image`)、base(預設 `https://generativelanguage.googleapis.com`)、api_version(預設 `v1beta`)。
- submit:`tokio::spawn`(axum handler 在 runtime 內)一個 task 做 HTTP,submit 立即回 job(Running);task:POST `{base}/{ver}/models/{model}:generateContent`,header `x-goog-api-key`,body `{"contents":[{"parts":[{"text":prompt}]}],"generationConfig":{"responseModalities":["TEXT","IMAGE"]}}`;response 200 取 `candidates[0].content.parts[]` 第一個有 `inlineData` 的 → base64 decode `data`、`mimeType` 作 content-type → ResultStore.put → job succeeded 帶 `/v1/results/{id}`。非 200 或無 image part(safety block/only text)→ job failed 帶 reason。poll 讀 JobStore。
- 註冊:有 key 才註冊 gemini(image kind);無 key 只有 stub。catalog 反映實際註冊者。

### 測試

- 單元:generateContent body 建構、response inlineData 解析(有/無 image part/非 200)、ResultStore put/get。
- 整合(key-free,mock Gemini):起一個 fake-gemini axum(回固定 inlineData 真 PNG bytes),GeminiImageProvider 的 base 指過去 → 起 gateway → submit(kind=image, provider=gemini)→ poll running→succeeded → 抓 result URL 回同 PNG bytes。**證明整條管線回真位元組,零外部 key**。
- gated live(真 Gemini):`FRONDA_GEMINI_API_KEY` 設了才跑,真呼叫真圖,斷言回非空 image bytes;未設跳過。

## Implementation Contract

- `/v1/results/{id}` serve 真 bytes + content-type;404 缺失。
- mock-Gemini 整合:submit→poll→succeeded→fetch 回原 PNG bytes(byte-equal)。
- 無 key:gemini 不註冊,catalog image 只列 stub,既有 stub 迴圈不回歸。
- `cargo test -p fronda-gen-gateway` 全綠(mock,不需網路/key);gated live 就緒。

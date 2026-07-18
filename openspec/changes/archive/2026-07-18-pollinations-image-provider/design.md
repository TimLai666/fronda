## Context

gateway 已有 GenerationProvider trait、ResultStore、GeminiImageProvider(參考結構)、capability-URL 結果服務。Pollinations:`GET {base}/prompt/{url-encoded prompt}`(base 預設 `https://image.pollinations.ai`),可選 query `width`/`height`/`seed`/`model`/`nologo`;回 200 + `image/jpeg` binary。無 auth。

## Decisions

### PollinationsImageProvider(鏡射 GeminiImageProvider 結構)

- config `PollinationsConfig{base}`(預設 `https://image.pollinations.ai`,可設);**無 key,永遠可註冊**。
- submit:`tokio::spawn` → GET `{base}/prompt/{urlencode(prompt)}`(可選 params→query:width/height/seed/model,從 request.params 取,缺則省略);200 → 取 response `Content-Type`(預設 image/jpeg)+ binary body → `ResultStore.put(bytes, content_type)` → job Succeeded 帶 `{public_base}/v1/results/{id}`。非 200 → Failed 帶 status+snippet。poll 讀 JobStore。
- 註冊:app_state 一律註冊 pollinations(image),與 stub 並存;gemini 有 key 才加。catalog image 列 stub + pollinations(+gemini if key)。

### 測試

- 單元:prompt url-encode、params→query 建構、response 處理(200 bytes / 非 200)。
- 整合 key-free(mock):fake-pollinations axum server 在 `/prompt/{...}` 回固定 JPEG bytes;provider base 指過去 → gateway submit(provider:"pollinations")→ poll→succeeded → 抓 result URL 回 byte-equal JPEG。
- **live(network-gated,免 key)**:`FRONDA_GEN_LIVE_POLLINATIONS=1` 設了才真呼叫真 Pollinations,斷言回非空 image/* bytes(數 KB);未設跳過。這條由維護者實跑一次確認真 AI 圖回得來。

## Implementation Contract

- mock 整合:submit(provider:"pollinations")→poll→succeeded→fetch 回原 JPEG bytes(byte-equal),零外部呼叫。
- pollinations 一律在 image catalog;stub 迴圈與 gemini(有 key)不回歸。
- `cargo test -p fronda-gen-gateway` 全綠(mock,不需網路);live-gated 就緒。

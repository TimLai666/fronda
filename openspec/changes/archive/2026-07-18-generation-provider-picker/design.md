## Context

Protocol v1.1 已定義 request 的 optional `provider` 與 `GET /v1/providers`。gateway 端已接受 provider。Fronda 端目前不送 provider、無 catalog 讀取、無 picker。

## Decisions

### provider 透傳(generation_core + agent_contract)

- `GenerationRequest` 加 `provider: Option<String>`;`build_submit_body`(http_generation_backend)在 provider 有值時加 `"provider"` 欄位(缺省省略,gateway 走預設)。
- generate 工具(cmd_generate_video/image/audio)接受 optional `provider` 參數 → 透傳進 GenerationRequest;schema/描述加 provider(選填,未指定用預設)。無 provider 行為不變。

### catalog fetch(client)

- `HttpGenerationBackend` 加 `fetch_providers() -> Result<ProvidersCatalog, String>`(GET `/v1/providers`,bearer,解析 `{video:[{name,models}],...}`);純解析函式可測。GenerationBackend seam 是否加此方法:catalog 讀取是 UI 便利,不走 recovery seam;放 HttpGenerationBackend 具體型別即可(app 直接用),不擴 trait。

### picker UI(generation_view)

- 生成 panel 開啟時(或按需)`fetch_providers()`;依目前 kind(video/image/audio)列該 kind 的 provider 下拉 + 選定 provider 的 model 下拉;選定寫入送出時的 request.provider / model。fetch 失敗或未設定端點 → 不顯示 picker(或顯示「no endpoint」),既有行為不回歸。gpui 互動依慣例編譯+模型測試。

## Implementation Contract

- GenerationRequest.provider round-trip;build_submit_body 有/無 provider 各一測試。
- generate 工具帶 provider → request.provider 設定(e2e mock backend 斷言收到)。
- fetch_providers 解析 catalog 純測試;錯誤路徑。
- generation_view picker 結構測試(catalog→下拉項)。
- `cargo test --workspace` 全綠、desktop check;無端點時既有 honest error 不回歸。

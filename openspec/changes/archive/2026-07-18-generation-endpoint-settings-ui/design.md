## Decisions

### prefs 取代 env(鏡射 whisperModelPath)

- pane_prefs 加 `GENERATION_URL_KEY="generationEndpointUrl"` / `GENERATION_TOKEN_KEY="generationEndpointToken"`;`load_generation_endpoint(path) -> Option<(String,String)>`(兩者皆非空白才回 Some)、`save_generation_endpoint(path, url, token)`(逐欄位:空白 → remove key;沿用 read/write_prefs_root 原子寫)。
- `HttpGenerationBackend::from_config()` 改讀 `pane_prefs::default_prefs_path()` 的兩鍵(經 `resolve_config` 純解析),不再讀 env。`GenerationBackendConfig::from_env` 移除(或改名為測試專用);`resolve_config` 純函式保留。
- hub 的 install 點不變(仍呼叫 from_config,現在 prefs-backed);未設定 → None → honest error(零回歸)。

### Settings UI(AI/Agent pane,鄰接 whisper 欄位)

- 兩個 `TextField`:「Generation endpoint」URL 與 token;載入現值、Enter/blur commit 經 `save_generation_endpoint`;空白即移除鍵。chrome 沿用 whisper 欄位/skill sheet 樣式。token 欄位視覺與一般欄位相同(先不做遮蔽,phase 2 keychain 再議)。

### 測試專用 env

gated live 測試(`live_round_trip_submits_and_polls_to_success`)已直接 `GenerationBackendConfig::new(url,token)`,url/token 由測試自 env 讀取作參數注入——與 production `from_config` 無關,保留;註解標明 test-only。

## Implementation Contract

- 未設定 prefs:三個 generate 工具維持 honest error(既有測試不回歸)。
- prefs 有 URL+token:from_config 建 backend;save→load round-trip 一致;空白移除鍵(斷言 JSON 無鍵)。
- Settings 欄位載入/commit 的 pure 測試(save/load/清除/他鍵保留)。
- `cargo test -p fronda-app-shell-gpui` 兩相全綠、desktop check;gated live 測試仍可用(env 注入)。

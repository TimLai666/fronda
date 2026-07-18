## 1. 實作

- [x] 1.1 依 design「prefs 取代 env（鏡射 whisperModelPath）」,pane_prefs 加 generation endpoint URL/token 讀寫 helper+ pure 測試（save/load/清除/他鍵保留/壞檔）。驗證：`cargo test -p fronda-app-shell-gpui pane_prefs` 綠。
- [x] 1.2 依 design「prefs 取代 env」與「測試專用 env」,http_generation_backend `from_config` 改讀 prefs（移除 env 依賴,resolve_config 保留;gated live 測試改註記 test-only env 注入）。驗證：兩相綠、既有 honest-error 測試不回歸。
- [x] 1.3 依 design「Settings UI（AI/Agent pane,鄰接 whisper 欄位）」,settings_view AI/Agent pane 加 Generation endpoint URL + token 欄位（載入/commit,鄰接 whisper 欄位）。驗證：兩相綠、desktop check。

## 2. 收尾

- [x] 2.1 98-generation-protocol.md Configuration 段 env → prefs/UI;`cargo test --workspace` 全綠。驗證：內容審查。

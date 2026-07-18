## Summary

Fronda 是 GUI 軟體,生成端點設定(URL + token)不該靠環境變數。改為存進 `preferences.json`、由 Settings 介面設定——鏡射既有 `whisperModelPath` 欄位的模式。`HttpGenerationBackend::from_config()` 從讀 env 改為讀 prefs;Settings 的 AI/Agent pane 加「Generation endpoint URL」與「token」兩個欄位。gated live 測試改用 test-only 環境變數注入測試參數(不影響 production 路徑)。

## Motivation

使用者指出:GUI 軟體應該用介面設置,不是環境變數。D6 的 client config 目前 env-only,與 whisper model path 已建立的 prefs+UI 模式不一致。

## Non-Goals

- Gateway 服務自身的 config(bind/token/provider keys)——它是獨立 server daemon,env/config 檔是慣例,非 GUI 一部分。
- provider BYO-key 的 UI 管理——phase 2(真實 provider 落地時,且視 gateway 是否由 app 管理)。
- OS keychain 安全儲存 token——token 先存 preferences.json 明文(與 app 既有本地 config 一致);安全儲存 adapter 是既有的 follow-up(AGENTS.md 已記)。

## Impact

- Affected code: crates/app_shell_gpui/src/{pane_prefs.rs, http_generation_backend.rs, settings_view.rs}
- Affected specs: specs/rust-rewrite/98-generation-protocol.md 的 Configuration 段(env → prefs/UI)

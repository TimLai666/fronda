## Summary

決策 D4：`transcribe-local` optional feature ＋ whisper-rs（whisper.cpp bindings）實作既有 transcription seam；模型檔使用者提供（GGML/GGUF 路徑設定，不入庫）；feature off 或模型缺失時維持現行 host-deferred 行為（transcript 工具 honest 回報不可用）。

## Impact

- Affected specs: 無 delta
- Affected code: crates/app_shell_gpui/{Cargo.toml,src/transcribe.rs(新),掛載點}

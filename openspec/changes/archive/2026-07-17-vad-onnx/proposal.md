## Summary

決策 D3：`vad` optional cargo feature（app_shell_gpui）＋ `ort = 2.0.0-rc.12`（鎖版，download-binaries）跑 Silero VAD，實作既有 `SpeechAnalyzer` seam；模型檔（MIT，~2MB）入庫 assets；feature off 或模型缺失時回 None（RMS fallback 不變）。remove_silence 自此在有 feature 的 build 用真 VAD。

## Impact

- Affected specs: 無 delta（speech-analysis-seam spec 的 host 邊界自此有實作）
- Affected code: crates/app_shell_gpui/{Cargo.toml,src/vad.rs(新),src/editor_state_hub.rs 或掛載點}

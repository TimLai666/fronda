## Summary

決策 D1：NDF-NTSC timecode 精度以 host seam 補齊——`ClipAudioSource::timecode_frame_duration(source) -> Option<(i64, i64)>`（default None），`ProjectAudioSource` 以 ffmpeg 讀 tmcd stream time_base 實作；`manifest_tc_seconds` 有精確值時採用（frame × num/den），否則維持現行 DF-1001/1000 與 frame/quanta fallback。media.json 不動（鏡射 Swift 現場讀架構）。

## Impact

- Affected specs: 無 delta
- Affected code: crates/agent_contract/src/tool_exec.rs（seam + manifest_tc_seconds）；crates/app_shell_gpui/src/audio_source.rs（ffmpeg tmcd 實作）

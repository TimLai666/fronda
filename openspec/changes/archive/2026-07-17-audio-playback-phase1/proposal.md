## Summary

決策 D2 階段一：cpal 輸出裝置 adapter＋timeline 播放出聲＋audio meter 改餵真實輸出。新 `app_shell_gpui::audio_playback`：cpal stream、ring buffer、由 audio_core mixer 逐 chunk 供料（沿用 render_core::audio_plan 的 placements 解析與既有 decode seam）；播放/暫停/seek 掛既有 transport（PlayPause 等 MenuAction 已有 dispatch 點）。meter：播放時 StereoMeter 改 ingest 實際輸出 chunk（既有 playhead-driven 模式保留為暫停時 fallback）。

## Non-Goals

- scrub audio 與 PCM cache（#339，階段二）；變速/JKL 倍速音高處理（先靜音跳過非 1x）

## Impact

- Affected specs: 無 delta（audio meter 條目註記 live 模式）
- Affected code: crates/app_shell_gpui/{Cargo.toml(+cpal),src/audio_playback.rs(新),src/lib.rs,src/app_root.rs 或 preview_view.rs 的 transport 接線,src/preview_view.rs meter 餵入點}

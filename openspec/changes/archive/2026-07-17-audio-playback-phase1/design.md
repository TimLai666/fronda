## Decisions

cpal 預設輸出裝置、f32 stereo、裝置採樣率（mix 產出 48k 時以線性重採樣 or 直接以裝置率 mix——選後者：mix_timeline_audio 已參數化 sample_rate）。ring buffer（生產者＝背景 mix 執行緒、消費者＝cpal callback；underrun 補零）。播放狀態機：play 啟動 stream＋背景供料（從 playhead 幀對應秒起算），pause 停供料、stop stream 或保留 idle；seek 清 buffer 重供。與 UI 的 playhead 同步：以 audio clock（已消費 frame 數）回推 playhead，經 hub revision 通知（或既有 playback tick 機制——讀現行 PlayPause 實作決定，如現在完全沒有 playback tick，本階段建立它）。錯誤（無輸出裝置）→ 靜默降級回既有無聲播放，log 一行。測試：純環節（ring buffer、供料切塊、underrun、clock 回推）全測；cpal 實體輸出人工驗證列 follow-up。

## Implementation Contract

- ring buffer/供料/clock 純測試；播放狀態轉移測試；無裝置降級測試（mock backend trait 包 cpal——測試不開真裝置）。
- `cargo test -p fronda-app-shell-gpui` 兩相全綠、desktop check 通過（macOS/Windows/Linux 皆可編譯——cpal 跨平台）。

## Why

media panel 的影片素材仍是色塊縮圖。real-thumbnails spec 記載的邊界是「不引入解碼子系統」，但這個邊界可以在不破壞三平台建置的前提下跨過：以系統 ffmpeg 執行檔作為外部解碼 adapter（子行程抽首格），零建置期依賴、零新 crate、支援所有常見編碼；ffmpeg 不存在時行為與現況完全相同（色塊）。GitHub CI 的 ubuntu/macos runner 皆預裝 ffmpeg，可寫真實抽格測試。

## What Changes

- 新增 video_thumbnails adapter 模組：偵測 ffmpeg（FRONDA_FFMPEG 環境變數優先、否則 PATH 上的 ffmpeg），以子行程對影片來源抽單格縮圖（寬 160、0.5 秒處）寫入 Fronda 快取目錄；以「來源路徑＋mtime」雜湊為快取鍵，命中即直接回傳，來源更新自動失效
- 縮圖抽取在背景執行緒進行（子行程阻塞不佔 UI）；完成後寫入行程內快取表，media panel 下次渲染出現
- media panel 的 Video 類型 tile：縮圖就緒時以 gpui img 渲染，未就緒或無 ffmpeg 時維持型別色塊
- real-thumbnails spec 的邊界敘述更新：影片縮圖經系統 ffmpeg adapter 提供，無 ffmpeg 時退回色塊；連結進 app 的原生解碼函式庫仍為未採用的架構決策

## Non-Goals

- 引入 ffmpeg 函式庫綁定（ffmpeg-next 等）或任何原生解碼依賴
- 自動下載 ffmpeg 執行檔
- preview 畫面的影格渲染與播放（transport 已有時間軸，畫面渲染仍屬解碼子系統整合的後續架構工作）
- 音訊波形縮圖

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `real-thumbnails`: 影片 tile 由「永遠色塊」改為「系統 ffmpeg adapter 抽首格，無 ffmpeg 或抽取失敗時色塊」

## Impact

- Affected specs: 修改 `real-thumbnails`
- Affected code:
  - Modified:
    - crates/app_shell_gpui/src/lib.rs
    - crates/app_shell_gpui/src/media_panel_view.rs
  - New:
    - crates/app_shell_gpui/src/video_thumbnails.rs
  - Removed: (none)

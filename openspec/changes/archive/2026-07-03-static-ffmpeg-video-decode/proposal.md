## Why

影片縮圖目前靠子行程呼叫系統 ffmpeg 執行檔：執行時依賴使用者機器 PATH 上有 ffmpeg，沒裝就只有色塊。改為把 ffmpeg 靜態編進 binary、行程內解碼，執行時零系統依賴——任何使用者一裝即有影片縮圖，且省去每張縮圖的 process spawn。

## What Changes

- 新增 ffmpeg-the-third（原生綁定）與 image（PNG 編碼）依賴；ffmpeg 以 vcpkg 靜態 triplet 連入 binary（Windows: x64-windows-static-md；Linux/macOS: 對應 static triplet），不連動態庫、執行時無 .dll/.so/.dylib 依賴
- video_thumbnails 的 extract() 內部由「spawn ffmpeg.exe」改為行程內解碼：開檔、取最佳視訊串流、解出約 0.5 秒處首格、swscale 縮到 160 寬、image 編成 PNG 寫入快取。快取路徑、per-source 汰換、總量修剪、request_thumbnail、tile 接圖等 API 與行為全部不變
- 移除 FRONDA_FFMPEG 環境變數與 PATH ffmpeg 依賴；解碼失敗仍靜默退回色塊
- CI 的 ubuntu/macOS job 建置前安裝 libclang 與系統 ffmpeg 開發庫（版本在 crate 支援的 5.1–8.1 內），驗證解碼碼跨平台可建；Windows 完整靜態於本機驗證

## Non-Goals

- preview 影格渲染與播放（transport 已有時間軸，畫面渲染屬更大的解碼整合）
- 音訊波形縮圖
- 自訂解碼參數 UI

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `real-thumbnails`: 影片縮圖由「子行程呼叫系統 ffmpeg」改為「靜態連入的 ffmpeg 行程內解碼」，執行時零系統依賴

## Impact

- Affected specs: 修改 `real-thumbnails`
- Affected code:
  - Modified:
    - crates/app_shell_gpui/Cargo.toml
    - crates/app_shell_gpui/src/video_thumbnails.rs
    - .github/workflows/ci.yml
  - New: (none)
  - Removed: (none)

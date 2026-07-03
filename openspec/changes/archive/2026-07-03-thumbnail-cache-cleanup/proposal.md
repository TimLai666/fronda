## Why

縮圖快取（Fronda 設定目錄的 thumbnails/）目前只增不刪：來源影片 mtime 變動就寫一個新 `<hash>-<mtime>.png`，舊版本檔案留存；長期使用會無限累積。這是 video-thumbnails-ffmpeg-adapter 明文記載待處理的 trade-off。清理邏輯與解碼方式（子行程或函式庫）無關，可獨立完成。

## What Changes

- 每次成功產生某來源的新縮圖後，移除同一來源（相同 hash 前綴）但不同 mtime 的舊版本快取檔（精準 per-source 汰換）
- 應用啟動時在背景執行緒對整個快取目錄做總量上限修剪：超過上限（256 MB）時依檔案修改時間由舊到新刪除，直到低於上限（全域 backstop）
- 快取路徑輔助改為可取得穩定的 per-source 前綴，供汰換比對

## Non-Goals

- 更動解碼方式（另案處理 ffmpeg 靜態編譯）
- 使用者可設定的快取上限 UI
- 依存取時間（atime）的 LRU（atime 在多平台不可靠，改用 mtime 近似）

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `real-thumbnails`: 縮圖快取新增 per-source 舊版本汰換與啟動時的總量上限修剪

## Impact

- Affected specs: 修改 `real-thumbnails`
- Affected code:
  - Modified:
    - crates/app_shell_gpui/src/video_thumbnails.rs
    - crates/app_shell_gpui/src/app_root.rs
  - New: (none)
  - Removed: (none)

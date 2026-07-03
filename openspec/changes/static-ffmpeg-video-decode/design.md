## Context

video_thumbnails 目前以 std::process::Command spawn 系統 ffmpeg.exe 抽首格。快取模型（<hash>-<mtime>.png、evict_stale_versions、prune_by_size、request_thumbnail、tile 接圖）已穩定且與解碼方式無關。Rust host 為 x86_64-pc-windows-msvc。本機已備妥 LLVM/libclang（bindgen）與 vcpkg。ffmpeg 內建原生解碼器涵蓋 h264/hevc/prores/vp9/mpeg4 等，解碼縮圖不需外掛編碼庫。

## Goals / Non-Goals

**Goals:**

- ffmpeg 靜態連入 binary，行程內解碼首格，執行時零系統依賴
- 只改 extract() 內部，快取/請求/tile 等既有 API 與行為不變
- 三平台可建（Windows 先本機驗證，Linux/macOS 於 CI 驗證）

**Non-Goals:**

- preview 影格渲染／播放、音訊波形、解碼參數 UI

## Decisions

### vcpkg 靜態 ffmpeg + ffmpeg-the-third 綁定

依賴 ffmpeg-the-third（維護中的 ffmpeg-next 分支，支援 ffmpeg 7.x）與 image（RGB→PNG 編碼）。ffmpeg 由 vcpkg 以靜態 triplet 提供：Windows 用 x64-windows-static-md（動態 CRT，對齊 Rust MSVC 預設），Linux/macOS 用預設靜態 triplet。建置時 ffmpeg-sys 經 bindgen（libclang）產生綁定並靜態連結，執行時無 .dll/.so/.dylib 依賴。替代方案手刻 MSYS2+MSVC configure 已評估：vcpkg 內建正確處理 MSVC 靜態編譯，遠比手刻可靠且三平台一致，故採 vcpkg。

### extract() 改為行程內解碼

移除 ffmpeg_program 與子行程呼叫。extract(source, cache_dir)：cache_path_for → 命中直接回傳（不變）；否則 decode_first_frame_png(source, cache)：ffmpeg::format::input 開檔、best(Type::Video) 取串流、由 parameters 建 decoder、依串流 time_base 換算 0.5 秒 seek（失敗則從頭）、讀 packet 送解碼直到收到第一個 frame、swscale 轉 RGB24 且寬 160、高依原比例（偶數）、image::RgbImage 編 PNG 寫入 cache。成功後 evict_stale_versions（不變）。任何步驟失敗回 None（tile 色塊，行為與現況一致）。ffmpeg::init() 以 OnceLock 只呼叫一次。

### 建置環境設定

本機與 CI 以環境變數導引 ffmpeg-sys 找 vcpkg：VCPKG_ROOT 指向 vcpkg 安裝、Windows 設 VCPKGRS_TRIPLET=x64-windows-static-md 且不設 VCPKGRS_DYNAMIC（靜態）、LIBCLANG_PATH 指向 LLVM。CI 在 workspace 測試（ubuntu）與 shell check（macOS）job 前置步驟安裝 libclang 與系統 ffmpeg 開發庫（apt/brew，版本落在 crate 支援的 5.1–8.1 內），經 pkg-config 連結——CI 驗證解碼碼跨平台可建可跑；Windows 的完整靜態連結於本機驗證，release 產物的三平台靜態打包待 release pipeline 建立。這些是建置期需求，不影響其他 crate。

## Implementation Contract

- 行為：不論系統有無 ffmpeg 執行檔，含影片素材的專案在 media panel 顯示該影片實際首格（160 寬）；快取命中不重解碼；來源 mtime 變動重解碼（沿用既有快取邏輯）。解碼失敗（損毀檔、不支援編碼）靜默退回色塊。
- 介面／資料形狀：extract(source: &Path, cache_dir: &Path) -> Option<PathBuf> 簽名與語意不變（僅內部改行程內解碼）；cache_path_for/cache_prefix_for/evict_stale_versions/prune_by_size/request_thumbnail 完全不變。新增私有 decode_first_frame_png(source, out) -> Option<()>。移除 ffmpeg_program 與 FRONDA_FFMPEG。
- 失敗模式：ffmpeg init/open/decode/scale/encode/write 任一失敗→None；不 panic；不留半成品檔（寫入 cache 前先寫暫存再 rename，或寫失敗即刪）。
- 驗收標準：
  - 單元測試：cache_path_for/prefix/evict/prune 既有測試全保留通過（證明快取模型未回歸）
  - 整合測試：以內含的 ffmpeg 解碼——測試在建置期已靜態連入 ffmpeg，故不需系統 ffmpeg；以 include_bytes! 或測試內產生的最小影片解碼出非空 PNG（若無法內嵌測試影片，改以 ffmpeg 的 lavfi 記憶體來源產生單格並斷言 RGB 資料非空）
  - cargo test --workspace（本機，已備 ffmpeg）、clippy -D warnings、desktop-app build 全過；app smoke
  - CI 三平台建置通過（Windows 本機先證，Linux/macOS 由 CI 證）
- 範圍界線：in scope＝依賴、extract 內部、CI 建置設定、spec 更新；out of scope＝Non-Goals。

## Risks / Trade-offs

- [ffmpeg-the-third 對 ffmpeg 版本敏感] → vcpkg 鎖定的 ffmpeg 版本需與 crate 支援範圍相容（7.x）；以實際建置驗證，不相容則調 crate 版本或 vcpkg baseline
- [CI 每次建 ffmpeg 約 15-20 分] → 加 vcpkg binary cache（actions/cache 快取 vcpkg installed 樹）降低重複成本；首次仍慢
- [三平台靜態連結細節差異] → Windows 先本機驗證（最難的 MSVC ABI），Linux/macOS 於 CI 迭代；unix 上 ffmpeg-sys 優先 pkg-config，必要時以 FFMPEG_DIR 指向 vcpkg 樹強制
- [binary 體積增加數十 MB] → 靜態 ffmpeg 的固有成本，換取執行時零依賴，符合本 change 目標

## 1. 依賴與靜態 ffmpeg

- [x] 1.1 依 design 決策「vcpkg 靜態 ffmpeg + ffmpeg-the-third 綁定」：以 vcpkg 建置 x64-windows-static-md 的 ffmpeg（avcodec/avformat/swscale），Cargo.toml 加入 ffmpeg-the-third 與 image 依賴。驗證：設定 VCPKG_ROOT/VCPKGRS_TRIPLET/LIBCLANG_PATH 後 cargo build -p fronda-app-shell-gpui --features desktop-app --bin fronda 能連結靜態 ffmpeg 通過

## 2. 行程內解碼

- [x] 2.1 實作修改後需求「Image media renders real thumbnails」的解碼部分，依 design 決策「extract() 改為行程內解碼」：移除 ffmpeg_program/FRONDA_FFMPEG；新增 decode_first_frame_png（開檔、best video stream、seek 0.5s、解首格、swscale RGB24 寬 160、image 編 PNG 寫 cache），extract 改呼叫之並沿用 evict；ffmpeg::init 以 OnceLock 一次。驗證：既有 cache_path_for/prefix/evict/prune 測試全過（快取模型未回歸）；新增整合測試以靜態連入的 ffmpeg 解出非空 PNG；desktop-app build 通過；手動跑 app 匯入影片看到實際首格（人工項記錄於完成報告）

## 3. CI 三平台建置

- [x] 3.1 依 design 決策「建置環境設定」：ci.yml 的 workspace 測試（ubuntu）與 shell check（macOS）job 加前置步驟安裝 LLVM 與 vcpkg ffmpeg 靜態並設定 VCPKG_ROOT/VCPKGRS_TRIPLET/LIBCLANG_PATH（含 vcpkg installed 樹的 actions/cache）。驗證：push 後觀察 CI 三個 Rust 相關 job 綠燈

## 4. 全面驗證

- [x] 4.1 cargo test --workspace、cargo clippy --workspace --tests -- -D warnings 全過；app smoke（啟動加 MCP initialize）；CI 全綠。驗證：指令輸出與 CI 結果審閱

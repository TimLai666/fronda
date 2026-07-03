## 1. build.rs macOS framework

- [x] 1.1 依 design 決策「build.rs 擴充 macOS framework」：build.rs 於 target_os=macos 時 emit ffmpeg 靜態碼引用的系統 framework（Security/VideoToolbox/CoreMedia/CoreVideo/AudioToolbox/CoreFoundation/CoreServices 等，實際以連結錯誤收斂）。驗證：macOS release job 的 cargo build --release 連結通過（無 unresolved framework 符號）

## 2. release workflow

- [x] 2.1 依 design 決策「獨立 release.yml，matrix 三平台」「靜態 ffmpeg 連結環境（每平台）」「Linux 桌面系統庫」「打包與上傳」：新增 .github/workflows/release.yml（tag v* + workflow_dispatch、matrix linux/macos/windows），每平台裝 libclang/nasm 與（Linux）gpui 系統庫、vcpkg 靜態 ffmpeg、設環境、cargo build --release --features desktop-app --bin fronda、打包上傳 artifact、tag 時附到 Release。驗證：workflow_dispatch 觸發後三平台 job 全綠並產出 artifact（實作需求「Tagged and manual releases produce static-ffmpeg binaries per platform」的 dispatch scenario）

## 3. 驗證

- [x] 3.1 手動 dispatch 一次確認三平台綠燈與 artifact 內含 fronda 執行檔；確認既有 main CI 仍綠、release.yml 與 ci.yml 分離。驗證：CI 結果與下載的 artifact 內容審閱

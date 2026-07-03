## Why

static-ffmpeg-video-decode 交付了 Windows 的完整靜態 ffmpeg binary，但沒有任何 release 產物流程：使用者無處下載可執行檔，Linux/macOS 的「ffmpeg 靜態編入、零 runtime 依賴」也還沒被實際建置驗證。現在建立 release pipeline 填補這個缺口。

## What Changes

- 新增 .github/workflows/release.yml：git tag（v*）或手動 workflow_dispatch 觸發，matrix 建 Linux、macOS、Windows 三平台的 --release 桌面 binary
- 每個平台以 vcpkg 靜態 triplet 把 ffmpeg 連入 binary（Windows x64-windows-static-md、Linux x64-linux、macOS arm64-osx），libclang 供 bindgen；產物打包（tar.gz/zip）上傳為 workflow artifact，tag 觸發時附加到 GitHub Release
- Linux job 安裝 gpui 桌面所需系統開發庫（wayland/xkbcommon/x11/vulkan/fontconfig 等）——這是本 repo 首次在 Linux 建 desktop-app
- build.rs 擴充：macOS 靜態 ffmpeg 需連結的系統 framework（Security/VideoToolbox/CoreMedia/CoreVideo/AudioToolbox/CoreFoundation 等），比照現有 Windows 系統庫處理
- vcpkg 安裝以 actions/cache 快取,降低重複 ffmpeg 建置成本

## Non-Goals

- 程式碼簽章、公證（notarization）、安裝程式（.dmg/.msi/AppImage 打包器）
- 自動版號、changelog 產生
- 既有 ci.yml（PR 驗證）的改動——release 是獨立 workflow

## Capabilities

### New Capabilities

- `release-artifacts`: tag 或手動觸發時，為三平台產出 ffmpeg 靜態連入的 --release 桌面 binary 並上傳

### Modified Capabilities

(none)

## Impact

- Affected specs: 新增 `release-artifacts`
- Affected code:
  - Modified:
    - crates/app_shell_gpui/build.rs
  - New:
    - .github/workflows/release.yml
  - Removed: (none)

## Context

static-ffmpeg-video-decode 已證明 Windows 的 vcpkg x64-windows-static-md 靜態 ffmpeg 連結（含 build.rs 補系統庫）。gpui-ce 有 gpui_linux（wayland/x11/vulkan）。ci.yml 的 gpui-shell-check 只在 macOS 跑 cargo check；Linux 從未建過 desktop-app。開發機為 Windows，Linux/macOS 僅能經 CI 驗證。

## Goals / Non-Goals

**Goals:**

- tag/dispatch 觸發，三平台產出 ffmpeg 靜態連入的 release 桌面 binary
- 產物上傳為 artifact；tag 時附到 GitHub Release
- 可用 workflow_dispatch 在無 tag 情況下驗證

**Non-Goals:**

- 簽章／公證／安裝包、自動版號、改動 ci.yml

## Decisions

### 獨立 release.yml，matrix 三平台

新增 .github/workflows/release.yml，on: push tags v*、workflow_dispatch。matrix 含 { os: ubuntu-latest, triplet: x64-linux }、{ os: macos-latest, triplet: arm64-osx }、{ os: windows-latest, triplet: x64-windows-static-md }。每 job：checkout → rust stable → 裝 libclang/nasm 與（Linux）gpui 系統庫 → 快取 vcpkg → vcpkg install 靜態 ffmpeg → 設環境 → cargo build --release -p fronda-app-shell-gpui --features desktop-app --bin fronda → 打包 → 上傳。與 ci.yml 分離，PR 驗證不受影響。

### 靜態 ffmpeg 連結環境（每平台）

vcpkg install "ffmpeg[core,avcodec,avformat,swscale,swresample]:<triplet>"。設 VCPKG_ROOT=$VCPKG_INSTALLATION_ROOT（runner 內建 vcpkg）、VCPKGRS_TRIPLET=<triplet>、LIBCLANG_PATH。Windows 沿用已驗證設定。Linux/macOS 額外設 PKG_CONFIG_PATH 指向 vcpkg 靜態 .pc，供 ffmpeg-sys pkg-config 靜態探測。actions/cache 快取 vcpkg 二進位快取目錄，key 含 triplet。

### Linux 桌面系統庫

Linux job 前置 apt 安裝 gpui/Zed 慣用建置庫：libwayland-dev、libxkbcommon-dev（含 x11）、libxcb 系列、libvulkan-dev、libfontconfig-dev、libasound2-dev、libssl-dev、pkg-config、nasm、cmake、clang、libclang-dev。實際清單以 CI 建置錯誤迭代收斂，缺什麼補什麼。

### build.rs 擴充 macOS framework

比照 Windows 系統庫，build.rs 於 target_os=macos 時 emit cargo:rustc-link-lib=framework=<name>，涵蓋 ffmpeg 靜態碼引用的 Security、VideoToolbox、CoreMedia、CoreVideo、AudioToolbox、CoreFoundation、CoreServices 等。實際清單以 macOS 連結錯誤迭代收斂。

### 打包與上傳

Linux/macOS 以 tar.gz、Windows 以 zip 打包 fronda 執行檔（命名含平台）。actions/upload-artifact 供 dispatch 驗證下載；push tag 時以 softprops/action-gh-release 附到對應 Release。

## Implementation Contract

- 行為：對 repo 推 v* tag（或手動 dispatch）後，release workflow 為 Linux/macOS/Windows 各產出一個含 fronda 桌面執行檔的壓縮包，其中 ffmpeg 已靜態連入（執行時不需系統 ffmpeg）；tag 觸發時壓縮包出現在該 tag 的 GitHub Release。dispatch 觸發時壓縮包為可下載的 workflow artifact。
- 介面／資料形狀：.github/workflows/release.yml（tag+dispatch 觸發、三平台 matrix）；build.rs 於 macOS 追加所需 framework 連結。產物命名如 fronda-<os>-<arch>.tar.gz / .zip。
- 失敗模式：任一平台建置或連結失敗即該 matrix job 紅燈（release 求全平台成功，不遮蔽）；vcpkg/系統庫缺失以 CI 錯誤呈現並迭代補齊。
- 驗收標準：
  - workflow_dispatch 手動觸發一次，三平台 job 全綠並產出 artifact（Linux 首次 desktop-app 建置成功、三平台靜態 ffmpeg 連結成功）
  - 下載其中 Linux 或 macOS artifact 確認含 fronda 執行檔（非空）
  - 既有 ci.yml 不受影響、main CI 仍綠
- 範圍界線：in scope＝release.yml、build.rs macOS framework、三平台靜態建置與打包上傳；out of scope＝簽章/公證/安裝包、版號、ci.yml。

## Risks / Trade-offs

- [Linux 首次建 gpui desktop-app，系統庫清單未知] → 以 CI 迭代收斂；gpui-ce 有 gpui_linux 支援，deps 為 Zed 慣用集
- [三平台 vcpkg 靜態 ffmpeg 各有系統庫/framework 差異] → Windows 已證；Linux 靠 vcpkg .pc 的 Libs.private，macOS 靠 build.rs framework；逐一以連結錯誤補齊
- [vcpkg 每次建 ffmpeg 慢] → actions/cache 二進位快取；release 非高頻，可接受
- [僅能經 CI 驗證 Linux/macOS] → 以 workflow_dispatch 迭代，誠實回報每次結果

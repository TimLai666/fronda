## Context

media panel 已對 Image 素材渲染實檔（MediaItem.source_path 解析、gpui img）。影片 tile 為色塊。fronda_config_dir() 已提供 Fronda 自有狀態目錄。MediaPanelView 以 revision 監看重建、每次 render 讀取狀態的模式已建立。GitHub CI（ubuntu/macos runner）與本開發機皆有 ffmpeg 可執行檔。

## Goals / Non-Goals

**Goals:**

- 影片素材 tile 顯示真實首格縮圖（有 ffmpeg 時），快取避免重複抽取，來源檔更新後縮圖跟著更新
- 抽取不阻塞 UI 執行緒；無 ffmpeg／抽取失敗時 tile 與現況相同
- adapter 邏輯可測：快取鍵、命中短路、無 ffmpeg 回 None、真實抽格（ffmpeg 存在時）

**Non-Goals:**

- 原生解碼函式庫、自動下載 ffmpeg、preview 影格渲染、音訊波形

## Decisions

### 系統 ffmpeg 子行程 adapter（video_thumbnails 模組）

新增 video_thumbnails 模組（無 gpui）：
- ffmpeg_program() -> String：FRONDA_FFMPEG 環境變數優先，否則 "ffmpeg"（交給 PATH 解析）
- thumbnail_cache_dir() = fronda_config_dir()/thumbnails
- cache_path_for(source) ：以來源絕對路徑字串＋檔案 mtime 的簡單雜湊（與 tile_hue 同風格的位元組運算，避免引入雜湊 crate）組檔名 <hash>.png；mtime 進鍵值使來源更新自動產新鍵，舊檔留存由快取目錄自然累積（v1 不做清理，記入 trade-off）
- extract(source, cache) -> Option<PathBuf>：快取命中直接回傳；否則執行 ffmpeg -y -ss 0.5 -i <source> -frames:v 1 -vf scale=160:-2 <cache>（stdout/stderr 靜默），成功且檔案存在回 Some，任何失敗（含 ffmpeg 不存在的啟動錯誤）回 None
- 選 -ss 0.5 而非 0：避開常見的黑首格；-2 保偶數高度滿足編碼器

### 背景抽取與行程內結果表

模組內 process-wide 結果表 OnceLock<Mutex<HashMap<PathBuf, Option<PathBuf>>>> 與進行中集合：request_thumbnail(source) 非阻塞——已有結果回傳之，進行中回 None，否則標記進行中並 std::thread::spawn 執行 extract、完成後寫表。MediaPanelView render 對 Video 且有 source_path 的 item 呼叫 request_thumbnail，Some 時以 img 渲染；因 render 每幀呼叫，完成後的下一次重繪自然出現（media panel 本就隨互動與 revision 重繪；不額外接完成通知，記入 trade-off：完成瞬間若無任何重繪觸發會延遲到下次重繪才顯示）。用 std::thread 而非 gpui background executor，讓模組保持無 gpui、可純測。

### spec 邊界更新

real-thumbnails 的 Video/audio 段改述：影片經系統 ffmpeg adapter；音訊維持色塊；原生解碼函式庫仍為未採用的明確架構決策。

## Implementation Contract

- 行為：系統有 ffmpeg 時，含影片素材的專案在 media panel 顯示該影片的實際首格（160 寬）縮圖；第二次開啟不再啟動 ffmpeg（快取命中）；影片檔內容更新（mtime 變動）後縮圖重抽。無 ffmpeg 的機器上行為與本 change 前完全一致。
- 介面／資料形狀：video_thumbnails::{extract(source: &Path, cache: &Path) -> Option<PathBuf>, cache_path_for(source: &Path, cache_dir: &Path) -> Option<PathBuf>, request_thumbnail(source: &Path) -> Option<PathBuf>}；ffmpeg 指令形如 ffmpeg -y -ss 0.5 -i in -frames:v 1 -vf scale=160:-2 out.png。
- 失敗模式：ffmpeg 缺失、子行程非零退出、輸出檔未產生、來源 mtime 不可讀→一律 None（tile 色塊），結果表記錄 None 避免重複嘗試；快取目錄不可寫→None。
- 驗收標準：
  - 單元測試：cache_path_for 對同來源同 mtime 穩定、mtime 變動產新鍵；extract 以 FRONDA_FFMPEG 指向不存在程式時回 None；快取命中不執行子行程（預放快取檔後以壞 ffmpeg 路徑仍回 Some）
  - 整合測試（ffmpeg 存在時執行，否則 skip 並印明）：先以 ffmpeg 產生 1 秒測試影片（lavfi testsrc），extract 回 Some 且 PNG 檔非空
  - cargo test --workspace、clippy -D warnings、desktop-app check、app smoke 全過
- 範圍界線：in scope＝adapter、背景抽取、Video tile 接圖、spec 更新；out of scope＝Non-Goals 全部。

## Risks / Trade-offs

- [快取目錄無清理，長期累積] → 縮圖各約數 KB；清理策略留待儲存管理功能，記錄於此
- [完成後無主動重繪通知，最壞延遲到下次重繪] → media panel 隨互動與 revision 頻繁重繪，實際延遲不可察；若成問題再接通知
- [測試機/CI 無 ffmpeg 時整合測試不執行] → 測試明確印出 skip 原因；單元測試不依賴 ffmpeg，核心邏輯仍有覆蓋
- [ffmpeg 輸出格式差異（老版本旗標）] → 使用的旗標（-ss/-frames:v/-vf scale）為十年以上穩定介面

## Context

video_thumbnails 的快取檔名為 `<hash>-<mtime_ms>.png`，其中 `<hash>` 為來源絕對路徑的 FNV-1a、對每個來源穩定。cache_path_for 目前一次算出完整路徑。fronda_config_dir()/thumbnails 是快取目錄。app_root open_main_window 已是啟動接線點（已在此啟動 MCP）。

## Goals / Non-Goals

**Goals:**

- 來源更新後不留舊縮圖（per-source 汰換）
- 快取總量有上限，啟動時自動修剪、不阻塞 UI
- 邏輯純函式、可用 temp 目錄測試；與解碼方式無關

**Non-Goals:**

- 解碼方式變更、可設定上限 UI、atime LRU

## Decisions

### cache_path_for 拆出穩定前綴

抽出 source_key(source) -> Option<(String, u128)> 回傳 (hash 十六進位字串, mtime_ms)；cache_path_for 以此組 `<hash>-<mtime>.png`；新增 cache_prefix_for(source) -> Option<String> 回傳 `<hash>-`。前綴用於辨識同來源的所有版本檔。

### per-source 舊版本汰換（evict_stale_versions）

extract 成功寫入後呼叫 evict_stale_versions(cache_dir, prefix, kept)：掃描 cache_dir，刪除檔名以 prefix 起始且不等於 kept 檔名者。只影響同來源舊 mtime 版本，不碰其他來源。刪除失敗忽略（快取非關鍵）。

### 啟動時總量上限修剪（prune_by_size）

prune_by_size(cache_dir, max_bytes) -> u64（回傳釋放位元組）：列舉快取目錄檔案與其 size、modified 時間，總量 <= max_bytes 時直接返回 0；否則依 modified 由舊到新排序，逐一刪除並累減總量，直到 <= max_bytes。常數 THUMBNAIL_CACHE_MAX_BYTES = 256 * 1024 * 1024。app_root open_main_window 以 std::thread::spawn 呼叫（背景、不阻塞、失敗僅忽略）。用 mtime 而非 atime：atime 在 Windows/Linux 常被停用或不精確，mtime（縮圖寫入時間）為穩定近似。

## Implementation Contract

- 行為：某影片來源 mtime 變動後產生新縮圖，該來源的舊縮圖檔被移除，快取內該來源只剩一個檔；其他來源的縮圖不受影響。快取總量超過 256 MB 時，下次啟動背景修剪會刪到上限以下（先刪最舊）。快取未超標時啟動不刪任何檔。所有清理失敗都靜默、不影響 app。
- 介面／資料形狀：cache_prefix_for(&Path) -> Option<String>；evict_stale_versions(cache_dir: &Path, prefix: &str, kept: &Path)；prune_by_size(cache_dir: &Path, max_bytes: u64) -> u64；常數 THUMBNAIL_CACHE_MAX_BYTES。
- 失敗模式：目錄不可讀、檔案刪不掉→忽略該檔繼續；kept 檔不存在也不誤刪其他來源。
- 驗收標準：
  - 單元測試：evict 移除同前綴舊版本、保留 kept、不碰不同前綴；prune 未超標回 0 不刪、超標刪最舊直到低於上限、空目錄安全；cache_prefix_for 對同來源穩定
  - extract 成功後同來源舊版本被清（以 temp cache 目錄模擬，需 ffmpeg 或改以直接寫檔＋evict 驗證邏輯）
  - cargo test --workspace、clippy -D warnings、desktop-app check、app smoke 全過
- 範圍界線：in scope＝汰換、修剪、啟動接線、spec 更新；out of scope＝Non-Goals。

## Risks / Trade-offs

- [mtime 近似 LRU 可能刪到仍在用但寫入較早的縮圖] → 最壞只是下次重新抽取；256 MB 上限下極少觸發
- [啟動修剪掃描大量檔案的成本] → 背景執行緒、僅啟動一次；檔案數與快取上限成比例，可接受

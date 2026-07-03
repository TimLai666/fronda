## Context

ProjectBundle::save_to 會整包重寫（含 chat、transcripts、media 目錄同步），但 EditorStateHub 只持有 timeline 與 manifest；用整包路徑儲存會以空集合覆蓋磁碟上的 chat/transcripts。project.json 與 media.json 的檔名常數（TIMELINE_FILENAME、MANIFEST_FILENAME）在 core_model，寫入 helper（write_json）是 project_io 的私有函式。menu 的 SaveProject 在 app_root 的 handle 分支目前為空。

## Goals / Non-Goals

**Goals:**

- 儲存只觸碰 project.json 與 media.json，磁碟上其他專案內容原樣保留
- MCP 編輯後 Cmd/Ctrl+S 能把結果持久化，重開專案讀回同樣內容
- 無專案開啟時儲存回報錯誤而非靜默成功

**Non-Goals:**

- SaveProjectAs、dirty 標記、自動儲存、其他 bundle 內容寫回

## Decisions

### project_io 新增窄路徑 save_project_state

新增公開函式 save_project_state(root: &Path, timeline: &Timeline, manifest: &MediaManifest) -> Result<(), BundleError>：確保目錄存在後以既有 write_json 寫 project.json 與 media.json。放 project_io 而非 shell，因為它屬於磁碟格式契約層、可與 open 的 round-trip 一起純測試。替代方案是走 ProjectBundle::open 改欄位再 save()，會整包重寫且對自身 media 目錄做無意義同步，被否決。manifest 一律寫出（共享狀態必有 manifest，空 manifest 寫出空 entries 是合法格式）。

### EditorStateHub::save 綁定目前專案根目錄

save(&self) -> Result<(), String>：project_root 為 None 時回 Err("no project open" 類訊息)；有 root 時鎖 executor、以 save_project_state 寫回。鎖範圍只涵蓋讀取（clone timeline/manifest 後解鎖再寫檔），避免磁碟 IO 佔住 MCP 的 executor 鎖超過必要時間。

### SaveProject 選單接線

app_root 的 SaveProject 分支呼叫 hub.save()，Err 以 eprintln 記錄。SaveProjectAs 維持 no-op（需對話框）。

## Implementation Contract

- 行為：load_bundle 開啟專案 → MCP create_folder → hub.save() → 重新 ProjectBundle::open 同一路徑，讀回的 manifest 含新資料夾、timeline 與儲存時一致；專案內既有的 chat/*.json 等檔案內容不變。未開專案時 save() 回 Err，不寫任何檔案。
- 介面／資料形狀：project_io::save_project_state(&Path, &Timeline, &MediaManifest) -> Result<(), BundleError>；EditorStateHub::save(&self) -> Result<(), String>。
- 失敗模式：目錄不可寫、序列化失敗回傳對應 BundleError 轉字串；executor 鎖 poisoned 回 Err 不 panic。
- 驗收標準：
  - project_io 新測試：save_project_state 後以 ProjectBundle::open round-trip 驗證 timeline fps 與 manifest 內容，且目錄中預先放置的 chat/session.json 檔案原樣保留
  - app_shell_gpui 新測試：hub load_bundle → 透過 executor 執行 create_folder → save() → 重新 load_bundle 讀回含該資料夾；無 root 時 save() 回 Err
  - cargo test --workspace 與 cargo clippy --workspace --tests -- -D warnings 全過
- 範圍界線：in scope＝save_project_state、hub.save、SaveProject 接線；out of scope＝SaveAs、dirty、自動儲存、bundle 其他檔案。

## Risks / Trade-offs

- [只寫兩檔的窄儲存與 Swift 整包儲存語意差異] → Swift 版其他檔案（chat 等）由各自子系統寫入，project.json/media.json 本來就是主要儲存面；記入 spec 作為明確決策
- [MCP 正在 mutation 時按下儲存] → executor 鎖保證讀到一致快照；寫檔在鎖外，最壞情況存到「儲存瞬間」的狀態，符合預期

## Context

共享狀態（EditorStateHub）與 MCP 掛載已完成，但狀態內容永遠是 Timeline::default()。project_io::ProjectBundle::open 已能讀取完整 .palmier 套件（timeline 為必要檔，manifest 為選配）。timeline 畫面由 TimelineView 渲染 timeline_model::TimelineState，目前資料來源是 with_default_tracks() 的寫死示範內容。core_model 側 Track 有 id、type(ClipType)、muted、hidden、clips；Clip 有 id、start_frame、duration_frames、media_ref；對應 view 側 TrackRow（id、kind、label、muted、hidden）與 ClipSlot（id、track_id、start_frame、duration_frames、label）。

## Goals / Non-Goals

**Goals:**

- 一條可測試的載入管線：.palmier 路徑 → ProjectBundle → 共享 executor（MCP 立即可見）
- timeline 畫面渲染共享狀態的真實 tracks/clips，MCP mutation 後畫面跟著更新
- NewProject 與載入路徑都經過 hub，畫面與 MCP 看到的永遠是同一份

**Non-Goals:**

- 檔案選擇對話框、儲存回寫、timeline 編輯互動、media panel 綁定、chat/transcript/visual index 載入

## Decisions

### EditorStateHub 增加 load_bundle 與專案根目錄記錄

load_bundle(path) 呼叫 ProjectBundle::open，成功後以既有 load_project 載入 timeline 與 manifest（manifest 為 None 時用 MediaManifest::default()），同時把專案根目錄記進 hub（Mutex<Option<PathBuf>> 欄位，供日後儲存與相對路徑解析）。失敗回傳 Result 帶 BundleError 訊息字串，不 panic、不改動現有狀態。app_shell_gpui 因此新增 project_io 依賴。替代方案是把 load_bundle 放在 app_root，但載入邏輯不依賴 gpui、放 hub 可純測試。

### timeline_model 增加 core Timeline 到 TimelineState 的純對應函式

新增 TimelineState::from_core(timeline: &core_model::Timeline, manifest: &MediaManifest)：Track 依 type 對應 TrackKind（ClipType::Audio → Audio，其餘 → Video），標籤依序編號（Video 1、Audio 1，對齊既有示範命名）；Clip 對應 ClipSlot，標籤用 manifest.display_name_for(media_ref)，查不到時退回 media_ref；fps 取 timeline.fps；total_frames 取所有 clip 最大 start_frame + duration_frames，並保底既有預設 600 避免空專案出現零寬 timeline。zoom/scroll/playhead 用 TimelineState::new() 預設值。純函式、無 IO，放 timeline_model 以便單元測試。

### TimelineView 以 revision 重建資料、保留 view 狀態

TimelineView 增加 state_revision 欄位，render 開頭比對 EditorStateHub::global().revision()：不同時以 from_core 重建 tracks/clips/fps/total_frames，但保留現有 state 的 zoom_scale、scroll_x、scroll_y、playhead_frame（view 狀態不因資料重建而跳動），然後 cx.notify()。與 agent_panel_view 既有的 revision 監看模式一致。with_default_tracks 不再是 runtime 資料來源，僅測試使用。

### AppRoot 的 open_project_at 與 NewProject 經過 hub

AppRoot 增加 open_project_at(&mut self, path, cx)：呼叫 hub.load_bundle，Ok 時 open_editor(cx)，Err 時停留原畫面並以 eprintln 記錄（shell 尚無 toast/alert 元件，錯誤呈現另案）。menu 的 NewProject 分支改為先 hub.load_project(Timeline::default(), MediaManifest::default()) 再 open_editor，確保新專案時 MCP 看到的是乾淨狀態而非上一個專案殘留。OpenProject 分支維持 open_editor 現狀（無檔案對話框可觸發 open_project_at，該路徑供測試與日後對話框接線）。

## Implementation Contract

- 行為：以 fixtures 的 .palmier 目錄呼叫 EditorStateHub::load_bundle 後，MCP 的 get_timeline 回傳該專案的 timeline 內容，hub revision 遞增；TimelineView 下一次 render 顯示該專案的 tracks 與 clips。載入不存在的路徑回傳 Err，hub 狀態與 revision 不變。NewProject 後 MCP get_timeline 回傳空白 timeline。
- 介面／資料形狀：EditorStateHub::load_bundle(&self, path: &Path) -> Result<(), String>、EditorStateHub::project_root(&self) -> Option<PathBuf>；TimelineState::from_core(&core_model::Timeline, &MediaManifest) -> TimelineState；AppRoot::open_project_at(&mut self, &Path, &mut Context<Self>)。
- 失敗模式：load_bundle 對缺 timeline 檔、路徑不存在、JSON 解碼失敗回傳 Err(訊息含路徑)，共享狀態保持原樣；from_core 對零 clip 專案回傳空 clips 且 total_frames 為預設下限，不除零、不 panic。
- 驗收標準：
  - app_shell_gpui 新測試：load_bundle 以 tempdir 寫出最小 project.json 後載入成功（fps 反映於 executor、revision 遞增、project_root 記錄）、載入不存在路徑回 Err 且 revision 不變
  - timeline_model 新測試：from_core 對應 track 種類與 clip 標籤（manifest 命中與退回 media_ref 兩種）、空專案 total_frames 保底、total_frames 取最大結束 frame
  - cargo test --workspace 與 cargo clippy --workspace --tests -- -D warnings 全過
  - cargo check -p fronda-app-shell-gpui --features desktop-app --bin fronda 通過
  - 手動驗證：建一個含 project.json 的 .palmier 目錄，跑 app 後由測試碼或 curl 確認 get_timeline 反映載入內容（UI 需 open_project_at 接線後由 NewProject/程式路徑觸發）
- 範圍界線：in scope＝load_bundle、from_core 對應、TimelineView 重建、open_project_at 與 NewProject 接 hub；out of scope＝檔案對話框、儲存、編輯互動、media panel、錯誤 UI。

## Risks / Trade-offs

- [大型專案每次 revision 變更全量重建 TimelineState] → 目前資料量級（數百 clip）在 UI 幀預算內；之後若不足再做增量
- [display_name_for 對離線媒體回傳 Offline 字樣作為 clip 標籤] → 沿用既有 RES 契約行為，屬正確呈現而非缺陷
- [NewProject 覆蓋共享狀態會丟掉未儲存的 MCP 編輯] → 尚無儲存管線，任何專案切換本來就不保存；儲存另案時一併處理確認流程
- [ClipType::Image/Text 對應到 Video track 種類] → view 端 TrackKind 目前僅 Video/Audio 兩種，映射保守；日後擴充 TrackKind 時由 from_core 單點調整

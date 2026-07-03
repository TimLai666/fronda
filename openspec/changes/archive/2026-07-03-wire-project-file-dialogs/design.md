## Context

gpui-ce 的 App 提供 prompt_for_paths(PathPromptOptions{files, directories, multiple, prompt}) 與 prompt_for_new_path(directory, suggested_name)，皆回傳 oneshot Receiver，非同步送回 Option<路徑>（取消為 None）。Context<T>::spawn 提供 async 任務並可透過 WeakEntity::update 回主執行緒更新 view。AppRoot 已有 open_project_at（load_bundle 成功才切 Editor），EditorStateHub 已有 save 與 project_root。.palmier 專案是目錄套件。

## Goals / Non-Goals

**Goals:**

- Cmd/Ctrl+O 開目錄選擇對話框並載入選定專案
- Cmd/Ctrl+Shift+S 另存目前狀態到新目錄並切換 project_root
- 對話框取消時不做任何事、不留半套狀態

**Non-Goals:**

- recent projects、儲存回饋 UI、媒體匯入、副檔名過濾

## Decisions

### EditorStateHub 新增 save_as

save_as(&self, root: &Path) -> Result<(), String>：鎖內 clone timeline/manifest、鎖外 save_project_state 寫到新 root，成功後把 project_root 換成新 root。寫入失敗時 project_root 不變。與 save 共用讀取邏輯，抽出私有 snapshot helper 避免重複。

### 對話框以 cx.spawn 非同步接線

OpenProject：cx.prompt_for_paths(PathPromptOptions { files: false, directories: true, multiple: false, prompt: Some("Open".into()) }) 取得 receiver，cx.spawn 等待，Ok(Some(paths)) 且非空時以 WeakEntity::update 呼叫 open_project_at。SaveProjectAs：cx.prompt_for_new_path(起始目錄＝目前 project_root 或使用者家目錄，suggested_name Some("Untitled.palmier")) 等待後呼叫 hub.save_as，Err 以 eprintln 記錄。取消（None）與 channel 錯誤都靜默返回。替代方案是引入 rfd 套件，gpui 已內建故不採用。

## Implementation Contract

- 行為：按 Cmd/Ctrl+O 出現系統目錄選擇對話框，選定含 project.json 的目錄後畫面切到 Editor 且 MCP get_timeline 反映該專案；取消對話框畫面不變。按 Cmd/Ctrl+Shift+S 出現另存對話框，選定新位置後該目錄出現 project.json 與 media.json，且之後的 Cmd/Ctrl+S 存到新位置（project_root 已切換）。
- 介面／資料形狀：EditorStateHub::save_as(&self, &Path) -> Result<(), String>；AppRoot 的 OpenProject/SaveProjectAs 分支各自為一個 cx.spawn 任務；PathPromptOptions 以 directories-only 模式選 .palmier 目錄。
- 失敗模式：對話框取消或 receiver 錯誤→無動作；load_bundle 失敗→留在原畫面（沿用 open_project_at 行為）；save_as 寫入失敗→回 Err、project_root 不變、eprintln 記錄。
- 驗收標準：
  - app_shell_gpui 新測試：save_as 寫出 project.json/media.json 到新目錄且 project_root 切換；寫入失敗（唯讀或不可建立的路徑不可靠跨平台，改以「save_as 到合法新目錄後 save() 寫到新目錄」驗證 root 切換語意）
  - cargo check -p fronda-app-shell-gpui --features desktop-app --bin fronda 通過；cargo test --workspace 與 clippy -D warnings 全過
  - 手動驗證：跑 app 按 Ctrl+O 出現對話框、取消無事；選定 fixtures 專案目錄後切到 Editor（此項需人工操作，記錄於 Notes 由使用者抽查）
- 範圍界線：in scope＝save_as、兩個選單分支的對話框接線；out of scope＝recent projects、回饋 UI、匯入、過濾。

## Risks / Trade-offs

- [macOS 上 .palmier 目錄在對話框顯示為套件] → gpui 平台層行為，directories 模式仍可選取；Windows/Linux 本來就以資料夾呈現
- [headless 環境無法自動測對話框] → 對話框僅是路徑來源，載入／儲存邏輯已由 hub 測試覆蓋；UI 觸發留人工抽查
- [save_as 到已存在且含其他專案檔的目錄會覆蓋 project.json] → 平台另存對話框語意本身會確認覆蓋；窄儲存不動其他檔案

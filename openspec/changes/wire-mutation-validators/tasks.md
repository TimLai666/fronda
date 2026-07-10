## 1. 決策與接線

- [x] 1.1 盤點 mutation.rs 全部 validators 與 executor 行內檢查的重疊/缺口矩陣（工具 × 規則），記錄於本 change；依矩陣確認方案 A（統一接線）可行性——validator 輸入型別與 executor 的 args 解析相容性
- [x] 1.2 execute() dispatch 前接 tool→validator 映射（單行 match 或表驅動）；validator Err → 工具 Err 原文回傳；行內重複檢查刪除（保留 validator 沒有的執行期檢查如資產存在性）
- [x] 1.3 e2e 測試：volume 1.5 / opacity -0.1 / speed 0 / trim 負值 / frame 超 ceiling 各一，經 executor.execute 拒絕；既有全部工具測試不得回歸

## 2. 收尾

- [x] 2.1 三 gate exit code 全綠；AGENTS.md porting table #144 行更新為真 live 狀態

---

## 1.1 盤點結果：validator × executor 矩陣

方案 A（validators 統一接線在 `execute_inner` dispatch 前）**可行**，但不是全部 30 個 validators 都能照原樣接。逐一比對輸入解析後分三類：

### WIRED（23 個工具 → 20 個 validators，`ToolExecutor::validate_args` gate）

| Tool | Validator | Gate 新增的 live 保護 | 行內重複刪除 |
|---|---|---|---|
| set_clip_properties | validate_set_clip_properties(args, None) | **#144 volume/opacity 0..1、trim >= 0**（原本 dormant）；background/border 形狀+hex 檢查 | speed<=0、#264 bounds 迴圈、clipIds 空檢查（3 處刪除） |
| remove_clips | validate_remove_clips | — | clipIds 空檢查 |
| move_clips / move_clips_linked | validate_move_clips | clipIds 空拒絕；toTrack/toFrame 至少一個 | toFrame #264 bounds |
| add_clips | validate_add_clips | mediaIds 空拒絕（訊息統一） | mediaIds 空檢查 |
| insert_clips | validate_insert_clips | **mediaIds 空拒絕（原本行內沒有）** | frame<0、frame #264 bounds |
| remove_tracks | validate_remove_tracks（對齊後） | trackIds 空/非字串拒絕 | trackIds 空檢查 |
| add_texts | validate_add_texts(args, 由 trackIndex 解析的 track type)（對齊後） | **MUT-020：audio track 拒絕（proposal 引用的具體案例）**；顯式 startFrame/durationFrames 的範圍檢查提前 | 無（行內檢查在 default 之後跑，涵蓋 defaulted 值，非重複——保留） |
| add_captions | validate_add_captions | clipIds 給了但全非字串/空陣列 → 拒絕 | 無 |
| add_shapes | validate_add_shapes | — | entries 空檢查 |
| apply_animation | validate_apply_animation | clipId/preset 空字串拒絕 | 無（ok_or_else 是取值不是檢查） |
| apply_color | validate_apply_color | clipId 空字串拒絕 | 無 |
| apply_effect | validate_apply_effect | clipId 空字串拒絕 | 無 |
| create/rename/delete_folder, rename/delete_media, move_to_folder | validate_*（MUT-022，6 個） | 空字串 id/name 拒絕 | 無 |
| set_chroma_key / set_blend_mode / set_color_grade | validate_* | clipId 空字串拒絕 | 無 |
| generate_music | validate_generate_music | prompt 空字串拒絕 | 無 |

### 已統一（不需接線）

| Tool | 原因 |
|---|---|
| set_keyframes | executor 與 validate_set_keyframes 已共用 `keyframe_property_arity` + `parse_keyframe_rows`（AGENTS.md 記載的設計）；接線只會重複解析並打亂既有釘住的錯誤訊息 |
| hex color（MUT-023） | 兩層都走 `hex_color_parser::parse_hex_color` |

### UNWIRED — 解析形狀與 live executor 分歧（接了會拒絕合法呼叫）

| Validator | 分歧 | 處置 |
|---|---|---|
| validate_split_clip (MUT-016) | 驗證的是舊單數 `split_clip`（clipId+frame）；live 工具是 #186 的 `split_clips`（`splits[{clipId, atFrame}]` 或 `trackIndex`+`frames[]`）。executor 行內已含 bounds+interior 檢查 | 不接；加 doc 註記 |
| validate_ripple_delete_ranges (MUT-017/018) | validator 走 Swift 契約：clipId XOR trackIndex、range key 用 `startFrame`/`endFrame`、`seconds` 模式；Rust executor + schema 只支援 `trackIndex` + `ranges[{start, end}]`。接線會讓所有合法呼叫死在 "'ranges' must contain at least one valid range" | 不接；分歧記錄於此。executor 缺 clip-scoped 模式是既有 gap，不屬本 change |
| validate_import_folder | validator + schema 用 `path`；executor（stub）讀 `folderName` ——executor 自己就跟 schema 分歧。接 validator 會讓現行 folderName 呼叫全滅 | 不接；executor/schema 分歧另列 Notes |
| validate_inspect_color / validate_duplicate_project | 全欄位 optional / 無參數，接了零保護 | 不接 |
| validate_move_clips_linked | 簽名吃 `&[String]` 非 `&Value`（文件層函式）；move_clips_linked 工具改 gate validate_move_clips | 以 validate_move_clips 代替 |

### 對齊修正（executor 解析為準，validator 改寫）

1. **validate_remove_tracks**：原本解析 `trackIds` 為 u64 index 陣列；live executor + Rust schema 都是**字串 track id**（Swift 現行是 `trackIndexes` 整數——Rust 工具面早已分歧，executor 為準）。改為非空字串陣列 + 保序 dedup；`RemoveTracksInput.track_ids: Vec<String>`。
2. **validate_add_texts**：原本每個 entry 強制要求 `text`/`startFrame`/`durationFrames` 三鍵齊全，且越界 entry 被 filter_map **靜默丟棄**；live executor 接受 `content`（Swift key，優先）或 `text`，startFrame/durationFrames 可省略（default: 尾端接續 / 150），且任一壞 entry 整個呼叫拒絕。對齊：三欄位改 Option、`content` 優先、顯式值域檢查（startFrame >= 0、durationFrames >= 1、#264 bounds）硬錯誤。
3. **MUT-010（text-only 欄位拒絕）暫不啟用**：gate 傳 `clip_types: None`。Swift 現行 executor **有** live 的 text-only 拒絕，但 Rust executor 行為（video clip 也可寫 text_style）被 3 個測試釘住、且是 inspector 綁定沿用的行為。啟用與否是 Swift-parity 行為決策，超出「僅接線既有檢查」的 Non-Goal，列入 Notes 待決。

### 已知衝突（重要，需後續決策）

**Inspector 音量增益 vs #144 volume 0..1**：`inspector_view::scrub_commit_args` 把音量滑桿（-60..+15 dB，鏡射 Swift `VolumeScale`）轉線性後經 **同一個** `executor.execute("set_clip_properties")` 提交——+15 dB ≈ linear 5.62 > 1。Swift 的 inspector 直改 model 不過 tool 層，所以 Swift 可同時有「agent 拒絕 >1」與「UI 可 boost」；Fronda 的 UI 共用 executor，#144 上線後 **inspector 音量 >0 dB 的 commit 會被拒**（`run_tool` 只 eprintln，UI 靜默不動）。本 change 依 proposal 明定的成功準則（volume 1.5 必拒）照 #144 接線；後續需擇一：(a) UI 走 trusted 執行入口、(b) 工具契約放寬 volume 上限到 `linear_from_db(VOLUME_CEILING_DB)` 並改 schema 文案、(c) inspector commit 夾到 0 dB（犧牲 boost）。**尚未處理，見 Notes。**

### 驗證（exit code）

1. `cargo test --workspace` → EXIT=0（54 個 test binaries 全 ok，含 agent_contract 485 lib tests；新 e2e：volume 1.5 / opacity -0.1 / speed 0 / trim 負值 / frame 超 ceiling / add_texts audio track / insert_clips 空 mediaIds / 拒絕不 bump revision）
2. `cargo check -p fronda-app-shell-gpui --features desktop-app --bin fronda` → EXIT=0
3. `spectra validate wire-mutation-validators` → EXIT=0（warn: no delta specs——本 change 僅接線，無新 requirement）

### 釘住訊息的測試更新（依指示取較清楚訊息並註記）

- `set_clip_properties_rejects_non_positive_speed`：`speed must be > 0` → validator 的 `'speed' must be positive, got 0`
- `set_clip_properties_rejects_bad_background_color`：`invalid background color` → validator 的 `'background.color' is not a valid hex color`
- `exec_034_add_texts_missing_texts`：`Missing texts array` → `missing or empty 'texts' array`
- `exec_023_create_folder_missing_name`：`Missing name` → `missing or empty 'name'`
- mutation.rs 的 `mut_006_*`（改字串 id）與 `mut_019_*`（欄位改 Option）隨對齊更新

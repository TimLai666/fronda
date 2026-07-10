## Context

上游 #263（於 upstream/main@141c69b 驗證，v0.6.2 線）把工具面整併為 48 個工具：`ToolName` enum 4（Projects）+ 6（Timelines）+ 5（Media）+ 12（Clips）+ 3（Multicam）+ 4（Transcript）+ 3（Text/Captions）+ 4（Color/Effects）+ 5（Generation）+ 2（Meta）= 48。實際曝光分兩個 host：`ToolDefinitions.all`（43 個共用工具）+ MCP-only 4 個 project 工具 = MCP 47；`all` + in-app-only `read_skill` = in-app 44。Rust 現面（`crates/agent_contract/src/tools.rs:1`，斷言 `crates/agent_contract/src/tools.rs:1319`）是 64 個工具、單一清單、無 host 區分。

本 design 是 task 1.1 的產物：完整抄錄 v2 契約（全部 48 工具 schema、mutation envelope、get_timeline v2 輸出、get_transcript v2 輸出、organize_media/manage_tracks/close_project 語意、SYSTEM_INSTRUCTION 全文），並給出 Rust 64 → v2 的逐工具分類。契約來源全部釘在 `141c69b`（上游 main 已前進到 404e14f/v0.6.4；`141c69b..404e14f` 含 #288「validate video-to-audio span in agent path」等，可能微調 generate_audio 行為——不在本 change 範圍，後續 audit 再收）。

## Goals / Non-Goals

**Goals:**

- 落地 v2 契約全文為本 change 的單一事實來源（本檔 Appendix A–C）
- 逐工具分類：SAME / RENAMED / ABSORBED / NEW / RUST-EXTENSION-KEPT，含參數對應
- 定案最終工具面數量與 host 曝光策略

**Non-Goals:**

- multicam 三工具的實作（manage_multicam / change_cam / get_multicam 屬 multicam 引擎 change；本面為其留位）
- 上游 server-side 模型目錄（保留本地 catalog）
- 上游 141c69b 之後的 commit（404e14f 前的 #288 等另案 audit）

## Decisions

### 收斂策略：名稱與 schema 全面對齊 v2，Rust 擴展疊加其上

以 upstream@141c69b 的 `ToolDefinitions.swift` 為 schema 正本（含 description 全文——上游把大量行為契約寫在 description 裡，是 prompt 工程的一部分，逐字採用）。Rust 執行器行為對齊 Appendix 記載的 executor 語意；本 design 抄錄語意，不抄 Swift 語法。

### 最終面：53 個工具（48 − 3 multicam 深後 + 8 Rust 擴展）

- 上游 48 中，multicam 三工具（manage_multicam / change_cam / get_multicam）留位不實作 → 本 change 落地 45 個上游工具。
- Rust-native 擴展保留 8 個（上游沒有的）：`duplicate_project`、`add_shapes`、`apply_animation`、`create_compound_clip`、`dissolve_compound_clip`、`save_clip_preset`、`apply_clip_preset`、`list_clip_presets`。
- **45 + 8 = 53**（multicam 落地後 56）。
- 修正 proposal 的兩處過時假設：(1) `set_blend_mode` 不是擴展——上游 `set_clip_properties` 已含 `blendMode`（含完整 16 值 BlendMode enum），故歸類 ABSORBED；(2)「timeline 工具」中 `create_timeline`/`set_active_timeline` 是上游工具（SAME），`duplicate_timeline` 被 `create_timeline.from` 吸收。

### Host 曝光：跟進上游的雙面分割

上游 `ToolDefinitions.mcpServer = all + [get_projects, open_project, new_project, close_project]`、`inAppAgent = all + [read_skill]`。Rust 目前單一清單同時曝光兩者。決定：tools.rs 增加 host 標記（shared / mcp-only / in-app-only），MCP server 與 chat panel 各取其面。Rust 擴展歸 shared。這使工具數斷言變成三個數字：shared 48（= 45 上游共用 − 4 project − 1 read_skill + 8 擴展）、MCP 52（shared + 4 project）、in-app 49（shared + read_skill）。（若使用者傾向維持單面 53，實作時只需退回單清單——不影響本契約抄錄。）

### 被吸收工具直接退場，不留 alias

15 個 Rust 工具（見分類表）刪除定義與 executor 分支；`media_panel_view` 等 UI 呼叫點改走新工具。不設 deprecated alias——agent 面沒有向後相容需求（每次 session 都重新讀 tool list），MCP 客戶端亦以 tools/list 為準。

### ripple_delete_ranges 參數更名

Rust #207 的 `ignoreSyncLockTrackIndices` 對齊上游 v2 名稱 `ignoreSyncLockedTracks`（同語意：per-call sync-lock 豁免的 track index 清單）。

### Envelope 與讀取面為行為契約，不逐位元對齊

mutation envelope、get_timeline v2、get_transcript v2 依 Appendix 的形狀與欄位語意實作（含 cap 數值：30 changed clips、200 caption rows、10000 words、3 位小數捨入、shift 群組門檻 3）。JSON key 排序與空白不在契約內。

## Implementation Contract

### C-1. Surface inventory

| Class | Count | Tools |
|---|---|---|
| SAME（名稱不變，schema 收斂到 v2） | 40 | get_projects, open_project, new_project, get_timeline, inspect_timeline, create_timeline, set_active_timeline, set_project_settings, export_project, get_media, inspect_media, search_media, import_media, add_clips, insert_clips, move_clips, remove_clips, split_clips, ripple_delete_ranges, set_clip_properties, set_keyframes, apply_layout, undo, get_transcript, remove_words, remove_silence, add_texts, update_text, add_captions, apply_color, apply_effect, inspect_color, denoise_audio, list_models, generate_video, generate_image, generate_audio, upscale_media, send_feedback, read_skill |
| RENAMED | 1 | sync_audio → **sync_clips** |
| NEW（本 change 實作） | 4 | organize_media, manage_tracks, close_project, detect_beats |
| NEW（留位，multicam change 實作） | 3 | manage_multicam, change_cam, get_multicam |
| ABSORBED（Rust 工具退場，功能併入 v2 工具） | 15 | 見 C-2 |
| RUST-EXTENSION-KEPT | 8 | duplicate_project, add_shapes, apply_animation, create_compound_clip, dissolve_compound_clip, save_clip_preset, apply_clip_preset, list_clip_presets |

Rust 現面對帳：40 SAME + 1 RENAMED + 15 ABSORBED + 8 KEPT = 64 ✓（tools.rs 斷言值）。
v2 面對帳：40 SAME + 1 RENAMED + 4 NEW + 3 DEFERRED = 48 ✓（上游 ToolName 全數）。
最終面：45 + 8 = **53**。

### C-2. Per-tool diff（Rust 64 → v2）

Absorbed 工具與參數對應：

| Rust tool（退場） | 併入 | Param mapping |
|---|---|---|
| create_folder | organize_media | `name` → `createFolders: [path]`（可含中間層級，自動建立） |
| rename_folder | organize_media | `folderId, name` → `renames: [{item: folderPath, name}]`（folder 一律以 path 定位，無 id） |
| delete_folder | organize_media | `folderId` → `deletes: [folderPath]` |
| move_to_folder | organize_media | `mediaId, folderId` → `moves: [{items: [id…], into: path}]`（omit `into` = 移到 root） |
| rename_media | organize_media | `mediaId, name` → `renames: [{item: assetId\|timelineId, name}]` |
| delete_media | organize_media | `mediaId` → `deletes: [assetId\|timelineId]`（last-timeline guard 同語意保留） |
| list_folders | get_media | 無參數讀取 → get_media 無 filter 時輸出 folders（以 path 呈現）與 timelines |
| remove_tracks | manage_tracks | `trackIds` → `remove: [trackIndex]`（**id 改 index**，呼叫時序解析見 C-7） |
| create_matte | import_media | `hex, aspectRatio, name` → `source: {matte: {hex, aspectRatio}}, name` |
| import_folder | import_media | `path` → `source: {path}`（目錄遞迴匯入、鏡射子資料夾為 media folders，inline 完成） |
| duplicate_timeline | create_timeline | `timelineId` → `from`；`name` 保留 |
| set_blend_mode | set_clip_properties | `clipId, blendMode` → `clipIds: […], blendMode`（16 值 enum，見 Appendix C；'normal' 清除） |
| set_chroma_key | apply_effect | `clipId, keyHue, tolerance, softness, spill` → `clipIds, effects: [{type: "key.chroma", params: {keyHue, tolerance, softness, spill}}]` |
| set_color_grade | apply_color | 各 knob → apply_color 同名/對應 knobs（merge 語意，`reset:true` 歸零起算） |
| generate_music | generate_audio | `prompt` 等 → generate_audio（music-category model；`lyrics`/`instrumental`/`duration` 依模型） |

SAME 工具中有實質 schema 變更的重點（完整 schema 見 Appendix A）：

- **get_timeline**：新增 `startFrame`/`endFrame`（視窗）與 `captionDetail`；輸出改 v2（見 C-5）。
- **get_media**：新增 `ids`（placeholder 輪詢）、`folder`（path filter 含子層）、`pending`。
- **inspect_media**：新增 `clipId`（transcript 轉 project frames）、`overview`（storyboard）、`wordTimestamps`、`startSeconds`/`endSeconds`、`language`。
- **import_media**：`source` 改為 exactly-one-of `{url, path, bytes, matte}` + `mimeType`；url/path 背景匯入回 `{mediaRef, status:'downloading'}`，directory/bytes/matte inline `status:'ready'`。
- **export_project**：新增 `overwrite`（預設 true）、`fcpxmlTarget`（resolve|fcp，預設 resolve）、mode 增 `palmier`；video 模式背景執行回 `status=started`。
- **set_clip_properties**：新增 `blendMode`、`transform.flipHorizontal/flipVertical`；trim 欄位語意文件化（source-offset、PROJECT frames）。
- **ripple_delete_ranges**：`ignoreSyncLockTrackIndices` → `ignoreSyncLockedTracks`。
- **remove_words**：新增 `cutAggressiveness`（tight|balanced|loose）。
- **get_transcript**：新增 `granularity`（words|segments）；輸出改 v2（見 C-6）。executor 另接受未文件化的 `wordTimestamps` key（上游 allowed-keys 殘留），不抄進 schema。
- **apply_layout**：slots 增 `anchor`/`anchorX`/`anchorY`；頂層增 `fit`（fill|fit）。
- **add_texts / update_text / add_captions**：textStyle 展平為頂層/entry 屬性（fontName, fontSize, isBold, isItalic, color, alignment, borderColor, backgroundColor）+ `animation`/`highlightColor`。
- **sync_clips**（RENAMED）：新增 `mode`（auto|audio|timecode，timecode 對齊 confidence 1.0）、`targetClipId` 單數形式。

### C-3. Cross-cutting：unknown-key rejection 與 short-id

- 幾乎所有工具以 allowed-keys 白名單拒絕未知欄位，錯誤格式：`"<path>: unknown field(s) '<k>'. Allowed: <list>."`（`validateUnknownKeys`，含巢狀 entry path 如 `entries[3]`）。
- **Short-id 契約**（ToolExecutor+ShortId.swift）：輸出中所有已知 UUID 縮短為「不與其他 id 共享的最短前綴，下限 8 字元」；id 宇宙 = 全部 timelines、clips、captionGroupIds、linkGroupIds、media assets、multicam groups+members，取「工具執行前 ∪ 執行後」聯集（新建與剛刪除的 id 都保持短形）。輸入端：已知 scalar id keys（clipId, sourceClipId, referenceClipId, targetClipId, mediaRef, startFrameMediaRef, endFrameMediaRef, sourceVideoMediaRef, videoSourceMediaRef, captionGroupId, timelineId, item, from, reference, groupId, memberId）與 array id keys（clipIds, targetClipIds, items, ids, deletes, referenceMediaRefs, referenceImageMediaRefs, referenceVideoMediaRefs, referenceAudioMediaRefs）的值若為 ≥8 字元前綴則展開為完整 id；多重匹配丟錯 `"Ambiguous id '<ref>' matches N items; re-read with get_timeline or get_media for current ids."`；無匹配放行讓工具自報 not-found。

### C-4. Mutation envelope（ToolExecutor+MutationDelta.swift）

所有 clip-mutation 工具回傳統一 JSON envelope（get_timeline 詞彙的 timeline diff）。欄位皆「非空才出現」：

```jsonc
{
  // 變更/新建 clips 的結果狀態，get_timeline clip 形狀 + "track"(index)。
  // 上限 30 筆；超過時附 "clipsNote": "Showing 30 of N changed clips — re-read get_timeline for the rest."
  "clips": [ { "id": "…", "track": 2, "frames": [120, 300], … } ],
  // ≥3 個變更 clips 共享 captionGroupId 時折疊成 group 摘要（get_timeline captionGroups 形狀 + "track"）
  "captionGroups": [ { "captionGroupId": "…", "clipCount": 42, "frameRange": [0, 1800], "shared": {…}, "textPreview": "…", "track": 0 } ],
  // 純平移（同 track、同 duration、只有 start 變）且同 (track, delta) 群組 ≥3 筆時壓縮成 rule；
  // 小於 3 筆的平移改列入 clips。排序：(track, fromFrame)。
  "shifted": [ { "track": 1, "fromFrame": 480, "by": -72, "count": 11 } ],
  "removedClipIds": [ "…" ],                     // 排序後的消失 id
  "createdTracks": [ { "index": 3, "label": "V3", "type": "video" } ],
  "notes": [ "Track indices shifted — re-read get_timeline before the next index-based call." ],
  // …工具自身的 extra keys 併入頂層（如 manage_tracks 的 "tracks"、sync_clips 的同步報告、organize_media 的計數）
}
```

實作細節（契約內）：changed = touched（仍存在者）∪ 新出現 id ∪ 非純平移的位置變更；caption 折疊門檻與 shift 群組門檻同為 3；track 清單有增刪或重排且 payload 沒帶 `tracks` 時自動附上 notes 提醒；數值捨入到 3 位小數。

**回 envelope 的工具（22）**：add_clips, insert_clips, remove_clips, move_clips, split_clips, ripple_delete_ranges, set_clip_properties, set_keyframes, manage_tracks, apply_layout, sync_clips, manage_multicam, change_cam, add_texts, update_text, add_captions, apply_color, apply_effect, denoise_audio, remove_words, remove_silence, organize_media（例外：organize_media 刪除 active timeline 導致切換時，改回純 payload + `notes:["Active timeline changed — re-read get_timeline."]`）。
**不回 envelope**：undo（純文字 `"Undid: <action>. …"`）、create_timeline / set_active_timeline（自有 payload）、所有讀取工具、import/generate/export/project 工具、send_feedback、read_skill。

### C-5. get_timeline v2 輸出形狀（ToolExecutor+Timeline.swift）

頂層：Timeline serde dict（移除 `settingsConfigured`）+：

- `totalFrames`, `durationSeconds`（totalFrames / max(fps,1)）
- `currentFrame`, `canGenerate`（signed-in && hasCredits）
- `window`: `[start, min(end, totalFrames)]`（僅視窗查詢時）
- `multicamGroups`: `[{groupId, name, angles: [label], mics: [label]}]`（僅被 timeline 引用的 groups）
- `timelines`: `[{timelineId, name, active?: true}]`（僅專案有 >1 條 timeline 時）
- 全部浮點捨入 3 位小數

Track（每軌）：`label`（顯示標籤，鏡射 video 編號）、`index`；移除 `id`、`displayHeight`；預設值剝除（muted=false, hidden=false, syncLocked=true 時不出現）；`linkedClips`（被折疊的 audio partner 數）；`gaps`: `[[start, end)…]`（非 caption clips 之間的空段，首尾不報）；`clips`（視窗內可見者，caption 之外；視窗遮蔽時附 `totalClips`）；`captionGroups`。

Clip：`frames: [start, end)` 取代 startFrame/durationFrames；預設剝除（mediaType 'video'、sourceClipType==mediaType、speed 1、volume 1、opacity 1、trims/fades 0、identity transform/crop、預設 textStyle）；text clips 一律不報 trims；`color`（grade 物件，apply_color 詞彙，可直接回貼）；`effects: [{type, params(扁平), enabled(僅 false 時)}]`（排除 color.*）；`keyframes: {prop: [[frame, …values, interp?]]}`（interp 僅非 smooth 時出現；identity 常數軌剝除；非 identity 常數軌塌縮成 static 欄位，如 `crop: {left: 0.31}`；塌縮判準 0.0005 容差）；**A/V 折疊**：linkGroupId 恰兩員（一 audio 一 visual）時，audio 併入 visual 為 `audio: {id, track, frames?(僅偏移時), trimStartFrame?, trimEndFrame?, speed?, volume?, fades?, keyframes?, effects?}`（只帶與 visual 相異者），visual 移除 linkGroupId，audio 不再單獨列於其軌。

Caption groups：同 captionGroupId 的 clips 折疊為 `{captionGroupId, clipCount, frameRange: [min,max], shared(眾數殘餘屬性，transform 去 width/height), textPreview: "first … last"(60 字截斷) + clipsNote}`；`captionDetail:true` 時改附 `clipFormat: ["clipId","startFrame","endFrame","text"]` + `clips` rows（上限 200，超出附分頁 note）；屬性偏離眾數的 caption clips 個別出現在 `clips`。

### C-6. get_transcript v2 輸出形狀（ToolExecutor+Transcription.swift）

```jsonc
{
  "fps": 30, "timing": "projectFrames",
  "transcriptionSource": "cloud" | "local",       // cloud 僅在已登入且 credits 足夠時
  "clips": [ { "clipId": "…", "trackIndex": 1, "startFrame": 0, "endFrame": 900,
               "words": [[index, "text", startFrame], …]          // granularity=words
               // 或 "segments": [[firstWordIndex, "sentence…", start, end], …]
             } ],
  "wordFormat": ["index","text","start"],          // 或 "segmentFormat": ["firstWordIndex","text","start","end"]
  "speakers": [[firstWordIndex, "name"|null], …],  // run-length；有辨識出 speaker 時才出現
  "speakersNote": "[firstWordIndex, speaker] — each run holds until the next entry.",
  "totalWords": 12345, "nextStartFrame": 9000,     // 超過 10000 words cap 時
  "wordsNote": "First 10000 of N words. Continue with startFrame = nextStartFrame.",
  "skipped": [ … ]                                  // 無法轉錄的 clips
}
```

- word index：全域、穩定、0-based、timeline 順序；clipId scope 與視窗分頁不改變 index。
- word 時長：至下一 word 的 start；最後一 word 至 clip end。
- segments 切句：speaker 變更、字距 > 1 秒（fps frames）、run ≥ 48 words、句尾標點（. ! ?）。

### C-7. 新工具語意

**organize_media**（ToolExecutor+Organize.swift）——一次 undoable 動作內 create/move/rename/delete：

- Item 判別順序：asset id → timeline id → folder path；folder 一律 path（無 id）。
- Path 解析：`/` 分段、trim 空白、逐段 case-insensitive 比對；同層同名多筆時 exact-case 優先、否則丟 ambiguous 錯誤；`resolveOrCreateFolder` 逐段建立缺失層級並回報 created paths。
- 套用順序 createFolders → moves → renames → deletes，但 item 引用一律以呼叫前的 library 解析（parse 先於 mutate）；只有 `into` 目的地可指到同呼叫新建的資料夾。
- Rename 是改名不是移動（name, not path）。
- Delete：先 assets+folders 再 timelines（clipsRemoved 計數排除被刪 timeline 自身的 clips）；刪 asset 連帶移除引用 clips（回報 clipsRemoved）；刪 folder 連刪子層與 assets、內含 timelines 移到 root；刪光 timelines 被擋（"Can't delete every timeline"）；刪除仍被 nest 引用的 timeline 附 warning（render black）。
- Move 循環防護：folder 不可移入自身或子層（parse-time path 檢查）。
- 回傳：mutation envelope + `{createdFolders, moved(計數), renamed(計數), deleted:{assets,folders,timelines}, clipsRemoved, warnings}`；若 active timeline 被刪而切換，改回 payload + notes（見 C-4）。

**manage_tracks**（ToolExecutor+Clips.swift `manageTracks`）：

- 三動作陣列依 reorder → set → remove 順序執行；**所有 index 在呼叫時序一次解析成 track id**（up front），故單一呼叫內 index 不因前面的 reorder 漂移。
- reorder 逐條 live 套用後一次 commit；`to` 夾制在同型別 zone（video 只能在 video 區內移動）。
- set：muted/hidden/syncLocked 至少一項；idempotent（與現值相同不動）。
- remove：以解析出的 id 刪軌與其上所有 clips；他軌的 linked partners 留下。
- Multicam 防護：含 multicam clips 的軌不可 remove、不可 syncLocked=false（mute/hide 可）。
- 回傳：envelope + `tracks`（新順序 `[{index, label, type, muted?, hidden?, syncLocked?(僅 false)}]`）；有 reorder/remove 時附 notes「Track indices changed — 'tracks' is the new order; index 0 renders on top.」
- 空呼叫（三陣列皆空）丟錯。

**close_project**（ToolExecutor+Projects.swift）：

- 無參數 → 關 active project（無開啟專案時丟錯）；帶 name/id/path → 解析 URL 後必須是「已開啟」的專案，否則丟 `"Project at <path> isn't open."`。
- 先存檔再關閉；存檔失敗丟 `"Couldn't save '<name>' — project left open. <err>"`（專案保持開啟）。
- 回傳 `{status:"closed", name, openCount, active?: {name, path}}`（有下一個 active 專案時）。

**detect_beats**（ToolExecutor+Beats.swift）：on-device 節拍偵測，回 beats/downbeats（SOURCE 秒）+ 估計 bpm；全檔分析一次後快取，`startSeconds`/`endSeconds` 只裁回應。

**create_timeline 吸收 duplicate**（ToolExecutor+Timeline.swift）：`from` 給 timelineId 時走 duplicate（新 id、可再 `name` 改名、note 提醒 id 全新）；無 `from` 建空 timeline（繼承 fps/resolution）。兩路都切為 active。回 `{timelineId, name, active: true, note}`。

**undo**：session 內 agent 動作名稱堆疊；最新 undo action 不是 agent 的即拒絕；成功回純文字。

### C-8. 測試與斷言更新點（後續 task 的驗收面）

- 工具數斷言：tools.rs（64 → 53/host 分割後三數）、spec_tool_snapshots.rs、mcp_server spec_mcp_contract.rs、app_shell 呼叫點——實際檔案於 task 5.2 盤點。
- envelope shape 的 serde/snapshot 測試以 C-4 為準；get_timeline/get_transcript v2 以 C-5/C-6 為準。

## Risks / Trade-offs

- **Description 即契約**：上游把行為承諾寫進 description（如 add_clips 的 overwrite 語意、insert_clips 的 ripple 範圍）。抄錄後 Rust executor 行為若與 description 不符即為 bug——task 4/5 實作時要逐句對行為。
- **A/V 折疊改變既有消費者**：Rust get_timeline 現輸出未折疊的軌面；chat panel、MCP 客戶端、既有測試都要跟著改（task 4.2 範圍）。
- **organize_media 的 path 定位**：Rust 現有 folder id 模型（create_folder 回 id）要改成 path 解析層；case-insensitive 與 ambiguity 規則是新邏輯，需要專屬單元測試。
- **短 id 契約是新機制**：Rust 目前輸出完整 UUID。若不同步實作 short-id，envelope 與 get_timeline 的 token 成本會顯著高於上游；建議與 envelope 同 task 落地（純函式，易測）。若延後，description 中「IDs are short prefixes」段落需暫時改寫——偏離全文抄錄，standing decision 需使用者確認。
- **generate_audio 吸收 generate_music** 依賴模型 catalog 有 music 類目；Rust 本地 catalog 需含對應條目，否則吸收後功能倒退。
- **上游已前進**：141c69b 後的 #288 等可能改 generate_audio 驗證行為；本契約不含，後續 audit 單獨評估。

## Appendix A — Upstream v2 tool contract (verbatim, upstream/main@141c69b `Sources/PalmierPro/Agent/Tools/ToolDefinitions.swift`)

Conventions: every input schema is `{"type": "object"}` with the listed `properties`; "required" marks members of the schema's `required` array — everything else is optional. Descriptions are transcribed verbatim (Swift line continuations joined). Swift-side enum expansions (`BlendMode.allCases`, `VideoLayout.allCases`, `LayoutFit`, `TextAnimation.Preset.agentValues`, `effectCatalog()`) are resolved in Appendix C. Host exposure: `ToolDefinitions.mcpServer = all + [get_projects, open_project, new_project, close_project]`; `ToolDefinitions.inAppAgent = all + [read_skill]`.

### A-1. Projects (MCP server only)

#### 1. `get_projects`

> List the user's known projects, most recently opened first: each entry's id, name, path, whether it's currently open, and whether it's the active project (the one editing tools act on). Also returns a top-level `active` (name, path) for the current project, which may not appear in the list. Call this to discover what's available before open_project, or to find out which project is active. Takes no arguments.

Schema: no properties. Output (executor): `{openCount, projects: [{id, name, path, isOpen, isActive, isAccessible}], active?: {name, path}}`.

#### 2. `open_project`

> Open a project and make it the active one — every editing tool then acts on it. Identify it by `name` (the natural choice when the user names a project), `id` (from get_projects), or `path` to a .palmier package. If it's already open, it's brought to front; the user sees the window change. Returns a snapshot of what you opened: fps, resolution, mediaCount, canGenerate, and the timelines list — enough to orient before get_timeline.

- `name` (string) — Project name, matched case-insensitively against known projects. Errors list candidates when ambiguous or unknown.
- `id` (string) — Project id from get_projects.
- `path` (string) — Filesystem path to a .palmier package.

Output (executor): snapshot `{status:"active", name, path, fps, resolution:"WxH", mediaCount, canGenerate, timelines, openCount}`.

#### 3. `new_project`

> Create a new empty project in the user's Palmier Pro folder and make it active. Fails if a project with that name already exists — pick another name. Optionally set fps / aspectRatio / quality at creation so the first clips land on the right canvas (same semantics as set_project_settings). Returns the same snapshot as open_project.

- `name` (string) — Project name (without extension). Defaults to 'Untitled Project'.
- `fps` (integer) — Optional timeline frame rate (1-120).
- `aspectRatio` (string, enum: `16:9`, `9:16`, `1:1`, `4:3`, `2.4:1`, `9:14`) — Optional canvas aspect ratio.
- `quality` (string, enum: `720p`, `1080p`, `2K`, `4K`) — Optional resolution preset applied to the aspect ratio.

#### 4. `close_project`

> Save and close an open project. Omit all arguments to close the active project; or identify one by name, id (from get_projects), or path. Unsaved changes are saved first. When the active project closes, the next open project becomes active (returned as `active`) — with none left, the Home window shows and editing tools need open_project/new_project again.

- `name` (string) — Project name, matched case-insensitively. Omit everything to close the active project.
- `id` (string) — Project id from get_projects.
- `path` (string) — Filesystem path to a .palmier package.

Behavior: see contract C-7.

### A-2. Timelines

#### 5. `get_timeline`

> Always call at the start of a session. Returns project settings (fps, resolution, totalFrames, durationSeconds), tracks with their index (what every trackIndex parameter takes), type, and clips, plus canGenerate (if false, generation/upscale tools will fail — tell the user to sign in to Palmier and subscribe before attempting them). The clipId values here are what every other tool accepts.
>
> Every clip occupies frames: [start, end) — timeline frames, end exclusive, duration = end − start. gaps on a track lists its empty [start, end) spans; no gaps key means contiguous. A video clip's linked audio partner is folded into it as audio: {id, track, …} carrying only what deviates (volume, effects, differing trims); the partner is not repeated on its own track, which instead reports linkedClips (its folded count). Address the audio side by its nested id.
>
> Fields equal to their defaults are omitted: mediaType 'video', sourceClipType = mediaType, speed 1, volume 1, opacity 1, trims/fades 0, identity transform/crop, default textStyle, track muted/hidden false. Text clips never report trims. Keyframe tracks that animate nothing are shown as what they are: identity tracks are dropped, constant ones appear as the static field (e.g. crop: {left: 0.31}). A graded clip carries `color` — its grade in apply_color's own vocabulary, pasteable to other clips via apply_color's color parameter. Other effects appear as effects: [{type, params}], the exact shape apply_effect accepts.
>
> Caption clips (sharing a captionGroupId) come back per track as captionGroups summaries: clipCount, frameRange, shared style, and a textPreview — individual caption clips and their ids are NOT listed. That summary is all you need to restyle (update_text with captionGroupId) or judge coverage; the spoken words live in get_transcript. Only when you must touch individual caption clips (retime one, delete one, fix one word's style), re-read with captionDetail:true — ideally windowed — to get [clipId, startFrame, endFrame, text] rows, capped at 200 per group. Caption clips whose properties deviate from the group always appear individually in clips.

- `startFrame` (integer) — Optional. Window start (inclusive); only clips intersecting [startFrame, endFrame) are returned. Tracks report totalClips when the window hides some.
- `endFrame` (integer) — Optional. Window end (exclusive).
- `captionDetail` (boolean) — Optional. true expands captionGroups into per-clip [clipId, startFrame, endFrame, text] rows. Combine with a window; only needed to edit individual caption clips.

Output: see contract C-5.

#### 6. `inspect_timeline`

> See the composited timeline — what the user actually sees in the preview at a given frame: all video tracks stacked with their transforms, opacity, crop, and keyframes applied, plus text and caption overlays baked in. Use this to verify your edits landed (a PIP's position, a title's placement, layer order) — inspect_media shows the raw source asset, not the cut.
>
> Frames are project frames (from get_timeline). Pass a single startFrame for one composited frame; add endFrame to sample maxFrames evenly across [startFrame, endFrame) for a transition or sequence. Frames past content render black. Each image carries its frame number burned into the top-left (f157), and the metadata lists, per rendered frame, the clip ids visible on screen top-down (caption clips as their captionGroupId) — so what you see maps straight back to the clips to edit.

- `startFrame` (integer) — Project frame to render (default 0). With no endFrame, a single frame is returned.
- `endFrame` (integer) — Optional. Sample maxFrames evenly across [startFrame, endFrame) instead of one frame.
- `maxFrames` (integer) — Frames to sample when endFrame is set (default 6, max 12).

#### 7. `create_timeline`

> Creates a timeline and switches to it — every read and edit tool now targets it. Without 'from', the new timeline is empty and inherits fps/resolution from the previously active one. With 'from', it's a full copy of that timeline — the versioning primitive: copy, then edit the copy ("a tighter cut", "a 9:16 version") while the original stays intact; every clip and track id in the copy is NEW, so re-read get_timeline before editing. Undoable.
>
> Use timelines to organize a project: alternate versions, sections assembled separately, or reusable groups. A timeline can be placed inside another as a single clip (add_clips with the timelineId as mediaRef); it then appears as a clip with mediaType 'sequence'.

- `name` (string) — Optional display name. Defaults to 'Timeline N', or '\<source> copy' when duplicating.
- `from` (string) — Optional timelineId to duplicate instead of creating empty.

#### 8. `set_active_timeline`

> Switches the active timeline — the one every read and edit tool targets and the one the user sees. get_media lists the project's timelines (with timelineId). Always re-read get_timeline after switching; clip and track ids from the previous timeline are no longer valid targets.
>
> To edit the contents of a nested timeline (a clip with mediaType 'sequence'), switch to its mediaRef.

- `timelineId` (string, **required**) — Timeline id from get_media's timelines list (or a sequence clip's mediaRef).

Output: `{timelineId, name, active: true, totalFrames, fps, trackCount, note}`.

#### 9. `set_project_settings`

> Change the project's frame rate, resolution, or aspect ratio. Pass any combination of fps, explicit width+height, aspectRatio, and quality. aspectRatio and explicit width/height are mutually exclusive; quality scales the current aspect ratio (or the selected preset when combined with aspectRatio). The timeline's existing clips are re-fitted automatically: auto-fit transforms recalculate for the new canvas size, and all frame positions/durations rescale when fps changes. Undoable.

- `fps` (integer) — Frame rate in frames per second. Common values: 24, 25, 30, 48, 50, 60.
- `width` (integer) — Canvas width in pixels. Use with height for an exact resolution. Mutually exclusive with aspectRatio.
- `height` (integer) — Canvas height in pixels. Use with width for an exact resolution. Mutually exclusive with aspectRatio.
- `aspectRatio` (string, enum: `16:9`, `9:16`, `1:1`, `4:3`, `2.4:1`, `9:14`) — Preset aspect ratio — sets both width and height from the preset, or combined with quality to pick a specific size. Mutually exclusive with width/height.
- `quality` (string, enum: `720p`, `1080p`, `2K`, `4K`) — Resolution quality preset — scales the short edge to the target while preserving the current (or specified) aspect ratio.

#### 10. `export_project`

> Exports from the current project using the same modes as the Export dialog. mode defaults to video. video renders H.264, H.265, or ProRes; xml writes XMEML timeline XML; fcpxml writes FCPXML; palmier writes a self-contained .palmier project package. For timeline interchange, pick the format by the target editor: Premiere Pro -> xml; DaVinci Resolve or Final Cut Pro -> fcpxml (fcpxml also carries text, transforms, crop, opacity, and keyframes that xml cannot). Omit outputPath to write a unique file to ~/Downloads. Existing direct outputPath files are overwritten by default to match the UI save flow; pass overwrite=false to refuse. video renders in the background and returns status=started with the destination path; the app posts a system notification on completion or failure, so do not expect a final result inline. xml, fcpxml, and palmier finish before returning and report their result inline.

- `mode` (string, enum: `video`, `xml`, `fcpxml`, `palmier`) — Optional. Default video. Use xml for Premiere Pro, fcpxml for DaVinci Resolve or Final Cut Pro.
- `codec` (string, enum: `H.264`, `H.265`, `ProRes`) — Video mode only. Optional. Default H.264.
- `resolution` (string, enum: `720p`, `1080p`, `2K`, `4K`, `Match Timeline`) — Video mode only. Optional. Default Match Timeline.
- `outputPath` (string) — Optional. Absolute destination path. If omitted, a unique project-named file is written to ~/Downloads. If no extension is provided, the mode's extension is appended.
- `overwrite` (boolean) — Optional. Default true, matching the UI save flow. false refuses when outputPath already exists.
- `fcpxmlTarget` (string, enum: `resolve`, `fcp`) — fcpxml mode only. Optional, default resolve. Davinci Resolve and Final Cut interpret crop and position values differently; pick the app the file will be imported into.
- `timelineId` (string) — Optional. Timeline to export (from get_timeline's timelines list). Defaults to the active timeline. Not valid for palmier mode, which packages every timeline.

### A-3. Media library

#### 11. `get_media`

> The library inventory: media assets, folders, and timelines. Call before referencing any asset — every mediaRef in other tools comes from the asset ids returned here. Assets report name, type, durationSeconds, width/height/fps, hasAudio, folder path, and (for AI-generated assets) the generation prompt as a content hint. generationStatus appears only while an async generation/import is unresolved (preparing | generating | downloading | failed) — its absence means the asset is ready.
>
> Filters: ids (poll specific placeholders cheaply), folder (a path; includes subfolders), pending:true (only unresolved generations/imports). Filtered reads return just the matching assets; unfiltered reads also include folders (as paths) and timelines.

- `ids` (array of string) — Optional. Return only these asset ids — the cheap way to poll a generation placeholder.
- `folder` (string) — Optional folder path filter, e.g. 'B-roll/Sunset'. Includes subfolders.
- `pending` (boolean) — Optional. true returns only assets with an unresolved generationStatus.

#### 12. `inspect_media`

> Look at a media asset before referencing or editing it. Images: the image plus dimensions and EXIF. Video: sample frames plus a transcription of the audio track. Audio: transcription. Lottie: frames sampled evenly across the animation (over gray), plus framerate and duration — use this to verify a Lottie you wrote looks and moves right. Transcription is sentence-level segments — [text, start, end] tuples, capped at 400 — in source seconds, or project frames when clipId is set. When capped, pass the returned nextStartSeconds as startSeconds for the next page.
>
> Long media: pass overview=true for a one-image storyboard, read the segments, then re-call with startSeconds/endSeconds to zoom — windowed calls only transcribe that span, so they are fast.

- `mediaRef` (string, **required**) — Asset ID from get_media.
- `clipId` (string) — Optional. A clip referencing this mediaRef; transcript times come back as project frames for that clip (out-of-range entries dropped).
- `maxFrames` (integer) — Video and Lottie. Sample frame count (default 6, max 12).
- `startSeconds` (number) — Video/audio. Source-time window start; scopes frames and transcription.
- `endSeconds` (number) — Video/audio. Window end (default: asset duration).
- `wordTimestamps` (boolean) — Video/audio. Add word-level [text, start, end] tuples (capped at 10000 — most clips return all words at once; narrow with startSeconds/endSeconds only for very long media). Use for word-boundary edits like filler-word removal.
- `overview` (boolean) — Video only. One storyboard grid of visually distinct, timestamped moments instead of frames — far more coverage per token; few tiles means static footage. maxFrames ignored.
- `language` (string) — Optional BCP-47 language tag of the spoken audio (e.g. 'es', 'fr', 'ja', 'zh-Hans'). Defaults to the system language. Specify when the spoken language differs from the system locale — on-device models are language-specific and will produce garbled output if the wrong language is used.

#### 13. `search_media`

> Search the media library by content: what's on screen (visual) and what's said (spoken). Visual matching is semantic and on-device — phrase the query like an image caption ('a wide shot of a harbor at sunset'), not keywords; covers videos and stills. Spoken matching layers exact keywords over on-device semantic matching of transcript segments — quote the words said, or paraphrase them; transcripts are created automatically while indexing (and by inspect_media and add_captions), so coverage grows as indexing completes. The two groups rank independently and are never blended. Scores are uncalibrated — use them for ordering only.
>
> Hits are source-second ranges (image hits have no time range). To place exactly that moment, pass [startSeconds, endSeconds] straight to add_clips as source — no unit conversion.
>
> An `index` object appears only while it can explain missing results (status: indexing | modelNotInstalled | downloadingModel | preparing | disabled | failed, with indexedAssets vs indexableAssets). When present, moments may be incomplete — report that instead of concluding the footage doesn't exist, and don't poll in a loop. No index key means visual search was complete. Spoken results work regardless.

- `query` (string, **required**) — What to find. Visual: a caption-style scene description. Spoken: the words to match.
- `scope` (string, enum: `visual`, `spoken`, `both`) — Optional. Default both.
- `mediaRef` (string) — Optional. Restrict the search to one asset from get_media.
- `limit` (integer) — Optional. Max hits per group (default 10, max 50).

#### 14. `import_media`

> Imports external media into the project's library — the bridge for assets coming from other MCP servers (stock libraries, music services, web search) or local files the user already has. The 'source' object must set exactly one of: url (HTTPS only — downloaded in the background, the dominant case; max 1 GB), path (absolute local file path — copied into the project in the background; may also be a directory, which is imported recursively, mirroring its subfolder structure as media folders), bytes (base64-encoded inline data — max ~15 MB of base64 ≈ 11 MB binary; use url/path for anything larger), or matte (a generated solid-color PNG). For url, type is inferred from the URL path's file extension unless source.mimeType is set as an override (needed for signed URLs whose path has no usable extension). For bytes, source.mimeType is required.
>
> Supported types and extensions: video (mov, mp4, m4v), audio (mp3, wav, aac, m4a, aiff, aifc, flac), image (png, jpg, jpeg, tiff, heic). Anything else is rejected — the caller must transcode externally.
>
> url and file-path imports run in the background and return {mediaRef, status:'downloading'} — poll get_media with ids:[mediaRef] until generationStatus clears, then the asset is usable in add_clips. Directory, bytes, and matte imports finish inline with status:'ready'. Costs nothing.

- `source` (object, **required**) — Exactly one of url, path, bytes, or matte must be set. mimeType is required when bytes is set; for url it acts as a type-inference override.
  - `url` (string) — HTTPS URL. Pre-signed URLs are fine but must not expire mid-download.
  - `path` (string) — Absolute local file or directory path, readable by the Palmier process. A directory is imported recursively — every openable file is pulled in and the folder structure is replicated as media folders.
  - `bytes` (string) — Base64-encoded media data. Prefer url or path for anything over ~10MB.
  - `matte` (object; required key: `hex`) — Generates a solid-color PNG matte instead of importing a file.
    - `hex` (string) — Hex color, e.g. '#000000' or '#FFFFFF'.
    - `aspectRatio` (string, enum: `Project`, `16:9`, `9:16`, `1:1`, `4:3`, `9:14`, `2.4:1`) — Defaults to Project (timeline resolution). Other values use the project's short edge.
  - `mimeType` (string) — Required when bytes is set. Optional override for url when its path has no usable extension (e.g. signed URLs). Accepted: video/mp4, video/quicktime, audio/mpeg, audio/wav, audio/aac, audio/mp4, image/png, image/jpeg, image/tiff, image/heic.
- `name` (string) — Display name in the library. Defaults to the filename derived from url/path, or 'Imported asset' for bytes.
- `folder` (string) — Optional destination folder path, e.g. 'B-roll/Sunset'. Created if missing. Omit for the project root.

#### 15. `organize_media`

> Reorganizes the library in one undoable action: create folders, move items into folders, rename items, delete items. An item is a media asset id (from get_media), a timelineId, or a folder path like 'B-roll/Sunset' — the tool tells them apart. Folders are always addressed by path, never by id; destination paths are created if missing. Arrays apply in order (createFolders, moves, renames, deletes), but item references resolve against the library as it was before the call — only 'into' destinations may name folders the same call creates.
>
> Deleting an asset also removes every clip referencing it (reported as clipsRemoved). Deleting a folder deletes its subfolders and assets; timelines inside move to the root instead. Deleting a timeline leaves nest clips referencing it rendering black (a warning reports how many); the last remaining timeline can't be deleted. Returns only what actually happened — createdFolders, moved, renamed, deleted, clipsRemoved, warnings.

- `createFolders` (array of string) — Folder paths to ensure exist, e.g. ['Hero shots/Takes']. Existing folders are left alone. Rarely needed — moves and generation 'folder' params create folders on their own.
- `moves` (array of object) — Each entry files items into one destination folder.
  - `items` (array of string, **required**) — Asset ids, timeline ids, and/or folder paths to move.
  - `into` (string) — Destination folder path; created if missing. Omit to move to the project root.
- `renames` (array of object; required keys: `item`, `name`)
  - `item` (string) — Asset id, timeline id, or folder path.
  - `name` (string) — New display name (a name, not a path — renaming never moves).
- `deletes` (array of string) — Asset ids, timeline ids, and/or folder paths to delete.

Behavior: see contract C-7.

### A-4. Clips

#### 16. `manage_tracks`

> Track-level operations in one undoable action: reorder (stacking order — index 0 renders on top; a video track can only move within the video zone, audio within audio), set flags (muted silences an audio track; hidden excludes a video track from the render; syncLocked controls whether ripple edits shift it), and remove (deletes tracks with every clip on them; linked partners on OTHER tracks stay). Arrays run reorder → set → remove; every index refers to the track order at call time (resolved up front). Returns the resulting track order — remaining indexes shift after reorder/remove. Tracks holding multicam clips can't be removed or sync-unlocked (mute/hide stay free).

- `reorder` (array of object; required keys: `index`, `to`) — Moves, applied in order. Use to fix stacking, e.g. bring a PIP inset's track to index 0.
  - `index` (integer) — Track to move (0-based, current order).
  - `to` (integer) — Destination index; clamped to the track's type zone.
- `set` (array of object; required key: `index`)
  - `index` (integer) — Track to change (0-based, current order).
  - `muted` (boolean) — Silence/unsilence the track's audio.
  - `hidden` (boolean) — Exclude/include a video track in the render.
  - `syncLocked` (boolean) — Whether ripple edits shift this track along.
- `remove` (array of integer) — Track indexes to remove, with all their clips.

Behavior: see contract C-7.

#### 17. `add_clips`

> Places one or more media assets on the timeline as a single undoable action. Each entry's asset type must be compatible with its target track (video/image are interchangeable across video/image tracks; audio requires an audio track). When a video asset with audio is placed on a video track, a linked audio clip is automatically created on an audio track (an existing one if available, otherwise a new one). The whole batch is one undo step.
>
> trackIndex is optional. Omit it on all entries and the tool auto-creates the needed tracks — one shared video track for visual entries and one shared audio track for audio entries (matches the captioning pattern in add_texts). To target existing tracks, set trackIndex on every entry. Mixing (some entries specify, others omit) is rejected — split into two calls.
>
> Tracks work as layers: clips on the SAME track are sequential — if a new clip's range overlaps an existing clip on that track, the existing clip is trimmed/split/removed to make room, matching the UI's drag-onto-track overwrite behavior.
>
> NESTING: mediaRef may also be a timelineId — the timeline is placed as a single live nested clip (mediaType 'sequence'), with a linked audio clip when the child has audio. Duration defaults to the child's full length; source and endFrame work as for video. Cycles (a timeline containing itself) and empty timelines are rejected.

- `entries` (array of object, **required**; required keys: `mediaRef`, `startFrame`) — Clips to add. Each entry is validated up front; one bad entry rejects the whole call with no partial state.
  - `mediaRef` (string) — ID of the media asset from get_media
  - `trackIndex` (integer) — Optional. Track index (0-based). Omit on every entry to auto-create one shared track per asset zone (video/audio).
  - `startFrame` (integer) — Timeline frame position to place the clip (project frames).
  - `endFrame` (integer) — Optional. Occupy timeline frames [startFrame, endFrame) — a gap from get_timeline copies straight in. For stills and frame-exact fills. Mutually exclusive with source.
  - `source` (array of number) — Optional. [startSeconds, endSeconds] — which span of the source to use, in the source seconds search_media hits and inspect_media segments speak. For stills this is the display length in seconds. Omit both for the whole asset. Mutually exclusive with endFrame.

#### 18. `insert_clips`

> Inserts one or more media assets at a single point and RIPPLES: every clip at or after atFrame is pushed right to open a gap, so nothing is overwritten. This is the non-destructive counterpart to add_clips (which clears the landing region, trimming/splitting/removing whatever's there). Use insert_clips to splice footage in without losing existing clips; use add_clips to fill empty space or deliberately overwrite.
>
> Entries are laid end-to-end starting at atFrame on the target track (entry[0] at atFrame, entry[1] immediately after, ...). The push equals the sum of the entries' durations and is applied to the target track, every sync-locked track, AND the audio track any auto-created linked audio lands on — so a clip and its linked audio stay aligned. As in add_clips, a video asset with audio spawns a linked audio clip. One undoable action; one bad entry rejects the whole call with no partial state.
>
> trackIndex is required — ripple needs an existing track to push. For placement into empty space, use add_clips.
>
> As in add_clips, mediaRef may be a timelineId to splice in a nested timeline.

- `trackIndex` (integer, **required**) — Track index (0-based, from get_timeline) to insert into and ripple.
- `atFrame` (integer, **required**) — Timeline frame (project frames) where insertion begins. Every clip at or after this frame on rippled tracks shifts right by the total inserted duration.
- `entries` (array of object, **required**; required key: `mediaRef`) — Clips to insert, placed sequentially from atFrame. Validated up front; one bad entry rejects the whole call.
  - `mediaRef` (string) — ID of the media asset from get_media.
  - `source` (array of number) — Optional. [startSeconds, endSeconds] — which span of the source to use, in source seconds; for stills, the display length. Omit for the whole asset. Mutually exclusive with durationFrames.
  - `durationFrames` (integer) — Optional. Exact length in project frames (entries stack end-to-end, so they have lengths, not positions). Mutually exclusive with source.

#### 19. `move_clips`

> Moves one or more clips to a new track and/or frame position. Single undoable action. Each move specifies the clip ID and at least one of toTrack (must be compatible with the clip's media type) and toFrame. Overlap on the destination is resolved as in add_clips (existing clips on the destination track are trimmed/split/removed). Linked partners follow the named clip: startFrame propagates as a delta to preserve l-cut / j-cut offsets; tracks stay with the named clip. Multicam clips must move as a whole group; partial group moves and camera lane changes are refused.

- `moves` (array of object, **required**; required key: `clipId`) — Per-clip move requests. At least one of toTrack or toFrame is required per entry.
  - `clipId` (string) — The clip ID to move.
  - `toTrack` (integer) — Destination track index (0-based). Omit to keep the clip on its current track.
  - `toFrame` (integer) — Destination start frame. Omit to keep the clip at its current start.

#### 20. `remove_clips`

> Removes one or more clips by ID as a single undoable action. Any clip that belongs to a link group (e.g. a video with its paired audio) takes its whole group with it, matching the UI's linked-delete behavior.

- `clipIds` (array of string, **required**) — Clip IDs to remove.

#### 21. `split_clips`

> Splits clips into two at one or more cut points, all in a single undoable action. A split only inserts a boundary — it never trims media or moves clips, so unlike ripple_delete_ranges nothing shifts and there's no gap to close.
>
> Two modes — pass exactly one:
> • splits: an array of {clipId, atFrame} (project frames). Use when you know the clip IDs.
> • trackIndex + frames: cut one track at the given project frames; each frame is matched to whichever clip on that track contains it. Pairs naturally with get_transcript / get_timeline project frames.
>
> Every frame must fall strictly between a clip's start and end. Multiple cuts on the SAME clip are allowed — pass all the frames at once and each is resolved against the current sub-clips. Duplicate cut points are ignored. Linked audio/video partners are split at the same frame so A/V stays in sync, and the right halves are regrouped into their own link pair. One bad cut point rejects the whole call with no partial state.

- `splits` (array of object; required keys: `clipId`, `atFrame`) — Explicit cuts. Each item is {clipId, atFrame}.
  - `clipId` (string) — The clip ID to split
  - `atFrame` (integer) — Project frame to split at (strictly between clip start and end)
- `trackIndex` (integer) — Track to cut (use with 'frames')
- `frames` (array of integer) — Project frames to cut on trackIndex; each is matched to the clip containing it.

(No top-level required array.)

#### 22. `ripple_delete_ranges`

> Cuts one or more ranges out and closes the gaps in one undoable action — the fast path for filler-word/dead-air removal. Replaces hand-cranked split_clips → remove_clips → move_clips loops: pass every range at once.
>
> Two modes — pass exactly one of clipId or trackIndex:
> • trackIndex (preferred for transcript-driven cuts): ranges are PROJECT frames and may span any number of clips on that track. get_transcript returns a clips array with nested words in project frames — collect every cut across the whole timeline and pass them in ONE call, no per-clip splitting and no re-reading the timeline between cuts. units must be 'frames'.
> • clipId: ranges are cut within that single clip only, clamped to its visible span. Allows units 'seconds' (source-media seconds, e.g. inspect_media WITHOUT a clipId or search_media hits); 'frames' = project frames. Use when you already have one clip's per-word timestamps.
>
> Overlapping ranges merge. Linked audio/video partners of every touched clip are cut on the same span so A/V stays in sync. Remaining clips shift left to close every gap; sync-locked tracks shift along to preserve alignment (their content isn't cut). Refuses without changing anything if a sync-locked track can't absorb the shift (e.g. it would move past frame 0). The refusal names the blocking track (e.g. "V2") — map it to its index via get_timeline and pass that index in ignoreSyncLockedTracks to cut anyway, leaving that track's clips in place. Returns the anchor track's post-cut layout (clip ids/frames) so you don't need to re-read.

- `trackIndex` (integer) — Cut project-frame ranges spanning every clip they cross on this track, in one call. From get_transcript's clips array. Mutually exclusive with clipId; requires units 'frames'.
- `clipId` (string) — Cut ranges within this single clip only, clamped to its visible span. Mutually exclusive with trackIndex.
- `ranges` (array of [number, number], **required**; minItems 2 / maxItems 2 per pair) — Ranges to remove, each a [start, end] pair (end > start). In the unit given by 'units'.
- `units` (string, enum: `seconds`, `frames`) — Interpretation of range values. 'frames' (default) = project/timeline frames, matching get_transcript and inspect_media-with-clipId. 'seconds' = source-media seconds (clipId mode only).
- `ignoreSyncLockedTracks` (array of integer) — Track indices to exempt from sync-lock for this call only. Their clips stay put instead of shifting to close the gap. Use to get past a refusal naming a sync-locked overlay track (e.g. a text track that can't absorb the shift) when the cut doesn't touch that track's content.

#### 23. `set_clip_properties`

> Apply the same generic clip property values to one or more clips in a single undoable action. Pass any combination of durationFrames, trimStartFrame, trimEndFrame, speed, volume, opacity, transform, or blendMode (video/image clips only). For text content, typography, captions, and text animation, use update_text.
>
> NOT for preview layout — split screen, picture-in-picture, grid, sidebar, and any multi-clip canvas arrangement belong to apply_layout, which sets transform and crop together. Do not use transform here (or set_keyframes position/scale/crop) to build those layouts.
>
> All values apply to every clip in clipIds; for per-clip differences, make separate calls. trimStartFrame/trimEndFrame are offsets from the source media, not the timeline. speed 1.0 is normal, <1.0 slows (clip gets longer on the timeline), >1.0 speeds up. volume and opacity are 0.0–1.0. transform is for rare single-clip tweaks only — 0–1 normalized canvas coords, partial merge; flipHorizontal/flipVertical mirror across the axis.
>
> For moves and start-frame changes, use move_clips. For animated values (keyframes), use set_keyframes — setting volume or opacity here clears any existing keyframe track on that property.
>
> Timing changes (durationFrames, trimStartFrame, trimEndFrame, speed) on a linked clip carry over to its linked partner so audio/video stay in sync — same as the timeline UI. Per-clip fields (volume, opacity, transform, blendMode) don't propagate. trim and speed are skipped for text partners.
>
> Timing fields (trims, durationFrames, speed) are refused on multicam clips — they would slip the clip out of sync; property fields stay editable, and angle changes go through change_cam.

- `clipIds` (array of string, **required**) — Clip IDs to update. The property values below apply to every clip in this list.
- `durationFrames` (integer) — New duration in frames.
- `trimStartFrame` (integer) — SOURCE-media offset, NOT a timeline frame: frames trimmed off the start of the source — measured in PROJECT frames (the timeline's fps, same units as startFrame/durationFrames; never the source's own fps). To turn a get_transcript project frame P into this clip's source offset, use trimStartFrame + (P − startFrame) × speed; setting trimStartFrame to that value makes the clip begin at P's source content.
- `trimEndFrame` (integer) — SOURCE-media offset, NOT a timeline frame: frames trimmed off the end of the source, in PROJECT frames. Maps the same way as trimStartFrame via startFrame/speed.
- `speed` (number) — Playback speed multiplier (default 1.0). >1 speeds up, <1 slows down. The clip's timeline length is rescaled to keep the same source content (2x speed → half the frames), unless you also pass durationFrames to set the length explicitly.
- `volume` (number) — Volume 0.0-1.0. Clears any existing volume keyframes.
- `opacity` (number) — Opacity 0.0-1.0. Clears any existing opacity keyframes.
- `transform` (object) — Single-clip only — not for split screen, PIP, or grid (use apply_layout). Partial transform: centerX, centerY, width, height, flipHorizontal, flipVertical; omitted fields keep current value.
  - `centerX` (number), `centerY` (number), `width` (number), `height` (number)
  - `flipHorizontal` (boolean) — Mirror across the vertical axis.
  - `flipVertical` (boolean) — Mirror across the horizontal axis.
- `blendMode` (string, enum: the 16 `BlendMode` raw values — see Appendix C) — Video/image clips only. How the clip composites over the tracks below it (Premiere/Photoshop blend modes). 'normal' is the default (source-over) and clears any blend. Rejected on text/audio clips.

#### 24. `set_keyframes`

> Set animated keyframes on one property of one clip. Replaces the existing keyframe track for that property (pass an empty array to clear). Frames are CLIP-RELATIVE offsets (0 = first frame of the clip), so keyframes follow the clip when it moves. Rows are sorted by frame internally and the LAST row for any duplicate frame wins. Values must be finite numbers. Each row is `[frame, ...values, interp?]` where interp ∈ {linear, hold, smooth} (default smooth).
>
> Properties and their value layouts:
> • volume `[frame, value]` — value 0.0–1.0
> • opacity `[frame, value]` — value 0.0–1.0
> • rotation `[frame, degrees]` — clockwise degrees
> • position `[frame, topLeftX, topLeftY]` — TOP-LEFT corner in 0–1 normalized canvas coords. NOT the center. (Default static transform centers a full-canvas clip, so top-left of the static is (0, 0); a centered half-size clip has top-left (0.25, 0.25).)
> • scale `[frame, width, height]` — clip's normalized width and height in 0–1 canvas coords (1.0 = fills the canvas axis). NOT a scale factor.
> • crop `[frame, top, right, bottom, left]` — side insets in 0–1 of the source media.
>
> Motion keyframes (position/scale/rotation) override the static `transform` value when active.

- `clipId` (string, **required**) — The clip ID.
- `property` (string, **required**, enum: `volume`, `opacity`, `rotation`, `position`, `scale`, `crop`) — Which property's keyframe track to set.
- `keyframes` (array of array, **required**) — Replacement keyframe rows. Empty array clears the track. Row shape depends on property — see tool description.

#### 25. `apply_layout`

> Arrange multiple clips into a common multi-video layout (split screen, picture-in-picture, grid) in one undoable action — the fast path for composing several videos in one frame. Use this instead of hand-setting transforms and screenshot-checking alignment with inspect_timeline.
>
> You pick a named layout and assign a clip to each of its slots; the tool computes every transform and crop so each clip FILLS its region edge-to-edge WITHOUT stretching — the source is cropped to the slot's shape (cover), like a layout template the videos are dropped into. Pass fit='fit' to letterbox the whole source inside its slot instead (no crop, may leave bars) — use only when the full frame must stay visible (e.g. a screen recording).
>
> The crop is centered by default. When that chops off something important (a face cropped at the forehead, a subject off to one side), bias which part survives: 'anchor' is a coarse shortcut ('top' keeps the top, etc.), while anchorX/anchorY (0–1) give continuous control for in-between framing — e.g. anchorY 0.35 moves the crop only slightly toward the top, not all the way. To nudge framing after the fact, call apply_layout again with adjusted anchorX/anchorY (clipIds mode re-crops in place).
>
> Two modes (don't mix across slots):
> • Place new clips: give each slot a 'mediaRef' (from get_media) plus top-level startFrame (default 0) and endFrame. Creates one stacked video track per slot at that time range; for PIP the inset is placed on top automatically. Video clips bring their linked audio.
> • Re-layout existing clips: give each slot 'clipIds' — one or more existing clips, all framed into that slot (handy when a track holds several sequential takes). Only transforms/crop change — timing and tracks are untouched (so existing track order decides stacking).
>
> Every slot of the chosen layout must be filled. Layouts and their slot names:
> • full — main
> • side_by_side — left, right
> • top_bottom — top, bottom
> • pip_bottom_right / pip_bottom_left / pip_top_right / pip_top_left — main, inset
> • grid_2x2 — top_left, top_right, bottom_left, bottom_right
> • main_sidebar — main (70%), sidebar (30%)
> • three_up — left, center, right

- `layout` (string, **required**, enum: the 10 `VideoLayout` raw values — see Appendix C) — Which layout template to apply.
- `slots` (array of object, **required**; required key: `slot`) — One entry per slot of the chosen layout. Each entry names a 'slot' and gives exactly one of 'mediaRef' (place a new clip) or 'clipIds' (re-layout existing clip(s) into that slot). Don't mix placement (mediaRef) with re-layout (clipIds) across slots.
  - `slot` (string) — Slot name for the chosen layout (e.g. 'left', 'inset', 'top_right').
  - `mediaRef` (string) — Asset ID from get_media to place into this slot. Use this OR clipIds.
  - `clipIds` (array of string) — Existing clip(s) to frame into this slot — every listed clip gets this slot's transform/crop (pass one id for a single clip, or several when a track holds sequential takes). Use this OR mediaRef. Clips sharing a slot may sit on the same track; clips in DIFFERENT slots still must not overlap on one track.
  - `anchor` (string, enum: `center`, `top`, `bottom`, `left`, `right`, `top_left`, `top_right`, `bottom_left`, `bottom_right`) — Coarse shortcut for which part of the source to keep when cover-cropping (default center). For in-between framing use anchorX/anchorY instead — the named values are just shortcuts for them.
  - `anchorX` (number) — Fine horizontal framing, 0–1: 0 keeps the left edge, 0.5 centers (default), 1 keeps the right. Only affects slots cropped horizontally. Overrides anchor's x.
  - `anchorY` (number) — Fine vertical framing, 0–1: 0 keeps the top (e.g. a forehead), 0.5 centers (default), 1 keeps the bottom. Nudge by small amounts (e.g. 0.35) to move the crop gradually. Only affects slots cropped vertically. Overrides anchor's y.
- `startFrame` (integer) — Placement mode only (mediaRef slots). Project frame where the layout begins. Default 0.
- `endFrame` (integer) — Placement mode only (mediaRef slots). The placed clips occupy [startFrame, endFrame). Required when placing new clips.
- `fit` (string, enum: `fill`, `fit`) — How each clip fills its slot. 'fill' (default) covers the slot and center-crops the source (no stretch). 'fit' letterboxes the whole source inside the slot.

#### 26. `sync_clips`

> Align one or more clips to a reference clip by shifting targets on the timeline — use for dual-system sound (camera + external audio) or multicam. Default mode 'auto' aligns by embedded source timecode when both files carry one (exact, confidence 1.0), falling back to audio cross-correlation otherwise (seeded by capture dates when present); force a method with mode. referenceClipId stays put unless a target would land before frame 0, in which case the whole group shifts right together (reported as shiftedFrames). Returns offsetFrames, confidence (0–1), and method (timecode|audio) per target; refuses weak audio matches. Refused on multicam clips — a group's members are already aligned by its sync maps (manage_multicam).

- `referenceClipId` (string, **required**) — Clip the others align to. Stays put.
- `targetClipId` (string) — Single clip to align. Use targetClipIds for several.
- `targetClipIds` (array of string) — Clips to align with the reference.
- `mode` (string, enum: `auto`, `audio`, `timecode`) — auto (default): timecode when available, else audio. audio/timecode force that method.
- `searchWindowSeconds` (number) — Max ± offset to search in seconds, audio mode only (default 30).
- `minConfidence` (number) — Minimum audio correlation confidence 0–1 (default 0.5).

#### 27. `undo`

> Reverts the assistant's most recent timeline edit (a cut, move, trim, split, or clip/text/caption add) as one step. The recovery path when an edit went too far — e.g. a ripple_delete_ranges removed more than intended. Verify a cut first (get_transcript reflects the post-cut audio), then undo if it overshot, then retry with corrected ranges.
>
> Undoes only edits the assistant made this session, most-recent-first — it never touches the user's own manual edits, and refuses if the latest change wasn't the assistant's. After undoing, the timeline is restored to its state before that edit; the ids/frames the edit returned are no longer valid, so re-read with get_timeline or get_transcript if you'll edit again. Takes no arguments.

Schema: no properties.

### A-5. Multicam (deferred in this change; slots reserved)

#### 28. `manage_multicam`

> Create or ungroup a multicam group. create syncs session media into ordinary stamped timeline clips: one program video track, one audio track per mic, and angle switches through change_cam. Use member kind angle for scratch-camera audio, mic for program audio, and both for a camera whose audio should play. Pin offsetSeconds when correlation cannot align a member. ungroup strips stamps and leaves clips in place.

- `create` (object; required key: `members`)
  - `members` (array of object, **required**; required keys: `mediaRef`, `kind`) — Session source files, at least two.
    - `mediaRef` (string) — Media asset id from get_media.
    - `kind` (string, enum: `angle`, `mic`, `both`) — angle = camera scratch audio, mic = audio in the mix, both = camera plus program audio.
    - `angleLabel` (string) — Handle used by change_cam. Default: file name.
    - `offsetSeconds` (number) — Pin this member's group-clock offset instead of correlating.
  - `name` (string) — Group name. Default: Multicam N.
  - `master` (string) — angleLabel or mediaRef whose audio clock defines the group. Default: first mic/both member.
  - `startFrame` (integer) — Timeline frame to place the group. Default: timeline end.
  - `searchWindowSeconds` (number) — Max ± audio sync search window, seconds (default 240).
- `ungroup` (object; required key: `groupId`)
  - `groupId` (string) — Group to dissolve; its clips stay put, unstamped.

#### 29. `change_cam`

> Switch a multicam group's camera angle over timeline frame ranges, full-frame or in a multi-angle layout. Batched entries are one undo step. Ranges where an angle was not recording clamp or skip. Returns switched count, optional clamps/skips/overlayClipIds, and program rows over the touched span.
>
> Each entry is EITHER {range, angle} — full-frame switch — or {range, layout, angles} — PiP/split/grid: angles fill the layout's slots in order (first = the full-frame program slot; fewer angles than slots leaves cells empty), extra angles land as synced overlay clips above the program. A later full-frame entry over the same range clears the layout. Overlay clips are ordinary group clips — restyle with set_clip_properties/apply_layout, remove with remove_clips.

- `groupId` (string) — The multicam group (from manage_multicam create or get_timeline's multicamGroups). Or pass clipId.
- `clipId` (string) — Any clip of the group on the active timeline.
- `entries` (array of object, **required**; required key: `range`) — Switches to apply, in order. Later entries win on overlap.
  - `range` (array of integer) — [startFrame, endFrame) in timeline frames.
  - `angle` (string) — angleLabel to show full-frame. Omit when using layout.
  - `layout` (string) — Multi-angle layout: side_by_side, top_bottom, pip_bottom_right, pip_bottom_left, pip_top_right, pip_top_left, grid_2x2, main_sidebar, three_up.
  - `angles` (array of string) — angleLabels in slot order for layout; [0] is the program slot.

#### 30. `get_multicam`

> Read a multicam group: members (angleLabel, kind, offsetSeconds, confidence, which is master), the current program cut as run-length [angle, startFrame, endFrame) rows in timeline frames, and the track indexes the group occupies. Use it to learn angle labels before change_cam, or to review the cut as one program instead of piecing it together from get_timeline's clips. Window long timelines with startFrame/endFrame.

- `groupId` (string) — The multicam group id. Or pass clipId.
- `clipId` (string) — Any clip of the group on the active timeline.
- `startFrame` (integer) — Optional window start for program rows.
- `endFrame` (integer) — Optional window end (exclusive).

### A-6. Transcript

#### 31. `get_transcript`

> Returns the spoken transcript of the CURRENT timeline in project frames — the post-edit caption track in one call. Unlike inspect_media (which transcribes one source asset in isolation, in source seconds), this walks every audio/video clip on the timeline, maps each word through that clip's trim/speed/position, and concatenates in timeline order. Deleted ranges are gone by construction, so after cuts this always reflects what's actually audible — no stale results, no per-clip frame math. The app chooses cloud only when the signed-in account has enough credits for the uncached request; otherwise it uses local transcription and reports the resolved transcriptionSource in the response.
>
> Returns clips in timeline order, each with its words as compact [index, text, startFrame] rows (a word runs to the next word's start; the last word to its clip's end). Speakers, when identified, arrive as run-length turns: speakers = [[firstWordIndex, name], ...]. The index is a stable, global, 0-based position in timeline order; pass it straight to remove_words to cut that word (the intuitive path for text-based editing). Indices stay global even when scoped with clipId or paged with a window. Capped at 10000 words; page with startFrame/endFrame using nextStartFrame.
>
> For comprehension rather than cutting — summarizing, finding a topic, take selection on long media — pass granularity='segments': sentence rows [firstWordIndex, text, start, end] at a fraction of the tokens, whose firstWordIndex jumps you back into word mode for the cut window.
>
> Use for transcript-driven edits (filler-word / dead-air removal, locating a quote, take selection) and to verify what remains after cutting. To cut, prefer remove_words (give it the indices); drop to ripple_delete_ranges only for non-word-aligned spans.

- `startFrame` (integer) — Optional. Only return words ending after this project frame. Use with the returned nextStartFrame to page a long timeline.
- `endFrame` (integer) — Optional. Only return words starting before this project frame.
- `clipId` (string) — Scope the transcript to a single clip — returns only what that clip says, in project frames. Answers "what's in clip X?" without scanning the whole timeline.
- `granularity` (string, enum: `words`, `segments`) — words (default) for cutting with remove_words; segments for cheap sentence-level reading — rows carry firstWordIndex to drill back into words.
- `language` (string) — Optional BCP-47 speech language. Applies to local only; cloud auto-detects.

Output: see contract C-6. (Executor also tolerates a `wordTimestamps` key not present in the schema.)

#### 32. `remove_words`

> Cut speech by the word, Descript-style — the primary tool for text-based editing (filler words, flubbed sentences, dropped retakes, tightening a ramble). Pass words for precise get_transcript indices/ranges, or matches for exact filler tokens like "um" and "uh". This resolves them to frames, removes the surrounding pause so survivors don't end up double-spaced, merges adjacent removals, cuts linked A/V partners, and closes the gaps. You never deal in frame numbers — that's the whole point versus ripple_delete_ranges.
>
> Workflow: call get_transcript, read it as prose, then pass the indices of the words to drop. Omit language by default; remove_words reuses the previous get_transcript source so cloud/local word indices stay aligned. Words across multiple clips on ONE track are handled in a single undoable action, and any linked A/V partner (e.g. the video paired with this audio) is cut automatically. Edit one track at a time: if your indices span multiple unlinked tracks (e.g. two separate mics), the call is refused — cut each track in its own call, or link the tracks into one unit first. After it runs, indices have shifted — re-read get_transcript before another remove_words.
>
> When to use which: words for selective edits after reading the transcript; matches for removing every exact filler token; ripple_delete_ranges only for spans that aren't word-aligned. Verify reworded retakes and sub-frame seam fragments against the word list, not a summary.

- `words` (array of integer | [integer, integer]) — Words to remove, by get_transcript index. Each element is either a single index (e.g. 42) or an inclusive [startIndex, endIndex] span (e.g. [12, 18]). Mutually exclusive with matches. Re-read after any edit.
- `matches` (array of string) — Exact single-word tokens to remove everywhere, case-insensitive with surrounding punctuation ignored, e.g. ["um", "uh", "hmm"]. Mutually exclusive with words. Avoid broad words like "like" unless the user explicitly wants every occurrence removed.
- `cutAggressiveness` (string, enum: `tight`, `balanced`, `loose`) — How much silence to leave between the words on either side of a cut. 'tight' butts them close (snappy, can feel clipped), 'balanced' (default) keeps a natural beat, 'loose' leaves more breathing room. The removed words' own frames always go regardless.
- `language` (string) — Optional BCP-47 speech language for local transcription. Omit to reuse the previous get_transcript language.

(No top-level required array.)

#### 33. `remove_silence`

> Remove dead air — quiet, speech-free sections — from the timeline's audio, ripple-closing the gaps. Sections come from on-device speech detection (the same spans marked red on waveforms): non-speech runs whose level sits well below the recording's own speech level, so music beds and loud ambience are never cut, and speech-boundary slop keeps the cuts from feeling clipped. Cuts linked A/V partners and honors sync lock; the whole pass is one undoable action.
>
> Use this to tighten pacing (long pauses, dead space between takes) before or instead of word-level edits: remove_silence handles pauses, remove_words handles fillers and flubbed lines. No transcript needed. If it reports no dead air, speech analysis may still be running in the background — wait a moment and retry. Takes no arguments.

Schema: no properties.

#### 34. `detect_beats`

> Detect musical beats and downbeats in a media asset's audio, on-device. Returns beats and downbeats in SOURCE seconds (multiply by fps for frame values, same convention as search_media hits) plus estimated bpm. Downbeats mark bar starts — cut on downbeats for edits that land musically; beats are fine for faster montage rhythms.
>
> Use for beat-synced editing: snapping cuts to a music bed, building montages where clip boundaries hit the beat, or timing text/caption entrances to the bar. To place a cut at a beat B on a clip, the timeline frame is startFrame + (B × fps − trimStartFrame) / speed. Works on music; speech or ambience returns few or no beats. Runs locally — no subscription needed.

- `mediaRef` (string, **required**) — Audio or video asset id from get_media.
- `startSeconds` (number) — Optional. Return only beats at or after this source-media second. The whole file is analyzed once and cached; windowing trims the response, not the work.
- `endSeconds` (number) — Optional. Return only beats at or before this source-media second.

### A-7. Text & captions

Shared property blocks referenced below —
`textBoxTransformProperties`: `centerX` (number, "0-1 horizontal center."), `centerY` (number, "0-1 vertical center."), `width` (number, "0-1 width."), `height` (number, "0-1 height.").
`textStyleProperties`: `fontName` (string, "Font name."), `fontSize` (number, "Canvas points."), `isBold` (boolean, "Bold."), `isItalic` (boolean, "Italic."), `color` (string, "Text color hex."), `alignment` (string, enum left|center|right, "Text alignment."), `borderColor` (string, "Text outline hex; enables outline."), `backgroundColor` (string, "Text box fill hex; enables fill.").
`animation` enum = `TextAnimation.Preset.agentValues` (Appendix C).

#### 35. `add_texts`

> Adds text clips as timeline layers. Omit trackIndex on every entry to create one new top video track; otherwise set trackIndex on every entry. Transform is normalized text-box center/size; center-only auto-fits, all four fields override the box. Use add_captions for spoken audio captions. Unknown fields are rejected.

- `entries` (array of object, **required**; required keys: `startFrame`, `endFrame`, `content`) — Text clips to add. Each entry merges:
  - `trackIndex` (integer) — Existing non-audio track. Omit on all entries to create a new top track.
  - `startFrame` (integer) — Timeline start frame.
  - `endFrame` (integer) — Occupy timeline frames [startFrame, endFrame) — copy a clip's frames pair to title exactly that span.
  - `content` (string) — Text. Supports \n.
  - `transform` (object: textBoxTransformProperties) — Text box. Omit for centered auto-fit; center only auto-fits size; all four override.
  - …textStyleProperties (all 8, flattened at entry level)
  - `animation` (string, enum: agentValues) — Animation preset; off clears.
  - `highlightColor` (string) — Active-word hex.

#### 36. `update_text`

> Updates text clips or a captionGroupId. Use for content, typography, color, outline color, background color, animation, or text-box transform. Content/typography changes auto-fit the box unless transform is passed. Unknown fields are rejected.

- `clipIds` (array of string) — Text clip IDs. Optional if captionGroupId is given.
- `captionGroupId` (string) — Caption group id from get_timeline.
- `content` (string) — Replacement text. Supports \n.
- `transform` (object: textBoxTransformProperties) — Partial text-box transform; omitted fields keep current values.
- …textStyleProperties (all 8, flattened at top level)
- `animation` (string, enum: agentValues) — Animation preset; off clears.
- `highlightColor` (string) — Active-word hex.

(No required array.)

#### 37. `add_captions`

> Transcribes the timeline's spoken audio and creates styled caption text clips on their own track — no targeting needed; it finds the spoken content itself. The app uses cloud only when the signed-in account has enough credits for the uncached request; otherwise it uses local transcription. Cloud auto-detects language. Per-word animations are timed from the transcript. Returns the caption group summary (captionGroupId, clipCount, frameRange, shared style, textPreview) — restyle it later with update_text and that captionGroupId.

- `language` (string) — BCP-47 speech language. Applies to local only; cloud auto-detects.
- `transform` (object) — Caption box position; size is auto-fit per caption.
  - `centerX` (number) — 0-1 horizontal center.
  - `centerY` (number) — 0-1 vertical center.
- `textCase` (string, enum: `auto`, `upper`, `lower`) — Letter case.
- `censorProfanity` (boolean) — Mask profanity.
- `maxWords` (integer) — Max words per caption.
- …textStyleProperties (all 8, flattened at top level)
- `animation` (string, enum: agentValues) — Caption animation preset.
- `highlightColor` (string) — Active-word hex.

### A-8. Color & effects

#### 38. `apply_color`

> Author/refine a color grade on video/image clips with named controls — the colorist path, distinct from apply_effect (looks/FX). Returns the clips with their resulting grade as a `color` object — the same object get_timeline shows; pass one back via the `color` parameter to copy a grade between clips (replaces the whole grade). MERGES with the clip's current grade: only the params you pass change, the rest are preserved, so you can nudge one knob at a time (pass reset:true to start from neutral). Applies as live, editable color.* effects; non-color effects untouched. Iterate: apply_color → inspect_color(clipId, reference) → read the gap → adjust → repeat. Undoable. All knobs optional. Color WHEELS use HUE (0–360°, standard) + AMOUNT per tonal zone — to push shadows teal, set shadowsHue 180 and shadowsAmount ~0.15. CURVES (master + per-channel R/G/B) give precise tone shaping — per-channel curves are tone-selective (e.g. pull the blue curve down in the highlights to tame a bright sky). HUE CURVES do secondary/qualified correction — target a source hue and shift its hue/saturation/lightness (e.g. desaturate greens, warm the skin) without a mask; pair with inspect_color's hueHistogram to find which hues are present. LUT applies a .cube film-look pack on top of the grade.

- `clipIds` (array of string, **required**) — Clip ids from get_timeline.
- `reset` (boolean) — Start from neutral instead of merging onto the clip's current grade. Default false.
- `color` (object) — A complete grade object as read from a clip's `color` key (get_timeline or an apply_color echo). Replaces the target clips' grade — the grade-copy path. Mutually exclusive with reset and individual knobs.
- `exposure` (number) — -3…3 EV. Overall brightness in linear light.
- `contrast` (number) — 0.5…1.5 (1 = neutral).
- `saturation` (number) — 0…2 (1 = neutral; <1 mutes).
- `vibrance` (number) — -1…1 (protects skin tones).
- `temperature` (number) — 2000…11000 K. HIGHER = WARMER, lower = cooler/bluer (6500 = neutral).
- `tint` (number) — -100…100. Positive = green, negative = magenta.
- `highlights` (number) — -1…1. Recover (<0) or lift (>0) highlights.
- `shadows` (number) — -1…1. Lift (>0) or deepen (<0) shadows.
- `blacks` (number) — -1…1. Black point. Negative deepens, positive lifts (faded look).
- `whites` (number) — -1…1. White point.
- `shadowsHue` (number) — Shadow color-push hue 0–360° (0 red, 30 orange, 60 yellow, 120 green, 180 cyan, 240 blue, 300 magenta). Use with shadowsAmount.
- `shadowsAmount` (number) — 0…1 strength of the shadow color push (0 = neutral).
- `shadowsLum` (number) — -0.5…0.5 shadow lift (brightness).
- `midsHue` (number) — Midtone color-push hue 0–360° (see shadowsHue). Use with midsAmount.
- `midsAmount` (number) — 0…1 strength of the midtone color push.
- `midsGamma` (number) — 0.5…2 midtone brightness (gamma; 1 = neutral).
- `highsHue` (number) — Highlight color-push hue 0–360° (see shadowsHue). Use with highsAmount.
- `highsAmount` (number) — 0…1 strength of the highlight color push.
- `highsGain` (number) — 0.5…1.5 highlight brightness (gain; 1 = neutral).
- `masterCurve` (array of [number, number]) — Luma tone curve as [x,y] control points in 0–1 (input→output), preserves chroma. E.g. [[0,0.06],[1,0.95]] = lifted/faded film toe.
- `redCurve` (array of [number, number]) — Red-channel tone curve, [x,y] points 0–1.
- `greenCurve` (array of [number, number]) — Green-channel tone curve, [x,y] points 0–1.
- `blueCurve` (array of [number, number]) — Blue-channel tone curve, [x,y] points 0–1. Tone-selective: e.g. [[0,0],[0.7,0.7],[1,0.85]] pulls blue only in the highlights (tames a sky) and leaves shadows.
- `hueCurves` (object) — Secondary/qualified correction (Resolve-style Hue-vs-Hue/Sat/Lum). Targets replace any existing hue curve. Selectivity is ~±22° around each target hue.
  - `targets` (array of object; required key: `targetHue`) — One or more source-hue regions to adjust (e.g. skin at 30, sky at 210).
    - `targetHue` (number) — Source hue to act on, 0–360° (0 red, 30 orange/skin, 60 yellow, 120 green, 180 cyan, 210 sky-blue, 240 blue, 300 magenta).
    - `hueShift` (number) — Rotate that hue by -30…30°.
    - `satScale` (number) — Saturation multiplier for that hue, 0–2 (1 = neutral; 1.3 pops it, 0.6 mutes it, 0 fully desaturates).
    - `lumShift` (number) — Lightness shift for that hue, -0.5…0.5.
- `lut` (object) — Apply a .cube 3D LUT (e.g. a film-look pack) on top of the primary grade; replaces any prior LUT. The agent does not author LUT data — pass a real file path.
  - `path` (string) — Absolute path to a .cube file (~ is expanded). Copied into project storage so it survives saves.
  - `strength` (number) — Dry/wet mix 0-1 (default 1).

#### 39. `apply_effect`

> Apply non-color effects (blur, sharpen, stylize, detail, key) to video/image clips as a live, editable effect stack — the looks/FX path, distinct from apply_color (grading). MERGES: each effect you pass is added or updated by type; effects you don't mention are left in place. Pass enabled:false to bypass one without removing it, or list its type in `remove` to delete it. Out-of-range params are clamped; params you omit keep their current (or default) value. Effects render in a fixed canonical order regardless of the order you pass them. Undoable. Returns the clips with their resulting effects as [{type, params}] — the same shape this tool accepts, so copying effects between clips is passing a clip's effects array back in.
>
> Available effects — type: param (range, default):
> *(generated from `EffectRegistry` — expansion in Appendix C)*

- `clipIds` (array of string, **required**) — Clip ids from get_timeline.
- `effects` (array of object; required key: `type`) — Effects to add or update on the clips.
  - `type` (string) — Effect type id, e.g. stylize.glow (see list above).
  - `params` (object) — Param values keyed by name. Out-of-range values are clamped; omitted params keep their current/default value.
  - `enabled` (boolean) — Default true. false bypasses the effect without removing it.
- `remove` (array of string) — Effect type ids to remove from the clips.

#### 40. `inspect_color`

> Measure color scopes of a timeline clip's current graded look (clipId) OR a raw media asset (mediaRef) — black/white points, % clipping, mean & per-channel levels, shadow/mid/highlight color tilt, saturation, warm-cool / green-magenta balance, and a saturation-weighted hueHistogram (12 bins of 30° from 0°/red — shows which hues are present, e.g. an orange cluster = skin, a cyan/blue cluster = sky) — and return the rendered frame too. Use this to grade by the numbers instead of eyeballing, to find hues to target with apply_color's hueCurves, or to measure footage/references before grading. clipId applies the clip's effects (graded look); mediaRef measures the raw asset. Pass a reference image/video id to also measure it and get the subject−reference GAP plus hints that map onto apply_color knobs. The loop: apply_color → inspect_color(clipId, reference) → read the gap → adjust → repeat until the gap is small.

- `clipId` (string) — Timeline clip to measure — returns its current GRADED look (effects applied). Provide this or mediaRef.
- `mediaRef` (string) — Media asset id from get_media to measure RAW (no grade). Provide this or clipId.
- `atFrame` (integer) — Optional project frame to sample a clip. Defaults to the clip's midpoint. Ignored for mediaRef.
- `reference` (string) — Optional image/video asset id from get_media to compare against; returns its scopes + the subject−reference gap.

#### 41. `denoise_audio`

> Remove background noise from audio clips using an on-device speech-enhancement model (DeepFilterNet3). strength is a dry/wet mix 0-1: 0 leaves the audio untouched, 1 is fully denoised. Full strength can sound thin or over-gated on real-world recordings, so the default is 0.6. The bake runs in the background — the timeline updates automatically when it finishes; no need to poll. Pass enabled:false to turn denoise off. Undoable.

- `clipIds` (array of string, **required**) — Audio clip ids from get_timeline.
- `strength` (number) — Dry/wet mix, 0–1 (default 0.6). Lower it if voices sound thin or over-compressed.
- `enabled` (boolean) — Default true. false removes the denoise effect from the clips.

### A-9. Generation

#### 42. `list_models`

> Lists AI models with their capabilities (durations, aspect ratios, resolutions, first/last frame support, reference support, voices/category for audio, upscaler speed). Always call before generate_video, generate_image, generate_audio, or upscale_media so the model you pick actually supports the constraints you need. Returns { models, loaded } — if loaded=false the catalog hasn't synced yet (e.g. user not signed in); the models array may be empty even when models exist, so do not conclude no models are available. Retry after the user signs in.

- `type` (string, enum: `video`, `image`, `audio`, `upscale`) — Filter by type. Omit to list all models.

#### 43. `generate_video`

> Starts an async AI video generation. Returns a placeholder asset ID immediately; generation runs in the background and the asset becomes usable in add_clips once ready. Costs real money and is not undoable.

- `prompt` (string, **required**) — Text description of the video to generate
- `name` (string) — Display name for the asset in the media library. Defaults to first 30 chars of prompt.
- `model` (string) — Model ID (e.g. 'veo3.1-fast'). Use list_models to see options. Defaults to first available model.
- `duration` (integer) — Duration in seconds. Valid values depend on model.
- `aspectRatio` (string) — Aspect ratio (e.g. '16:9', '9:16', '1:1')
- `resolution` (string) — Resolution (e.g. '720p', '1080p', '4k')
- `startFrameMediaRef` (string) — Media asset ID to use as the first frame (image-to-video)
- `endFrameMediaRef` (string) — Media asset ID to use as the last frame (supported by some models)
- `sourceVideoMediaRef` (string) — Media asset ID of a source video (required by video-to-video edit models; ignores duration/aspectRatio/resolution)
- `sourceClipId` (string) — Optional. Clip id (from get_timeline) referencing sourceVideoMediaRef. When set and the clip is trimmed, only the clip's visible range is sent to the model, not the full source — matches the UI's 'Use trimmed portion only'.
- `referenceImageMediaRefs` (array of string) — Media asset IDs of image references. Covers both reference-to-video generation (Seedance, Kling V3/O3 elements, Grok — refer as @Image1/@Element1 in prompt) and the single-image ref used by video-to-video edit models (Kling V3 Motion Control). See list_models maxReferenceImages for per-model cap.
- `referenceVideoMediaRefs` (array of string) — Media asset IDs of video references (Seedance only). Refer to them as @Video1, @Video2. See maxReferenceVideos and maxCombinedVideoRefSeconds.
- `referenceAudioMediaRefs` (array of string) — Media asset IDs of audio references (Seedance only). Refer to them as @Audio1, @Audio2. See maxReferenceAudios and maxCombinedAudioRefSeconds.
- `folder` (string) — Optional destination folder path, e.g. 'Hero shots/Takes'. Created if missing. Omit for the project root.

#### 44. `generate_image`

> Starts an async AI image generation. Returns a placeholder asset ID immediately; generation runs in the background. Costs real money and is not undoable.

- `prompt` (string, **required**) — Text description of the image to generate
- `name` (string) — Display name for the asset in the media library. Defaults to first 30 chars of prompt.
- `model` (string) — Model ID (e.g. 'nano-banana-pro'). Use list_models to see options. Defaults to first available model.
- `aspectRatio` (string) — Aspect ratio (e.g. '16:9', '9:16')
- `resolution` (string) — Resolution (e.g. '2K', '4K')
- `quality` (string) — Image quality (e.g. 'low', 'medium', 'high'). Only supported by some models — see list_models.
- `referenceMediaRefs` (array of string) — Media asset IDs to use as reference images
- `folder` (string) — Optional destination folder path, e.g. 'Hero shots/Takes'. Created if missing. Omit for the project root.

#### 45. `generate_audio`

> Starts an async AI audio generation: text-to-speech, text-to-music, or video-to-music (scoring a video). Returns a placeholder asset ID immediately; the asset appears in get_media and becomes usable in add_clips once ready. TTS models (elevenlabs-tts-v3, gemini-3.1-flash-tts) convert the prompt into speech and accept a 'voice'. Music models (lyria3-pro, minimax-music-v2.6, elevenlabs-music, sonilo-v1.1-video-to-music) generate tracks from a prompt; include lyrics/tempo/vocal style in the prompt for Lyria 3 Pro, pass 'lyrics' for MiniMax vocals, or set 'instrumental' true when the selected model supports it. Video-to-audio models (inputs include 'video' — see list_models, e.g. sonilo-v1.1-video-to-music, mirelo-sfx-v1.5-video-to-audio) generate audio that matches a VIDEO: provide a timeline span via videoSourceStartFrame+videoSourceEndFrame (e.g. to score the timeline), or a video asset via videoSourceMediaRef; the prompt is then an optional style guide. PLACEMENT: when you pass a timeline span, the result is placed on the timeline automatically at that span (no add_clips needed); for a media-asset source or a plain text-to-speech/music result, the asset lands in the library and you place it with add_clips. Use list_models with type='audio' to see each model's 'inputs', category, and voices. Costs real money and is not undoable.

- `prompt` (string) — Required for TTS (the text to speak) and text-to-music (style/mood/genre; MiniMax needs ≥10 chars). For Lyria 3 Pro, include lyrics, tempo, language, and vocal style directly in the prompt. Optional style guide for video-to-music models.
- `name` (string) — Display name for the asset in the media library. Defaults to first 30 chars of prompt.
- `model` (string) — Model ID. Use list_models with type='audio' to see options and their 'inputs'. Defaults to the first model.
- `voice` (string) — TTS only. Voice preset name. list_models shows voicesSample (first 3) + voiceCount; any voice supported by the model is accepted. Defaults to the model's defaultVoice. Ignored by music models.
- `lyrics` (string) — MiniMax Music only. Lyrics with optional [Verse]/[Chorus] section tags. If omitted and instrumental=false, MiniMax auto-writes lyrics from the prompt.
- `styleInstructions` (string) — Gemini TTS only. Optional delivery instructions (e.g. 'warm and slow', 'British accent').
- `instrumental` (boolean) — Music models only. true = no vocals when the selected model supports it. Defaults to false.
- `duration` (integer) — Length in seconds. ElevenLabs Music: 3–600. Sonilo text-to-music: up to 600. For a video source, defaults to the span/clip length. Ignored by TTS, MiniMax, and Lyria 3 Pro.
- `videoSourceStartFrame` (integer) — Video-to-audio models only. Start frame (timeline) of a span to render and score — pair with videoSourceEndFrame. Use get_timeline for frame numbers; for the whole timeline use 0 to the timeline's end frame.
- `videoSourceEndFrame` (integer) — Video-to-audio models only. End frame (exclusive) of the span to score. Must be > videoSourceStartFrame.
- `videoSourceMediaRef` (string) — Video-to-audio models only. Score this existing video asset instead of a timeline span. Mutually exclusive with the videoSource frames.
- `folder` (string) — Optional destination folder path, e.g. 'Hero shots/Takes'. Created if missing. Omit for the project root.

(No required array — `prompt` requirement is conditional per model category.)

#### 46. `upscale_media`

> Upscales an existing video or image asset to higher resolution using an AI upscaler. Returns a placeholder asset ID immediately; the upscaled asset appears in get_media once ready. Use list_models with type='upscale' to pick a model that supports the asset's type. Costs real money and is not undoable.

- `mediaRef` (string, **required**) — ID of the video or image asset to upscale
- `model` (string) — Upscaler model ID (e.g. 'bytedance-upscaler', 'seedvr-image-upscaler'). Defaults to the first model that supports the asset's type.
- `sourceClipId` (string) — Optional. Video clip id (from get_timeline) referencing mediaRef. When set and the clip is trimmed, only the clip's visible range is upscaled, not the full source.

### A-10. Meta

#### 47. `send_feedback`

> Report an agent limitation or bug to the Palmier team so they can improve the product. Use when you can't do what the user asked because a capability or tool is missing or behaves wrong, the result is clearly off, or the user is plainly hitting a rough edge. This sends directly — there is no user confirmation step — so write the report in English and PARAPHRASE in your own words: translate non-English user text to English, and never include verbatim user messages, prompts, file paths, media, transcript text, or any project content. App/OS version and your recent tool names are attached automatically. Use sparingly: at most once per distinct issue.

- `category` (string, **required**, enum: `missing_capability`, `wrong_result`, `confusing_ux`, `failure`, `suggestion`) — What kind of problem this is.
- `summary` (string, **required**) — One-line paraphrased summary of the issue. Becomes the report's subject.
- `details` (string) — Optional. Paraphrased explanation of what the user was trying to do and what went wrong or was missing. No verbatim content.
- `severity` (string, enum: `low`, `medium`, `high`) — Optional. How much this blocked the user.

#### 48. `read_skill` (in-app agent only)

> Load the full instructions for one of the skills listed under # Skills in your system prompt. Call this before starting a task that matches a skill's description, then follow the returned procedure. Pass the id exactly as listed.

- `id` (string, **required**) — The skill id, exactly as listed under # Skills.

## Appendix B — SYSTEM_INSTRUCTION verbatim (upstream/main@141c69b `Sources/PalmierPro/Agent/Tools/AgentInstructions.swift`)

Composition: in-app agent prompt = `serverInstructions` + `skillsSection(index)`; MCP server instructions = `serverInstructions` + `projectNavigation`. Rendered text below (Swift `\` line continuations joined; wrapped here only for readability — the runtime string is continuous per bullet).

### B-1. `AgentInstructions.serverInstructions`

```text
You are a creative AI assistant connected to palmier-pro, an AI-native video editor. Help the user build and edit their project by calling the tools this server exposes.

# Core model
- Timing: TIMELINE positions are project frames (startFrame, frames pairs, gaps, ranges); SOURCE positions are seconds (source spans, search hits, asset transcripts and durations). Tools convert between them — never multiply by fps yourself.
- Tracks are ordered and typed (video or audio); index 0 renders on top. Video clips, images, and text overlays all live on video tracks.
- A clip occupies frames [start, end). Placement takes startFrame + endFrame or source: [startSeconds, endSeconds]; lengths elsewhere are durationFrames. A video clip's linked audio is folded into it as audio: {id, track, …} — use that nested id to edit the audio side.
- A project can hold several timelines; exactly one is active and every read/edit tool targets it (get_media lists them; switch with set_active_timeline, then re-read). A nested timeline appears as a clip with mediaType 'sequence'.
- IDs are short prefixes — pass them back exactly as given, never padded or completed. Folders have no ids: they are paths ('B-roll/Sunset'), created on demand.

# Session
- Call get_timeline once per session (or after an out-of-band change). Don't re-read between your own edits — every mutation returns a delta in get_timeline vocabulary: clips (resulting state, with track), shifted rules ({track, fromFrame, by, count}), removedClipIds, createdTracks, and notes. Patch your model from that; re-read only after a failure that suggests it's stale. Caption clips arrive as captionGroup summaries — restyle whole groups from that alone; captionDetail=true (windowed) only to touch individual caption clips.
- Call get_media before referencing any asset; filter with ids (poll a generation), folder, or pending=true.
- Call list_models before any generate_* or upscale call. If get_timeline says canGenerate=false, generation will fail — ask the user to sign in to Palmier and subscribe first.
- Never describe an asset from its filename — inspect_media first. On long media work coarse to fine: overview=true storyboard, then transcript segments, then zoom with startSeconds/endSeconds.
- To find a moment ("the sunset shot", "where she mentions the budget"): search_media first, then pass hits straight to add_clips as source: [startSeconds, endSeconds].

# Editing
- Edits are undoable and effectively free — don't ask permission for individual edits; just say what changed.
- Composition (split screen, PIP, grid, position/size on canvas) is apply_layout's job: pick a layout, fill every slot, nudge framing with anchorX/anchorY. Never build layouts from set_clip_properties transform or set_keyframes. When an inset hides behind another track, fix stacking with manage_tracks reorder.
- Cutting, in order of preference: remove_silence for pauses and dead air (no transcript needed — run it first when tightening pacing); remove_words for fillers and flubbed lines — read the word-level transcript as prose once, then pass indices; it maps words to frames and closes the gaps. After a cut, indices shift — re-read get_transcript before the next remove_words. ripple_delete_ranges only for spans that aren't word-aligned; split_clips only inserts boundaries (nothing shifts).
- Beat-synced edits: detect_beats on the music asset first, then cut on downbeats (bar starts) — beats only for fast montage rhythms. Times are source seconds.
- Text: add_texts for authored overlays; add_captions transcribes the timeline's spoken audio (no targeting) — restyle with update_text and the returned captionGroupId. Color: apply_color (knobs merge; pass a clip's `color` object to copy a whole grade); other FX: apply_effect; iterate grades against inspect_color.
- Transcription language: omit unless the user names the spoken language. Cloud auto-detects; local is language-specific — pass BCP-47 (language='es') for non-English local runs, and if local output looks wrong, ask for the language and retry.
- A transcript summary is lossy: it hides reworded retakes and zero-width seam fragments (a word whose start equals the next word's start) — verify suspected fragments against the words, not the summary.

# Export
- export_project modes: video (default — H.264/H.265/ProRes, 720p–4K or Match Timeline), xml (Premiere), fcpxml (Resolve / Final Cut), palmier (self-contained package). Omit outputPath unless the user named a destination (default ~/Downloads). Video renders in the background — say so; a notification reports completion. The other modes finish inline.

# Generation
- Costs real money and is not undoable: propose prompt, model, duration, and aspect ratio, then wait for confirmation.
- Flow: images first — iterate stills until the user approves the look, then use the approved image as the video's startFrameMediaRef. Straight text-to-video only when asked or when no frame anchors the shot.
- Models (resolve via list_models): images — Nano Banana Pro and GPT Image for most stills (text, graphics, consistency), Grok for fast cheap iterations, Krea 2 or Recraft for cinematic mood. Video — Seedance 2.0 Fast at 720p while iterating, regular Seedance 2.0 for the approved take, Kling v3 if Seedance errors, Grok Imagine only for very simple scenes, Veo rarely.
- Generation and url/path imports return a placeholder id and run in the background. Don't busy-poll — fire and move on; when you must check, get_media ids:[placeholder] is the cheap read. On generationStatus 'failed', tell the user and ask before re-firing.
- Consistency: reuse referenceMediaRefs on images; startFrameMediaRef / endFrameMediaRef and the per-model reference*MediaRefs on video. Build base shots before derived ones; parallelize independent generations; organize related generations with a `folder` path on the call.
- Video models cannot render readable text — bake text into a still via generate_image, or use add_texts. Never generate UI screenshots, logos, title cards, text overlays, or motion graphics; those belong in the editor.
- import_media bridges external assets (url, path, or bytes) and makes solid-color mattes (source.matte with hex).
- Audio models (list_models type='audio'): TTS — the prompt is the exact words to speak; pass a supported voice, styleInstructions where offered. Music — the prompt describes style/mood/genre; lyrics with [Verse]/[Chorus] tags where supported (for Lyria 3 Pro, fold lyrics/tempo/language/vocal style into the prompt); instrumental only where supported.

# Prompt craft
- Images, 15–30 words: subject + setting + shot type + lighting/mood. Concrete nouns beat adjectives.
- Videos, 8–20 words: camera movement + subject action. With a startFrameMediaRef, don't re-describe the frame — spend the words on motion and sound. State dialogue, VO, SFX, and music explicitly; silent video is usually a bug.

# Feedback
- When a capability is missing or broken, a result is clearly wrong, or the user is plainly hitting a limitation, call send_feedback once with a paraphrased summary — never verbatim user content. Send workflow improvements as `suggestion`. One per distinct issue; mention it to the user briefly.

# Communication
- One or two sentences; lead with the outcome. The user watches the timeline change — never narrate steps, never recap what a tool returned. No preamble, no play-by-play. Match the app's calm, terse, HIG-style voice: never chatty, never marketing. When the user is vague about aesthetic direction, ask one focused question instead of guessing.
```

### B-2. `AgentInstructions.projectNavigation` (MCP server only; appended after serverInstructions)

```text

# Projects
These tools choose which project you edit — every other tool acts on the active project, and you may start with none open.
- get_projects: list known projects (id, name, path, whether open, which is active). Call this first when unsure what's available.
- open_project: make an existing project active by name, id (from get_projects), or path. Editing tools then target it; the return is a snapshot (fps, resolution, timelines, mediaCount) that orients you before get_timeline.
- new_project: create and open a fresh project. Give it a name; it's created in the Palmier Pro folder. Fails if that name already exists there.
- close_project: save and close a project (the active one when no argument is given). Close projects you opened for a lookup once you're done with them.
Only one project is active at a time — opening or creating one switches the active project, and the user sees the window change.
```

### B-3. `AgentInstructions.skillsSection(index)` (in-app agent only; empty string when the index is empty)

```text

# Skills
Playbooks for specific tasks. Before a task that matches one, call read_skill(id) to load its full procedure, then follow it.
<index>
```

Rust addition (this change): a clearly-delimited extension section appended after the upstream text, documenting the 8 Rust-native tools (task 5.1; exact copy written there, following the same terse register).

## Appendix C — Resolved Swift enum expansions (verified at 141c69b)

**`BlendMode.allCases.map(\.rawValue)`** (`Sources/PalmierPro/Models/BlendMode.swift`) — set_clip_properties.blendMode enum, 16 values:
`normal`, `darken`, `multiply`, `colorBurn`, `lighten`, `screen`, `colorDodge`, `overlay`, `softLight`, `hardLight`, `difference`, `exclusion`, `hue`, `saturation`, `color`, `luminosity`

**`VideoLayout.allCases.map(\.rawValue)`** (`Sources/PalmierPro/Models/VideoLayout.swift`) — apply_layout.layout enum, 10 values:
`full`, `side_by_side`, `top_bottom`, `pip_bottom_right`, `pip_bottom_left`, `pip_top_right`, `pip_top_left`, `grid_2x2`, `main_sidebar`, `three_up`

**`LayoutFit`** — apply_layout.fit enum: `fill`, `fit`

**`TextAnimation.Preset.agentValues`** (`Sources/PalmierPro/Models/TextAnimation.swift:51`) — animation enum, 11 values (`"off"` + allCases minus `.none`):
`off`, `fadeIn`, `popIn`, `slideUp`, `typewriter`, `wordReveal`, `wordSlide`, `wordPop`, `wordCycle`, `highlightPop`, `highlightBlock`

**Keyframe interpolation** — `linear`, `hold`, `smooth` (default smooth).

**`ToolDefinitions.effectCatalog()`** — the non-color lines interpolated into apply_effect's description, generated from `EffectRegistry.all` filtering out `color.*` (order: detail → blur → stylize → key, per the registry's `all` concatenation):

```text
• detail.clarity — Clarity & Haze: clarity (-1…1, default 0), dehaze (-1…1, default 0)
• blur.gaussian — Gaussian Blur: radius (0…100px, default 8)
• blur.sharpen — Sharpen: amount (0…2, default 0.4)
• blur.noiseReduction — Noise Reduction: amount (0…1, default 0)
• blur.motion — Motion Blur: radius (0…100px, default 0), angle (-180…180°, default 0)
• stylize.grain — Film Grain: amount (0…1, default 0), size (0.5…4, default 1.5)
• stylize.vignette — Vignette: amount (-1…1, default 0), midpoint (0…1, default 0.5), roundness (-1…1, default 0), feather (0…1, default 0.5)
• stylize.glow — Glow: intensity (0…1, default 0), radius (0…100px, default 20), threshold (0…1, default 0.6), warmth (0…1, default 0)
• key.chroma — Chroma Key: keyHue (0…1, default 0.333), tolerance (0…1, default 0), softness (0…1, default 0.5), spill (0…1, default 0.5)
```

(`audio.denoise` is not in the registry-driven catalog — denoise is its own tool, `denoise_audio`.)

## Appendix D — Rust-native extensions kept (from `crates/agent_contract/src/tools.rs`)

Kept on top of the v2 surface; upstream@141c69b has no equivalent. Instructions get a delimited extension section (task 5.1).

| Tool | Purpose | Source |
|---|---|---|
| `duplicate_project` | Duplicate the current project package | Rust-native |
| `add_shapes` | Vector shape overlays (rect/oval/circle/arrow/line + anims) | port of PR #46 (upstream later dropped the tool from its surface) |
| `apply_animation` | Apply an animation preset to an existing clip | port of PR #46 (same note) |
| `create_compound_clip` | Group clips into a nested sequence (timeline_core::compound::nest_clips) | Rust-native (#155/#255 lineage; upstream nests only via add_clips mediaRef=timelineId and has no grouping tool) |
| `dissolve_compound_clip` | Decompose a nest back onto the parent (decompose_nest) | Rust-native (upstream has no inverse either) |
| `save_clip_preset` | Save a clip's settings as a named preset | Rust-native #157 |
| `apply_clip_preset` | Apply a named preset to clips | Rust-native #157 |
| `list_clip_presets` | List saved presets | Rust-native #157 |

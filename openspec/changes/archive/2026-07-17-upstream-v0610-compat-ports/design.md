## Context

2026-07-17 上游 re-audit（47 單元，證據錨點見 workflow journal 與 `97-upstream-pr-audit.md` 的 2026-07-17 節）產出 15 個 PORT 判定；本 change 取其中 bug fix / 相容層，依 audit agents 給出的 file:line 證據施工。兩條施工線互不重疊：agent_contract 線（含 audio_core、media_library）與 core_model 線，可並行。

## Goals / Non-Goals

**Goals:** proposal 九項全數落地，每項有測試釘住；`cargo test --workspace` 全綠。
**Non-Goals:** 見 proposal Non-Goals（feature 決策清單）。

## Decisions

### #342 auto 模式永遠建立新 shared tracks

`cmd_add_clips` 的 all-omitted 分支改為 Swift 語意：不重用既有軌，視覺項建一條新視訊軌、音訊項（含 linked partners）建一條新音訊軌置底（audio zone 底部 = `tracks.len()` 插入語意已存在）。只動 all-omitted 分支；顯式 trackIndex 路徑不變。transplant 上游測試 `addClipsOmittingAudioTrackIndexAppendsBelowLinkedDialogue` 的意圖：先加 linked dialogue、再全略 index 加 music，斷言 dialogue 音訊完好、music 在其下方新軌。

### #307 manage_tracks 以 trackId 定址

- `Track` 已有穩定 `id`；`cmd_manage_tracks` 的 move/remove/set 選擇器接受 `{trackId}` 或 `{index}`（互斥），純 index 整數形式維持向後相容。
- reorder 目的地超出 zone 由 clamp 改為硬錯誤（對齊 Swift `d87faaea`）。
- 回傳 envelope extras 增加回執；移除舊的「Track indices changed」note。（施工註記：上游 `d87faaea` 的實際鍵名是 `reordered` + `removedTracks`，依「逐字對齊上游」原則採上游鍵名。）
- `get_timeline` 的 track 物件曝露 `trackId`；`id_universe` 納入 track ids；`SCALAR_ID_KEYS` 加 `"trackId"`（短 id 展開/縮寫自動生效）。
- schema/描述/system-instruction track bullet 逐字對齊 `d87faaea`。
- 數值邊界：Swift 接受整數浮點（2.0）作 index；Rust 維持 `as_i64` 嚴格性——文件化差異（audit 已標注，行為擇嚴格側，拒絕 2.5 與 "2"）。

### #274-followups detect_beats 契約

- 前置 `has_audio == Some(false)` 拒絕（沿用 tool_exec 既有 guard 模式），錯誤訊息對齊 Swift。
- windowed 呼叫以窗內 beats 重算 bpm：`audio_core::beat_detector` 曝露純函式 `estimate_bpm(beats) = 60 / median(inter-beat interval)`。（施工註記：Swift 實碼是 `count > 2` 才計算——≤2 beats 皆回 None，採 Swift 實碼並以測試釘住。）
- 空分析回 `note: "No beats found…"`；窗內無 beats 回獨立 note；bpm 為 None／downbeats 空時省略欄位（對齊 Swift 省略語意）。
- `beat_cache` key 增加檔案 (size, mtime) 標記：同 mediaRef 但檔案變更即重算。stat 走 executor 既有 `std::fs` 用法（同 import_media 路徑），MCP/headless 無檔案時視為無標記（不阻擋）。

### #333 / #338 匯入契約字串與查表

- `tools.rs` import_media 描述與 path 屬性字串逐字同步 `0ae452a0`（就地引用、同步 ready、檔案需留在原位）。
- CAF：`ClipType::from_extension` + `content_type_for_extension`（caf → `audio/x-caf`）+ `SupportedExtensions::AUDIO` + import_media 描述的格式清單 + mime 拒絕訊息。ffmpeg 解碼力假設標準 build（audit 註記），不加 fixture。

### core_model serde 相容欄位（#294/#336/#330 切片）

- `GenerationInput.target_language`：`rename = "targetLanguage"` + `skip_serializing_if = Option::is_none`（避免對 Swift 寫出 null）；round-trip 測試。
- `TextStyle`：新增 `is_underlined` / `is_struck_through` / `is_overlined`（bool，default false）、`tracking` / `line_spacing`（Option<f64>）、`font_case`（Option<String>，保留 Swift rawValue 原字串——不建 enum，避免未知 case 掉資料）、border 寬（依 Swift 實際結構：讀 `git show upstream/main:Sources/PalmierPro/Models/TextStyle.swift` 逐 key 對齊）、`Background` 擴充至 Swift 9 欄位結構。
- **TextStyleWire 橋接原則**（同 #65）：讀取接受新舊兩形；寫出 Swift v0.6.10 的鍵集合；未知欄位不足以表達時寧可保留原始值型別（Option + 原字串）也不丟資料。每個新欄位進 dual-write round-trip 測試（Swift 形→Rust→存→Swift 形逐鍵比對）。
- 施工前先 `git show upstream/main:…/TextStyle.swift`（與 Background 定義處）取得權威鍵名，禁止憑 audit 摘要拼鍵名。

## Implementation Contract

- add_clips 全略 index：dialogue+music 兩段流程後，原軌剪輯無變動、新軌各自成立（e2e 過 executor.execute）。
- manage_tracks：`{trackId}` 定址在 reorder 後仍指向同一軌；越 zone reorder 回硬錯誤；回執欄位出現在 envelope；get_timeline track 有 `trackId` 短 id。
- detect_beats：無音訊影片 → 明確錯誤；帶窗呼叫 bpm ≠ 全軌 bpm（合成不同 tempo 段測試）；空分析/空窗 note 文案對齊；換檔（同 id 改 size/mtime）後 cache miss。
- import_media 描述不再含 "downloading"/"copied into the project"；含 in-place 語意與 caf。
- media.json：Swift 形 targetLanguage round-trip 不丟。
- project.json：post-#330/#336 TextStyle 全鍵 round-trip 不丟（逐鍵斷言）。
- `cargo test --workspace` 全綠；4 個 tool-count 斷言檔不需變動（無工具增刪）。

## Risks / Trade-offs

- [#307 回執/錯誤語意影響既有 e2e 斷言] → 施工時同步更新受影響測試的預期值，逐一核對是行為對齊而非測試遷就。
- [#330 Background 結構重塑牽動既有讀取者（renderer caption background）] → renderer 只讀舊欄位子集；wire 提供舊欄位的相容 getter 或在轉換時填舊欄位，渲染行為本 change 不變。
- [beat_cache stat 在 MCP 無檔案環境] → 無法 stat 時退回無標記（等同現行為），不報錯。

## Migration Plan

Additive serde 與工具契約對齊，無資料遷移。回滾即 revert。

## Open Questions

（無——feature 級後續全部列於 proposal Non-Goals 與 audit 決策清單。）

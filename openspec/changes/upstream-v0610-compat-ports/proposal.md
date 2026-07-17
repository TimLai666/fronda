## Summary

移植 2026-07-17 上游 re-audit（`771b63e..cfa9e05e`，v0.6.1→v0.6.10，47 單元、69 個審計/覆核 agents）判定為「bug fix 或 on-disk 相容缺口」的 9 個項目。feature 級與 UI 級判定不在本 change（列於 `specs/rust-rewrite/97-upstream-pr-audit.md` 的決策清單）。

## Motivation

依 repo 既定規則「只移植明確被要求、或含 Rust 相關 bug fix 的上游 PR」，本批全部符合後者：

1. **#342 add_clips 自動軌道語意**（資料破壞級 bug）：Rust 自動模式重用第一條既有音軌/視軌，Swift 語意是永遠建立新的 shared tracks。在 #342 的測試情境（先加 linked dialogue、再加 music 全略 trackIndex），Rust 會把 music 放上 dialogue 的音軌，place_clips 的覆寫語意會修剪/移除重疊的 dialogue 音訊。
2. **#307 manage_tracks 定址修復**（工具契約 bug）：index 在 reorder/remove 後漂移。Swift 改以穩定 trackId 定址、reorder 的 zone-clamp 改為硬錯誤、新增 reordered/removedTracks 回執、get_timeline 曝露 trackId、short-id 系統納入 trackId。Rust 全部停在 pre-#307 契約。
3. **#274-followups detect_beats 契約修復**：無音訊影片應前置拒絕（現在落到泛型 decode error）、windowed 呼叫誤報全軌 bpm、空分析/空窗缺 note 欄位、beat_cache 無檔案標記導致換檔後供應舊 beats。
4. **#333 import_media 描述文字同步**（誤導性契約文字）：Rust 描述仍寫「copied into the project in the background / status:'downloading' — poll get_media」，但 Rust 執行器（與上游新契約一致）一直是就地註冊、同步回 ready——文字驅使 agent 做無謂輪詢。
5. **#338 CAF 支援**（跨 app 相容）：Swift 專案可含 .caf 資產，Rust 三條匯入路徑全拒絕。純查表擴充（extension/mime），無平台依賴。
6. **#294 切片：GenerationInput.targetLanguage**（media.json 資料遺失）：Swift 寫入的欄位經 Fronda 開啟→存檔會默默消失。additive serde 欄位，同 #136/#216 模式。
7. **#336 切片：TextStyle isUnderlined/isStruckThrough/isOverlined**（project.json 資料遺失）：TextStyleWire 橋接缺欄位，round-trip 掉資料，同 #65 isBold/isItalic 的缺口類型。
8. **#330 切片：TextStyle tracking/lineSpacing/fontCase/border.width/rich Background**（project.json 資料遺失）：Swift v0.6.9 起 TextStyle 寫入這批欄位（Background 擴為 paddingX/Y、cornerRadius、offsetX/Y、outlineColor/Width 等 9 欄），Rust round-trip 全部丟失。
9. **#338/#274 的回歸測試**：CAF 的 from_extension 表格測試、detect_beats 契約 e2e 測試（transplant 上游測試意圖）。

## Proposed Solution

依 audit 證據逐項對齊（詳 design.md）。切割原則：**只做 bug fix 與 on-disk/工具契約相容**；渲染語意（#336/#330 的畫線、fontCase 顯示、tracking kerning）、nested style agent args（#330）、generate_audio 全量契約（#294）維持後續 feature 決策。

## Non-Goals

- #294 的 generate_audio 工具全量契約（sourceMediaRef/cleanup/dubbing gating、list_models 欄位、catalog/payload 擴充）— feature 決策
- #330 的 nested style agent args 與渲染/FCPXML 語意 — feature 決策
- #336 的 underline/strikethrough/overline 渲染與 inspector UI — feature 決策
- #299 manage_project 工具整併、#298 export queue、#269 sync 引擎強化 — feature 決策
- UI tier3（#281 timeline 配色、#327 面板重設計、#319 settings/help、#280 fade handles、#284-leftover picker）— 獨立 UI parity 決策

## Impact

- Affected specs: 新 capability `upstream-v0610-compat`（9 條 requirement）；`specs/rust-rewrite/97-upstream-pr-audit.md` 補 2026-07-17 re-audit 節；CLAUDE.md porting table 增列
- Affected code:
  - Modified: crates/agent_contract/src/tool_exec.rs（#342 auto-track、#307 manage_tracks、#274f detect_beats、#333/#338 匯入字串）
  - Modified: crates/agent_contract/src/tools.rs（#307/#333/#338/#342 schema 與描述）
  - Modified: crates/agent_contract/src/mutation.rs（#307 validate_manage_tracks）
  - Modified: crates/agent_contract/src/id_short.rs（#307 trackId scalar key）
  - Modified: crates/audio_core/src/beat_detector.rs（#274f estimate_bpm 純函式）
  - Modified: crates/media_library/src/lib.rs（#338 SupportedExtensions::AUDIO）
  - Modified: crates/core_model/src/timeline.rs（#338 ClipType::from_extension + content_type_for_extension；#336/#330 TextStyle/TextStyleWire 欄位）
  - Modified: crates/core_model/src/media_manifest.rs（#294 GenerationInput.target_language）

## 1. Seam 與映射

- [x] 1.1 crates/agent_contract/src/tool_exec.rs：WordStamp{word, start_seconds, end_seconds} 與 TranscriptionProvider trait（transcribe(&MediaSource, language: Option<&str>) -> Result<Vec<WordStamp>, String>）；executor optional 掛載 + setter
- [x] 1.2 純映射函式（timeline_core::word_cut 或鄰近模組）：word stamps + clip placement（start_frame、trim_start_frame、speed、fps）→ 專案 frame 詞列；表格例（trim 偏移、speed≠1、多 clip 串接）單元測試釘住 → `timeline_core::word_cut::map_word_stamps`（以既有 `span_frames` 為單詞映射核心，6 tests 含 spec 表兩列）
- [x] 1.3 協調流程：對 timeline 音訊承載 clips 逐一 transcribe（語言取 Timeline.transcription_language）、映射合併後寫入 executor timeline words；provider 缺席時不動現有 set_timeline_words 邊界（既有測試不得回歸）→ `ToolExecutor::transcribe_timeline`（同一 source 只轉錄一次、timeline order 全域 index、provider 錯誤 atomic 不覆蓋既有詞）；get_transcript 讀同一儲存回真詞（空儲存輸出與原行為一致）

## 2. 觸發面決策與接線

- [x] 2.1 查上游 ToolDefinitions 是否有 transcribe 類工具：git show upstream/main 下的 ToolDefinitions.swift——有則照上游契約加工具（工具數斷言連動），無則僅提供 hub/host 呼叫的 API，不擴充工具面（記錄決策）→ **決策：上游（`Sources/PalmierPro/Agent/Tools/ToolDefinitions.swift`，48 tools）無獨立 transcribe 工具——轉錄內嵌於 get_transcript / add_captions / inspect_media 的描述。故不加工具，僅提供 `ToolExecutor::transcribe_timeline` pub API 供 host UI 觸發；tools.rs 與工具數斷言未動。**
- [x] 2.2 mock provider 測試：全鏈路（transcribe → 映射 → get_transcript 回真詞）→ `transcribe_timeline_full_chain_to_get_transcript`（含 remove_words 同儲存驗證）+ 語言傳遞 / 無 provider / 無音訊 clips / 共用 source 快取 / 錯誤 atomic 共 8 tests

## 3. Host 模型（gated）

- [x] 3.1 whisper 系或平台 STT 的 host adapter 為獨立後續 change；本 change 完成至 seam 邊界（決策維持：host adapter 不在本 change 實作）
- [x] 3.2 cargo test --workspace EXIT=0（2026-07-10 驗證）

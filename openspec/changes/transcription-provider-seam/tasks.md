## 1. Seam 與映射

- [ ] 1.1 crates/agent_contract/src/tool_exec.rs：WordStamp{word, start_seconds, end_seconds} 與 TranscriptionProvider trait（transcribe(&MediaSource, language: Option<&str>) -> Result<Vec<WordStamp>, String>）；executor optional 掛載 + setter
- [ ] 1.2 純映射函式（timeline_core::word_cut 或鄰近模組）：word stamps + clip placement（start_frame、trim_start_frame、speed、fps）→ 專案 frame 詞列；表格例（trim 偏移、speed≠1、多 clip 串接）單元測試釘住
- [ ] 1.3 協調流程：對 timeline 音訊承載 clips 逐一 transcribe（語言取 Timeline.transcription_language）、映射合併後寫入 executor timeline words；provider 缺席時不動現有 set_timeline_words 邊界（既有測試不得回歸）

## 2. 觸發面決策與接線

- [ ] 2.1 查上游 ToolDefinitions 是否有 transcribe 類工具：git show upstream/main 下的 ToolDefinitions.swift——有則照上游契約加工具（工具數斷言連動），無則僅提供 hub/host 呼叫的 API，不擴充工具面（記錄決策）
- [ ] 2.2 mock provider 測試：全鏈路（transcribe → 映射 → get_transcript 回真詞）

## 3. Host 模型（gated）

- [ ] 3.1 whisper 系或平台 STT 的 host adapter 為獨立後續 change；本 change 完成至 seam 邊界
- [ ] 3.2 cargo test --workspace EXIT=0

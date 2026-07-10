## Summary

97-audit 剩餘 M/S 級上游項目打包：autosave（#211）、10-bit HDR 匯出（#138）、viewer guides（#169/#167）、duplicate_clips 工具（#176）、變數字型 wght 渲染（#65 殘餘）、aspect 標籤人性化（#284）、專案卡 Duplicate 選單項（#67）、arrow/line 形狀光柵化（#45）、快捷鍵 parity 增補（#164）。

## Motivation

Multicam 落地後這些是「Swift 有 Rust 沒有」的最後功能塊（XL 的 XML import 另案）。逐項小，合批一次收。

## Proposed Solution

各項獨立實作（tasks 標 [P]）：autosave = hub 層 debounced 髒標記存檔 + 關專案時必存（Swift 語意）；HDR = video_export 的 HEVC Main10 + BT.2020/HLG 色彩標記 + ExportOptions/UI 選項；guides = preview 的 SMPTE 安全區/格式參考線 overlay 選單；duplicate_clips = 上游 #176 契約（完整保真複製，envelope/short-id 整合）；wght = render_core::text 的變數字型軸（ab_glyph 支援度查證，不支援則記錄阻擋）；#284 = list_models 的 aspect 顯示標籤 helper；#67 = 專案卡右鍵選單加 Duplicate（duplicate_project 工具既有）；#45 = compositor 的 arrow/line（端點座標空間以 Swift 光柵化程式碼為準查證）；#164 = 對照 Swift 快捷鍵表補缺（global_shortcuts/menu 既有架構）。

## Non-Goals

- XML/FCPXML 匯入（#154，獨立 XL）
- 生成/轉錄等 host-gated 服務

## Impact

- Affected specs: (per-item, 小型不逐一開 spec)
- Affected code:
  - Modified: crates/app_shell_gpui/src/editor_state_hub.rs, crates/app_shell_gpui/src/video_export.rs, crates/app_shell_gpui/src/export_model.rs, crates/app_shell_gpui/src/export_view.rs, crates/app_shell_gpui/src/preview_view.rs, crates/app_shell_gpui/src/app_root.rs, crates/app_shell_gpui/src/global_shortcuts.rs, crates/app_shell_gpui/src/menu.rs, crates/agent_contract/src/tool_exec.rs, crates/agent_contract/src/tools.rs, crates/render_core/src/text.rs, crates/render_core/src/compositor.rs, crates/generation_core/src/model_catalog.rs
  - New: (none)
  - Removed: (none)

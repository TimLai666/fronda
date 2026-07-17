## Context

Swift in-tree v0.6.10：SettingsView 各 pane、SkillsPane＋SkillDetailSheet、MCPInstructionsPane、ShortcutsPane、HomeView sidebar。Rust 對應 view 鏡射舊版；skill_store 唯讀（#199 PARTIAL）。#327 的 panel_components 可重用。

## Goals / Non-Goals

**Goals:** pane 結構/順序/群組與 Swift 一致；Skills 清單＋編輯（name/description/內容，save 先 parse 驗證再寫檔回傳 Result）；MCP help 新版式。**Non-Goals:** 見 proposal。

## Decisions

- 佈局結構對照 in-tree Swift 檔逐 pane 移植；可重用 panel_components 的群組/row 元件。
- `SkillStore::save`：路徑安全（僅寫入 skills 目錄內該 id 的 .md）、先以 required_fields 驗證 frontmatter、原子寫；失敗回 Err 訊息供 UI 顯示。編輯 UI 用既有 text_field/text_area（IME 安全）。
- agent logos：優先 SVG 內嵌（assets 慣例＋every_icon 測試自動涵蓋）；無合適 SVG 就文字 fallback 並回報。

## Implementation Contract

- 各 pane 群組結構列於回報對照表；skill save 的驗證/原子性/路徑安全有 pure 測試；載入-編輯-儲存 round-trip 測試。
- `cargo test -p fronda-app-shell-gpui` 兩相全綠、desktop check 通過。

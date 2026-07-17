## Summary

移植上游 #319（`4716e17f`，30 檔）：Settings panes 重排、全新 Skills 設定 UI（清單＋SkillDetailSheet 編輯器＋`SkillStore.save` 先驗證再寫檔）、MCP help 重設計、Shortcuts 排版、Home sidebar row 統一。視窗尺寸與 skill 驗證兩個 S 子項已先行落地（change `timeline-colors-window-sizes`）。

## Non-Goals

- ExternalAgentMenu 的外部 app 偵測（Claude/Codex/Cursor 開啟 skill）——需平台 adapter，列 follow-up
- agent logos 資產若上游為點陣圖且不宜內嵌，以文字替代並記錄

## Impact

- Affected specs: 無 delta（視覺 parity）
- Affected code: crates/app_shell_gpui/src/{settings_view.rs,help_view.rs,home_view.rs,skill_store.rs}＋必要新元件/資產

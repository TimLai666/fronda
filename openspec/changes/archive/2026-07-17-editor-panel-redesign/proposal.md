## Summary

移植上游 #327（`cfe9c18f`，24 檔 SwiftUI）editor 面板重設計：共用 EditorPanelGroup（可折疊群組＋header accessory＋reset）、EditorActionFooter、EditorPanelControls（EditorMenuValue/editorValueField/主按鈕樣式）、Inspector 各 tab（Video/Audio/Text/Adjust/AIEdit/Multicam）與 MediaPanel（Music/Speech/Caption tabs）重構、AppTheme 新常數。同場完成 text-style change 記錄的 follow-up：inspector 的 update_text 呼叫從 4 個 flat 相容鍵遷移到 nested style（遷移後 agent_contract 可在後續移除相容鍵）。

## Motivation

repo 規則要求與 Swift 視覺完全一致；Swift baseline 已 merge v0.6.10 in-tree，Rust inspector/media panel 仍鏡射舊版。audit 判定 PORT tier3-ui（M）。

## Non-Goals

- agent_contract 的 flat 相容鍵移除（inspector 遷移後的下一步，讓兩邊獨立可回滾）
- #319 settings/help/home 重設計（獨立 change）
- Multicam tab 的功能實作（UI 殼依 Swift，功能受既有 multicam UI deferral）

## Impact

- Affected specs: 無 delta（視覺 parity，THM/UIX 條文如有數值變更隨手修訂）
- Affected code: crates/app_shell_gpui/src/{inspector_view.rs,media_panel_view.rs,theme.rs}＋新共用元件檔；不動 agent_contract

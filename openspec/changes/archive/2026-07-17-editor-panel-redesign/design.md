## Context

Swift in-tree（v0.6.10）：EditorPanelGroup.swift、EditorActionFooter、EditorPanelControls、Inspector/*、MediaPanel/*。Rust：inspector_view.rs／media_panel_view.rs 鏡射 pre-#327 版；theme.rs 需補 #327 新 AppTheme 常數。

## Goals / Non-Goals

**Goals:** 版面結構、群組折疊、常數值與 Swift 一致；inspector update_text 改送 nested style。**Non-Goals:** 見 proposal。

## Decisions

- 共用元件放新檔 `panel_components.rs`（EditorPanelGroup/ActionFooter/controls 的 gpui 對應），inspector/media panel 改組裝它們；視覺常數從 Swift AppTheme 逐字抄進 theme.rs 對應 enum。
- 折疊狀態 session 內存活（Swift @State 同義），不持久化。
- inspector 的 update_text 呼叫改 nested `style` 物件（fontName/fontSize/color/alignment → style.*）；行為不變，e2e 由既有 inspector 綁定測試調整。
- gpui 互動以編譯＋模型/常數測試驗證（repo 慣例），視覺人工後補。

## Implementation Contract

- inspector 六 tab 與 media panel 三 tab 的群組結構/標題/順序對照 Swift 檔逐一列於回報；新常數有測試。
- update_text 呼叫 payload 含 nested style、無 flat 鍵（grep 斷言或單元測試釘住）。
- `cargo test -p fronda-app-shell-gpui` 全綠、desktop check 通過。

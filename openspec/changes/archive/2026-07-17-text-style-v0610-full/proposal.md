## Summary

完成 #336/#330 的剩餘半邊（on-disk serde 切片已在 `upstream-v0610-compat-ports` 落地）：文字線型（underline/strikethrough/overline）渲染、#330 的渲染語意（fontCase 顯示轉換、tracking 字距、lineSpacing 行距、rich Background 的 offset/cornerRadius/outline/per-axis padding、border.width 描邊寬）、FCPXML 對應輸出，以及 #330 的 nested partial-patch `style` 物件 agent 契約（add_texts/update_text/add_captions 以 nested style 取代平面欄位、add_captions 移除 textCase）。Swift baseline 已 merge 至 v0.6.10 in-tree，權威參照齊備。

## Motivation

2026-07-17 audit：#330 是 agent 工具契約（preserved compat surface）＋渲染語意變更，#336 渲染是 S-M。切片先行後，資料欄位已在但畫面與工具面不動，Rust 與 Swift 的可觀察行為分歧擴大中。

## Proposed Solution

三塊：(1) render_core/text.rs：線型 bars（underline 基線下、strikethrough x-height 中、overline ascent 上，粗細近似 Swift max(1, 比例)）、fontCase displayText、tracking-as-kern、lineSpacing 加性行距、background_style 的 offset/cornerRadius/outlineStroke/per-axis padding、border_width 取代 border.padding 作 glyph outline 寬。(2) tools.rs/tool_exec.rs：nested `style` 物件（outline/shadow/background 子物件、min/max 驗證、6 位 hex 保留現有 alpha vs 8 位設定 alpha、opacity、affectsLayout→auto-fit）、刪平面欄位、add_captions 刪 textCase。(3) fcpxml_export.rs：strokeWidth=border.width、title text 用 displayText。逐字對齊 `git show 6dd183c0` 與 in-tree Swift（v0.6.10）。

## Non-Goals

- #225 TextAnimator/per-word 動畫 overline（UI-deferred 既有項）
- inspector 的線型/樣式按鈕 UI（UI parity batch）

## Impact

- Affected specs: `upstream-v0610-compat`（ADDED：nested style 契約與渲染 requirement）
- Affected code: crates/render_core/src/{text.rs,fcpxml_export.rs}；crates/agent_contract/src/{tools.rs,tool_exec.rs,mutation.rs}

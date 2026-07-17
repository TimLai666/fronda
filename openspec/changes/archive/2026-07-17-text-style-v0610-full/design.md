## Context

serde 欄位已就位（TextStyle：三線型 bool、tracking/line_spacing f64、font_case String、border_width、TextBackgroundStyle 九欄）。render_core/text.rs 有 per-line loop（ascent/base_y 已算）；ab_glyph 無 underline metrics（Swift 也 fallback max(1, CTFont 值)——Rust 取字級比例近似並測試釘住）。agent 工具面 tools.rs:301-345,426-427 平面欄位、tool_exec.rs 6796-6858/7300-7351 套用點（audit 錨點，行號可能漂移以內容為準）。

## Goals / Non-Goals

**Goals:** 渲染輸出與 agent 契約對齊 Swift v0.6.10；FCPXML strokeWidth/displayText 同步；工具數不變。
**Non-Goals:** 見 proposal。

## Decisions

### 渲染

- displayText：fontCase mixed/uppercase/lowercase → 原文/大寫/小寫（未知 rawValue → 原文，round-trip 保底）。套用於量測與繪製與 FCPXML title 文字。
- tracking：逐字元 advance 加 tracking（canvas 座標，隨 font_size 縮放比照 Swift 實碼——實作時讀 in-tree TextRenderer 確認縮放基準）；與 Rust-native letter_spacing 並存時的優先序照 Swift（Swift 無 letter_spacing——native 欄位僅舊專案，兩者相加或 Swift 欄位優先，讀碼決定並記錄）。
- lineSpacing：行高 += line_spacing（加性，非乘數；與 native line_height 乘數的並存規則同上）。
- 線型 bars：underline y = baseline + ~0.12em、strikethrough y = baseline − 0.5×x-height 近似（0.25em）、overline y = ascent 頂；粗細 max(1px, font_size/18) 近似 Swift；顏色同文字色；隨 shadow 一起偏移繪製（Swift 陰影含線型）。
- background_style：矩形 offset_x/y 平移、corner_radius 圓角、outline_color/width 描邊、padding_x/y 取代舊單值 padding（舊 TextFill.padding 作 fallback：background_style 全預設且舊欄位有值時沿用——相容 pre-#330 專案渲染不變，測試釘住）。
- glyph outline：border_width 取代 border.padding 為描邊寬（舊檔 fallback 同上原則）。

### agent 契約

- add_texts/update_text/add_captions 的 schema 以 nested `style` 物件逐字對齊 6dd183c0（outline{color,width}/shadow{...}/background{...} 子物件、數值 min/max、opacity、affectsLayout）；刪平面欄位；add_captions 刪 textCase。
- hex 解析：6 位 → 保留目標現有 alpha；8 位 → 設定 alpha（Swift 語意）。partial-patch：僅覆寫出現的鍵。
- mutation validator 同步 nested 形；工具數不變（4 處斷言不動）。

### FCPXML

- strokeWidth = border_width；title 文字經 displayText。既有 keyframe/timeMap 輸出不動。

## Implementation Contract

- 渲染測試（render_core 既有 golden/結構測試模式）：三線型各一（bar 位置/存在）、uppercase displayText、tracking>0 增加行寬、lineSpacing 增加行距、background offset/outline 生效、pre-#330 舊欄位 fallback 不回歸（既有 caption background 測試全綠）。
- e2e：add_texts nested style 寫入 TextStyle 對應欄位；update_text partial patch 不動未給鍵；6/8 位 hex alpha 語意；textCase 參數在 add_captions 被拒。
- `cargo test --workspace` 全綠。

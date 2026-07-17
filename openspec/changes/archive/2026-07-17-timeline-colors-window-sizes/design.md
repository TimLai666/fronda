## Context

audit #281：Rust timeline 鏡射 pre-#281（0.38/0.22 alpha 填色、舊 palette、XS 圓角）；已收斂項（左色條、trim handles）Swift 已刪、Rust 從未有，無工作。audit #319：window.rs 仍是 #204 的 880/900；skill_store 只擋空 name。

## Goals / Non-Goals

**Goals:** 四組值與規則對齊 Swift 現行（v0.6.5 in-tree baseline 與 v0.6.10 一致）；THM-007 spec/測試/常數三處同步；window/skill 兩行為有測試釘住。
**Non-Goals:** 見 proposal。

## Decisions

### palette 以 hex 為源

`ui_constants.rs` 存 hex 權威值（從 Swift AppTheme 逐字抄），`theme.rs` 的 Hsla 由 hex 精確換算（不得目測近似）；THM-007 測試斷言 hex 表。新增 `TrackColor::SEQUENCE`（#B9B29A）進 palette 與測試——timeline_view 尚只分 Video/Audio lane，sequence 著色掛上與否依 Swift 現行 clip 著色邏輯（實作時讀 Swift TimelineView 確認 sequence clip 用色路徑，有就接、沒有就只落 palette 常數）。

### clip 樣式規則

填色全不透明（刪 alpha 疊算）；黑 thin 邊框僅當 clip 寬 >= ComponentSize.timelineClipBorderMinWidth(8)；選取 = 白 medium 邊框；圓角 Radius::XS_SM。新 token 依 AppTheme 規則加在對應 enum（Opacity.high=0.70、Border.timelineClip）。

### window 尺寸與 skill 驗證

WindowConfig home/settings 預設 → 1200x800（兩個既有測試更新值）；`parse_skill` 要求 name 與 description 皆非空白（trim 後非空），空 description 的 .md 不收錄——transplant Swift SkillFrontmatterTests 的意圖（有 description 收、空白拒）。

## Implementation Contract

- THM-007：ui_constants 測試斷言新 hex 表；spec 條文同步新值。
- timeline_view：無 alpha 疊色；寬 <8pt 的 clip 無黑框；選取 clip 白 medium 框；圓角 4。（gpui 視覺以編譯+常數測試+實跑抽查驗證。）
- window：`WindowConfig::for_home().default_height == 800`、settings 同（測試釘住）。
- skill_store：空/空白 description → 不收錄（新測試）；既有合法 skills 照收。
- `cargo test --workspace` 全綠。

## Risks / Trade-offs

- [skill 驗證收緊會讓使用者現有無 description 的 skills 靜默消失] → 與 Swift 行為一致（鏡射優先）；skill_store 既有「載入時 eprintln 記略過」慣例沿用，至少留 log。

## Migration Plan

視覺常數與載入條件變更，無資料變更；revert 即回滾。

## Open Questions

（無）

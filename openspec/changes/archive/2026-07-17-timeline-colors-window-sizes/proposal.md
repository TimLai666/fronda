## Summary

兩組小型 Swift parity 對齊（2026-07-17 audit 判定可先行落地的 S 效量項）：#281 timeline clip 視覺（新深色 TrackColor palette、全不透明 clip 填色、新邊框規則、圓角 XS→XS_SM）與 #319 的兩個非視覺 S 子項（視窗預設尺寸 home/settings → 1200x800、skill frontmatter 驗證收緊 name AND description 非空白）。

## Motivation

repo 規則要求與 Swift 視覺完全一致；Rust timeline 目前鏡射 pre-#281 外觀，theme.rs / ui_constants.rs（THM-007）/ spec 三處已 stale。#319 兩個 S 子項是可測的行為分歧（視窗尺寸、skills 收錄條件），先行對齊避免分歧擴大；#319 其餘 UI 重構另批。

## Proposed Solution

依 `git show`（#281：a7994bad 系列；#319：4716e17f）取得權威 hex 值與規則。palette 以 hex 為源（ui_constants.rs），theme.rs Hsla 從 hex 換算；新 token：Border.timelineClip、Opacity.high 0.70、ComponentSize.timelineClipBorderMinWidth 8、TrackColor::SEQUENCE（#B9B29A——palette 一次到位，即使 sequence clip 尚未特別著色）。clip 填色改全不透明、黑 thin 邊框 gated on width>=8、白 medium 選取環、圓角 XS_SM。window.rs home/settings 預設高 → 800（含測試更新）。skill_store parse 收緊 description 非空白（SkillFrontmatterTests 意圖移植）。

## Non-Goals

- #281 尚不適用項（compact duration label、keyframe marker、waveform 色——Rust 對應功能未建，落地時採 #281 形）
- #319 其餘 UI 重構（settings/help/home 重設計、Skills 設定 UI）
- Swift 端無對應的 timeline 視覺發明

## Impact

- Affected specs: specs/rust-rewrite/00-runtime-packaging-design-and-shell.md 的 THM-007 修訂
- Affected code:
  - Modified: crates/app_contract/src/ui_constants.rs（palette hex + THM-007 測試）
  - Modified: crates/app_shell_gpui/src/{theme.rs,timeline_view.rs,window.rs,skill_store.rs}

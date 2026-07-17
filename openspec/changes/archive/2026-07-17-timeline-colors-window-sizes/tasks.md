## 1. 實作

- [x] 1.1 依 design「palette 以 hex 為源」與「clip 樣式規則」實作 spec「Timeline clip visuals match the post-281 Swift palette」：從 `git show`（a7994bad bff5834a f60f0236 57a3994e 與 Swift AppTheme 現行檔）抄權威 hex/規則；ui_constants.rs palette+THM-007 測試、theme.rs Hsla 換算、新 token（Opacity.high、Border.timelineClip、ComponentSize.timelineClipBorderMinWidth、TrackColor::SEQUENCE）、timeline_view.rs 樣式規則、specs/rust-rewrite/00-runtime-packaging-design-and-shell.md THM-007 修訂。先寫常數/規則測試再改值。驗證：`cargo test -p fronda-app-contract` 與 `cargo test -p fronda-app-shell-gpui` 全綠。
- [x] 1.2 依 design「window 尺寸與 skill 驗證」實作 spec「Window defaults and skill frontmatter follow post-319 Swift」：window.rs home/settings → 1200x800（既有測試更新）；skill_store.rs parse 收緊 description 非空白＋略過 log＋新測試（transplant SkillFrontmatterTests 意圖，對照 `git show 4716e17f`）。驗證：`cargo test -p fronda-app-shell-gpui window` 與 `skill` 全綠。
- [x] 1.3 `cargo test --workspace` 全綠、desktop check 通過；AGENTS.md porting table 增列 #281 與 #319 slices。驗證：內容審查。

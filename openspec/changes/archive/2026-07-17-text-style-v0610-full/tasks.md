## 1. 實作

- [x] 1.1 [P] 依 design「渲染」實作 render_core/text.rs 的線型/fontCase/tracking/lineSpacing/background_style/border_width 語意與 fcpxml_export.rs 的 strokeWidth/displayText，實作 spec「Text styling renders and patches per Swift v0.6.10」渲染半邊；先寫渲染結構測試（線型 bar、uppercase、tracking 行寬、lineSpacing 行距、background offset/outline、pre-#330 fallback）再實作；讀 in-tree Swift TextRenderer/TextStyle 決定 tracking/lineSpacing 與 native 欄位並存規則並記錄於回報。驗證：`cargo test -p fronda-render-core` 全綠。
- [x] 1.2 [P] 依 design「agent 契約」實作 nested style 物件（schema 逐字 6dd183c0、平面欄位移除、add_captions textCase 移除、hex alpha 語意、partial patch、validator 同步），實作 spec 的契約半邊；先 transplant 上游測試意圖再實作；受影響既有測試逐一核對後更新。驗證：`cargo test -p fronda-agent-contract` 全綠、4 處工具數斷言不動。
- [x] 1.3 `cargo test --workspace` 全綠、desktop check 通過；AGENTS.md porting table #294/#336/#330 slices 列更新（rest 完成）。驗證：內容審查。

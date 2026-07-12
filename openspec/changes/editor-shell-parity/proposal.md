## Summary

把 editor shell 的面板骨架、面板外殼與尺寸調整行為對齊 Swift `EditorView.swift` 的 split 結構，並補上 Windows 平台的選單快捷鍵與基礎選單列，讓編輯器在視覺與操作上真正可用。

## Motivation

2026-07-11 實測（Windows 11、cargo build 後實跑 + 截圖比對）確認 editor 介面與 Swift 基準有結構性落差，這些落差不在 `specs/rust-rewrite/98-ui-parity-audit.md` 的 16 列盤點範圍內（該盤點逐面板檢查「內部」功能，未涵蓋面板之間的骨架）：

1. **Timeline 未橫跨全寬**：Swift Default 佈局是垂直分割（上 70%：Media | Preview | Inspector 三欄；下 30%：Toolbar + Timeline 橫跨全寬）。Rust 版把 Toolbar + Timeline 塞在中央欄內（固定 200px 高），Media 與 Inspector 全高，剪輯主工作區被壓縮成中央欄寬度。
2. **面板尺寸不可調整**：Swift 全部面板由 NSSplitView divider 拖動調整（並加厚 hit area）。Rust 版只有 timeline 高度可拖，Agent 240 / Media 500 / Inspector 260 皆固定。
3. **面板卡片外殼缺失**：Swift 每個面板是 surface 底色圓角卡片，外圍 panelGap/2 內距落在 base 底色上。Rust 版面板貼死、只有 1px border，空專案時全部深灰面板糊成一片黑。
4. **Media preset 行為發明**：Swift 切 preset 只重排結構、不動 visibility；Rust `pane.rs` 的 `apply_preset(Media)` 直接隱藏 Inspector/Timeline/Agent。
5. **Preview 面板底色錯誤**：Swift preview 面板底是 surface、畫布為置中矩形（zoom < 1 時帶白色邊框）；Rust 整片 `Background::BASE` 黑，空專案時無任何畫布邊界感。
6. **Welcome overlay 溢出**：首次啟動歡迎卡片右下溢出視窗，Get started 按鈕在螢幕外不可點（Swift 為置中卡片）。
7. **Windows 沒有選單入口**：`menu.rs` 已有完整 MenuAction 與快捷鍵表（例如 cmd+N），但 Windows 上既無選單列、快捷鍵也未綁定，File/Edit/View 整面功能（含 Import Timeline、佈局切換、面板開關）無入口。
8. **Media rail 圖示是字母替身**：rail 的 M / C 為文字字母，Swift 用 SF Symbols 圖示。

## Proposed Solution

1. **骨架重寫**（`editor_view.rs`）：照 Swift `EditorView.swift` 的 split 巢狀重建三個 preset。
   - Default：`[Agent] | [上：Media | Preview | Inspector（70%）/ 下：Toolbar + Timeline 全寬（30%）]`
   - Media：`[Agent] | [Media（30% 寬）| 右：（Preview | Inspector）55% / Toolbar + Timeline]`
   - Vertical：`[Agent] | [左：（Media | Inspector）55% / Toolbar + Timeline（50% 寬）| Preview]`
   - Agent 欄永遠是最外層左側兄弟欄，preset 切換不影響它。
2. **面板卡片外殼**（`editor_view.rs`）：每個面板包一層 surface 圓角卡片（`Radius::SM`）+ `PANEL_GAP / 2` 內距 + base 底色，重現 Swift 的 makeHosting panel shell。焦點光環（PanelFocusRing）依賴 pane focus 追蹤系統，本次不做（見 Non-Goals）。
3. **可拖 divider**（`app_root.rs` + `editor_view.rs`）：把既有 TimelineResizeDrag 模式泛化為單一 PanelResizeDrag 機制，支援 agent 寬 / media 寬 / inspector 寬 / timeline 高四個尺寸，divider 即面板間 5px 縫（游標 col-resize / row-resize）。Clamp 對齊 Swift `Constants.swift`：agent 240–640、media ≥ 280 + rail 寬、inspector ≥ 150、preview 寬 ≥ 400；timeline 高沿用既有 100–700。拖動時上限同時受視窗寬度扣除其他欄位與 preview 最小寬的約束。timeline 初始高改為視窗高 30%（Swift setPosition targetH * 0.7），僅在使用者未拖動前套用。
4. **Preview 空狀態**（`preview_view.rs`）：面板底色改 `Background::SURFACE`，畫布區域繪製依 timeline 長寬比 fit 的置中黑色矩形（重現 Swift fitSize + 置中 + 邊框行為的靜態部分）。
5. **Welcome overlay 置中 clamp**（`app_root.rs`）：歡迎卡片改為視窗置中且高度受限（超出時內部允許縮減 hero 區），三顆按鈕必須始終在視窗內。
6. **Windows 選單快捷鍵**（`menu.rs` + `global_shortcuts.rs` + `app_root.rs`）：為 MenuAction 快捷鍵表產生 gpui actions 與 keybindings（cmd 映射為 Windows 的 ctrl），`app_root` 以 on_action 轉發到既有 `perform_menu_action`。輸入欄位聚焦時的行為沿用既有 `input` context 規則（帶修飾鍵的選單快捷鍵不受 `!input` 限制，Ctrl+C/V/X/A/Z 等由 text field 自行處理者除外，遇衝突以 text field 優先）。
7. **Title bar 選單列基礎版**（`titlebar_view.rs`）：title bar 左側加 File / Edit / View / Window / Help 下拉，項目直接取 `menu.rs` 既有選單結構，點擊執行對應 MenuAction，樣式沿用 `context_menu.rs` 的深色選單。macOS 上維持系統選單（gpui_platform 既有路徑），選單列僅在非 macOS 顯示。
8. **Media rail 圖示**（`media_panel_view.rs` + `assets`）：M / C / 音符字母替換為 SVG 圖示（對照 Swift SF Symbols：photo 系 / captions 系 / music note 系），沿用專案既有 icons 資產風格。

## Non-Goals

- **gpui-ce Windows 平台層缺陷不在本次範圍**：實測發現的點擊事件偶發丟失、最大化後 hit-test 失效、外部 ShowWindow restore 後黑屏、進 editor 後偶發 silent crash，根因位於 gpui-ce 的 `gpui_windows`（`events.rs` 的 input callback take 期間丟事件、NCCALCSIZE 最大化偏移、renderer resize 失敗路徑）。gpui-ce 為未 pin 的外部 git 依賴，修復需 fork 或 vendor，證據量化（連續點擊成功率統計）與修復另開 change 處理。
- **PanelFocusRing 焦點光環**：依賴 pane focus 追蹤系統（Swift `editor.focusedPanel`），Rust 尚無對應狀態，本次不引入。
- **面板 visibility 跨啟動持久化（EDT-003）**：現況未實作，不在本次補。
- **Preview 畫布的即時合成內容**：僅修空狀態的底色與畫布邊界，合成 PNG 管線不動。
- **macOS / Linux 的選單行為**：僅針對 Windows 補入口，macOS 系統選單路徑不動。

## Alternatives Considered

- **直接修 gpui-ce 再做 UI**：互動可靠性是「沒辦法用」的一部分，但 fork 外部依賴的成本與風險高、且合成輸入的測試證據尚不足以定罪平台層；先完成 app 層確定可修的骨架與入口，再量化重測。
- **用絕對定位重現 NSSplitView**：gpui 的 flex 佈局已足以表達 Swift 的巢狀 split（固定尺寸欄 + flex_1 主欄），引入絕對定位反而增加維護成本。
- **把 Toolbar 併入 Timeline 面板內部**：Swift 的 timelineHC 本來就是 Toolbar + TimelineContainer 的 VStack，維持這個組合放在下半欄即可，不需改 toolbar_view 本身。

## Impact

- Affected specs: 新 capability `editor-shell-layout`；同步在 specs/rust-rewrite/03-timeline-editor-and-preview.md 的 J 節補 EDT-006（骨架結構）、EDT-007（卡片外殼）、EDT-008（divider 調整）、EDT-009（Windows 選單入口）並修正 EDT-002 的 preset/visibility 描述。
- Affected code:
  - Modified: crates/app_shell_gpui/src/editor_view.rs
  - Modified: crates/app_shell_gpui/src/pane.rs
  - Modified: crates/app_shell_gpui/src/app_root.rs
  - Modified: crates/app_shell_gpui/src/theme.rs
  - Modified: crates/app_shell_gpui/src/preview_view.rs
  - Modified: crates/app_shell_gpui/src/media_panel_view.rs
  - Modified: crates/app_shell_gpui/src/titlebar_view.rs
  - Modified: crates/app_shell_gpui/src/menu.rs
  - Modified: crates/app_shell_gpui/src/global_shortcuts.rs
  - Modified: specs/rust-rewrite/03-timeline-editor-and-preview.md
  - New: crates/app_shell_gpui/assets/icons（media rail 三顆 SVG 圖示，實際檔名以現有 icons 目錄慣例為準）

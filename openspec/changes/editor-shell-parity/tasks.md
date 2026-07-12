## 1. 骨架與 preset 行為（editor_view.rs、pane.rs）

- [x] 1.1 落實「preset 切換不動 visibility」：`crates/app_shell_gpui/src/pane.rs` 的 `apply_preset` 只設定 preset 欄位，實作 spec「Preset switching preserves pane visibility」。先改寫既有 `edt_002_media_preset` 測試（其斷言的隱藏行為是發明的）並新增測試斷言 apply_preset 前後 visibility 逐欄不變，再改實作。驗證：`cargo test -p fronda-app-shell-gpui pane` 全綠。
- [x] 1.2 依 design「用 flex 巢狀重現 NSSplitView 結構」在 `crates/app_shell_gpui/src/editor_view.rs` 新增 pure 的佈局描述層（節點樹：水平/垂直容器、固定尺寸欄、flex 主欄、pane 葉節點），三個 preset 各有建樹函式。先寫測試斷言：Default 樹滿足 spec「Default preset skeleton matches Swift split structure」（上：Media|Preview|Inspector、下：Toolbar+Timeline 全寬）、Media 樹滿足「Media preset skeleton matches Swift split structure」、Vertical 樹滿足「Vertical preset skeleton matches Swift split structure」、三樹皆滿足「Agent column is a preset-independent outer column」（Agent 為最外層左欄）、隱藏面板不出現在樹中且 Timeline 全寬性質在 Media/Inspector 隱藏時仍成立。驗證：新測試名 `default_tree_puts_timeline_full_width`、`media_tree_matches_swift`、`vertical_tree_matches_swift`、`agent_is_outer_column_in_all_presets` 全綠。
- [x] 1.3 讓 render 依 1.2 的描述樹產生 gpui 元素，並依 design「面板卡片外殼函式 pane_card」包裝每個 pane 葉節點，實作 spec「Panel card shell」：surface 圓角卡片 + PANEL_GAP/2 內距 + base 底，相鄰面板間可見 5px base 縫。驗證：`cargo check -p fronda-app-shell-gpui --features desktop-app --bin fronda` 通過，實跑截圖可見卡片縫隙與圓角。

## 2. divider 拖動（app_root.rs、theme.rs）

- [x] 2.1 [P] 依 design「PanelResizeDrag 統一拖動機制」在 `crates/app_shell_gpui/src/editor_view.rs`（或同 crate 新模組）實作 pure 的 clamp 計算函式：輸入拖動目標（AgentWidth/MediaWidth/InspectorWidth/TimelineHeight）、起始尺寸、位移、視窗尺寸與其他側欄現寬，輸出 clamp 後尺寸，落實 spec「Draggable pane dividers」的界限（agent 240–640、media ≥ 280 + rail 寬、inspector ≥ 150、timeline 100–700、preview 寬永不低於 400）。theme.rs 補齊缺少的 Layout 常數（AGENT_PANEL_MAX、MEDIA_PANEL_MIN、INSPECTOR_MIN、PREVIEW_MIN_WIDTH）。先寫邊界值測試（每個目標的 min/max/preview 保護/極端視窗）再實作。驗證：`cargo test -p fronda-app-shell-gpui resize` 全綠。
- [x] 2.2 在 `crates/app_shell_gpui/src/app_root.rs` 把 TimelineResizeDrag 泛化為單一 drag session（目標 + 起點 + 起始尺寸），AppRoot 持有 agent/media/inspector 寬與 timeline 高狀態，divider 縫掛 on_mouse_down 與 resize 游標（垂直縫 col-resize、timeline 縫 ns-resize），全域 on_drag_move 套 2.1 的 clamp；並依 design「timeline 初始高為視窗高 30%」以 sentinel 初值在首次 render 取 viewport 高計算。驗證：實跑拖動四條 divider，各自跟手且不越界、preview 不被擠到 400 以下；重啟後 timeline 初始高約為視窗高 30%。

## 3. 空狀態視覺（preview_view.rs、app_root.rs、media_panel_view.rs）

- [x] 3.1 [P] 依 design「preview 空狀態畫布」把 `crates/app_shell_gpui/src/preview_view.rs` 面板底色改 SURFACE，無合成 PNG 時繪製依 timeline 寬高比 fit、置中的 BASE 色畫布矩形帶 SUBTLE 邊框，實作 spec「Preview empty-state canvas」。驗證：實跑空專案，preview 卡內可見置中畫布矩形，面板底為 surface 灰。
- [x] 3.2 依 design「welcome overlay 置中 clamp」把 `crates/app_shell_gpui/src/app_root.rs` 的歡迎卡片改為視窗置中、max 高受限、hero 區可縮，實作 spec「Welcome overlay stays within the window」。驗證：實跑首次啟動（清 welcome_dismissed 狀態）在 1280×720 與最大化兩種視窗下，卡片與三顆按鈕完整可見。
- [x] 3.3 [P] 依 design「media rail SVG 圖示」新增三顆 SVG 圖示資產並把 `crates/app_shell_gpui/src/media_panel_view.rs` 的 `tab_btn` 從字母參數改為 svg 路徑參數，實作 spec「Media rail tab icons」。驗證：實跑 media rail 顯示三顆圖示、無字母替身，active 樣式不變。

## 4. Windows 選單（menu.rs、global_shortcuts.rs、titlebar_view.rs、app_root.rs）

- [x] 4.1 [P] 依 design「Windows 選單快捷鍵走 gpui actions」在 `crates/app_shell_gpui/src/menu.rs` 提供單一來源的選單結構查詢（五個選單各自的標題、MenuAction 項目、分隔線與快捷鍵提示），並以測試斷言快捷鍵綁定清單排除 text-input 擁有的 ctrl+a/c/v/x/z/y 組合（spec「Windows menu shortcuts」的排除條款）。驗證：`cargo test -p fronda-app-shell-gpui menu` 全綠。
- [x] 4.2 為帶快捷鍵的 MenuAction 宣告 gpui actions 並在 boot 綁 keybinding（cmd → ctrl；`cfg(not(target_os = "macos"))`），`crates/app_shell_gpui/src/app_root.rs` 以 on_action 轉發 `perform_menu_action`，實作 spec「Windows menu shortcuts」。驗證：實跑 Home 按 Ctrl+N 進入 editor（等同 New Project 按鈕）、editor 按 Ctrl+S 觸發儲存路徑（觀察無錯誤）、焦點在 chat 輸入框時 Ctrl+N 仍觸發。
- [x] 4.3 依 design「title bar 選單列（非 macOS）」在 `crates/app_shell_gpui/src/titlebar_view.rs` 渲染 File/Edit/View/Window/Help 下拉（用 `context_menu.rs` 元件、資料取自 4.1 的單一來源、顯示快捷鍵提示），點擊項目執行 MenuAction，實作 spec「Title bar menu bar on non-macOS」。驗證：實跑點 File → New Project 進入 editor；View 內的佈局切換與面板開關項目點擊生效。

## 5. spec 同步與端到端驗證

- [x] 5.1 [P] 在 specs/rust-rewrite/03-timeline-editor-and-preview.md 的 J 節新增 EDT-006（三 preset 的 Swift split 骨架與初始比例）、EDT-007（面板卡片外殼）、EDT-008（divider 拖動與 clamp）、EDT-009（非 macOS 選單快捷鍵與 title bar 選單列），並在 EDT-002 補「preset 切換不改 visibility」註記。驗證：內容審查，條目與本 change spec 的 requirement 一一對應。
- [x] 5.2 端到端驗證：`cargo test --workspace` 全綠、`cargo check -p fronda-app-shell-gpui --features desktop-app --bin fronda` 通過、實跑依序驗證三個 preset 骨架截圖與 Swift 結構一致、卡片縫隙可見、四條 divider 可拖、Ctrl+N 與 title bar 選單可用、welcome 卡片在小視窗完整、media rail 為圖示、preview 空狀態有畫布矩形；同時連續 20 次程式化點擊 New Project 統計成功率，量化 gpui-ce 平台層輸入問題（結果記入 change 附註供後續 change 使用）。驗證：上述每一項逐條打勾記錄於實測筆記。

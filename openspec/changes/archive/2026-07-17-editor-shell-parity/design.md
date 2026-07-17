## Context

Swift `EditorView.swift` 用巢狀 NSSplitView 組出編輯器骨架：最外層是「Agent 欄 | preset 區」的水平分割，preset 區內依 preset 再巢狀分割，且每個面板由 `makeHosting` 包成 surface 圓角卡片（外距 panelGap/2，落在 base 底色上）。所有 divider 可拖，最小值由 `Utilities/Constants.swift` 定義。

Rust `crates/app_shell_gpui/src/editor_view.rs` 目前是早期 scaffold：三個 preset 的巢狀結構與 Swift 不同（最關鍵的是 Default 佈局 Timeline 沒有橫跨全寬）、側欄寬度固定不可拖、無面板卡片外殼。`pane.rs` 的 `apply_preset` 會改寫 visibility（Swift 不會）。Windows 上 `menu.rs` 的選單與快捷鍵表完全沒有入口。

2026-07-11 實測截圖比對已確認上述每一項（見 proposal Motivation）。實作限制：gpui 沒有內建 splitter 元件，但 `app_root.rs` 已有 TimelineResizeDrag（on_mouse_down 記錄 session + 全域 on_drag_move 更新高度）這個可複製的拖動模式；`context_menu.rs` 已有可重用的深色選單元件（deferred + anchored + 外點關閉）。

2026-07-17 首次 macOS 實跑補充兩項事實（見 proposal Motivation 9、10）：`desktop-app` feature 未啟用 `gpui_platform/font-kit`，macOS 只畫幾何、無法 rasterize 字形；macOS 沒有任何選單被註冊（`bind_menu_shortcut_keys` 在 macOS 直接 return、titlebar 選單列被 `cfg` 隱藏、boot 從未呼叫 `App::set_menus`）。gpui-ce 的原生選單設施已存在且完整：`App::set_menus(menus)` 把 `Menu`/`MenuItem` 轉成 NSMenu，選單項的 key equivalent 由 **keymap** 反查（`Keymap::bindings_for_action`），選單觸發經 `on_app_menu_action` → `cx.dispatch_action` 送進 focus path，會命中 app_root 既有的 `on_action::<RunMenuAction>`。視窗的 `performKeyEquivalent` 先走 GPUI keymap dispatch，沒吃掉才輪到選單 key equivalent，不會雙重觸發。

## Goals / Non-Goals

**Goals:**

- editor 骨架三個 preset 的巢狀結構、初始比例、最小尺寸與 Swift `EditorView.swift` 一致
- 面板呈現為 surface 圓角卡片、面板間有 base 底色縫隙（Swift panel shell 視覺）
- Agent / Media / Inspector 寬與 Timeline 高可用滑鼠拖 divider 調整，並套用 Swift 的 clamp
- preset 切換不再改動面板 visibility
- preview 面板空狀態有 surface 底與置中畫布矩形
- welcome overlay 卡片置中且按鈕永遠在視窗內
- Windows 上選單快捷鍵可用、title bar 有基礎選單列
- media rail 三個 tab 用 SVG 圖示
- macOS 文字可見（desktop-app feature 啟用平台字型後端）
- macOS 有原生應用選單，選單項與 Command 快捷鍵 dispatch 與非 macOS title bar 選單相同的動作

**Non-Goals:**

- gpui-ce Windows 平台層缺陷（點擊丟失、最大化 hit-test、restore 黑屏、silent crash）— 另開 change
- PanelFocusRing 焦點光環（需 pane focus 追蹤系統）
- 面板 visibility / 尺寸的跨啟動持久化
- preview 即時合成內容管線
- Linux 選單行為（走非 macOS title bar 選單列，已涵蓋）
- macOS 選單項的動態 checked / disabled 狀態

## Decisions

### 用 flex 巢狀重現 NSSplitView 結構

gpui 沒有 splitter 容器，但 Swift 的三個 preset 都能表達為「固定尺寸欄 + flex_1 主欄」的巢狀 flex：水平向側欄（Agent / Media / Inspector）用固定 px 寬、Preview 用 flex_1；垂直向下半欄（Toolbar + Timeline）用固定 px 高、上半欄 flex_1。捨棄絕對定位方案（維護成本高、且 flex 已足夠）。Swift 的初始位置（Default 上下 70/30、Media 的 media 欄 30% 寬、Vertical 左右 50/50 與左欄內 55/45）轉為初始尺寸值：以視窗尺寸乘比例算出 px 後存入 AppRoot 尺寸狀態，之後拖動覆寫。

### PanelResizeDrag 統一拖動機制

把既有 TimelineResizeDrag 泛化：一個 `PanelResizeTarget` enum（AgentWidth / MediaWidth / InspectorWidth / TimelineHeight / MediaFraction / SplitFraction 依 preset 需要），AppRoot 持有各尺寸欄位與單一進行中 drag session（起點座標 + 起始尺寸 + 目標）。divider 是面板間 5px 的縫（hitbox 即 PANEL_GAP），水平縫 cursor_col_resize、垂直縫 cursor_ns_resize，on_mouse_down 開 session，既有的全域 on_drag_move 統一計算 delta 並套 clamp。clamp 對齊 Swift Constants：agent 240–640、media 下限 280 + rail 寬、inspector 下限 150、timeline 高 100–700；同時上限受「視窗寬 − 其他側欄目前寬 − preview 最小寬 400」約束，避免把 preview 擠到消失。

### 面板卡片外殼函式 pane_card

`editor_view.rs` 新增一個 wrap 函式：外層 `p(PANEL_GAP/2)` + base 底，內層 `bg(SURFACE)` + `rounded(Radius::SM)` + `overflow_hidden` 包住面板內容。相鄰面板各自的外距相加即 Swift 的 5px 縫。現有面板內容自帶的頂層背景色不需先剝除，surface 底只是保證卡片底色一致。deferred 彈出層（context menu、popover）由 gpui 頂層繪製，不受卡片 overflow_hidden 裁切。

### preset 切換不動 visibility

`pane.rs` 的 `apply_preset` 改為只設定 `preset` 欄位。visibility 僅由 toggle_pane / maximize 控制，對齊 Swift（buildLayout 重建時尊重既有 visibility flags）。現有測試 `edt_002_media_preset` 斷言 Media preset 隱藏 inspector/timeline/agent — 該斷言描述的行為本身是發明的，隨本 change 改寫；`specs/rust-rewrite/03-timeline-editor-and-preview.md` 的 `EDT-002` 條目同步補註「preset 切換不改 visibility」。

### timeline 初始高為視窗高 30%

AppRoot 的 timeline_height 初始改為 sentinel（未設定），editor render 首次取得 viewport 高時以 `round(h * 0.3)` 設定（對應 Swift `setPosition(targetH * 0.7)` 的下半欄）。使用者拖動後即為顯式值，不再重算。視窗 resize 不重算（Swift NSSplitView 同樣保持 px 位置由 autoresize 分配，簡化為保持 px 值並 clamp）。

### preview 空狀態畫布

`preview_view.rs` 面板根底色由 BASE 改 SURFACE；畫布區域繪一個依 timeline 寬高比 fit、置中的 BASE 色矩形做為畫布邊界（合成 PNG 存在時圖片本身即畫布，維持既有行為，僅底色不同）。Swift 在 canvasZoom < 1 時畫白色 moderate 透明度邊框 — 這裡對空畫布恆繪 `BorderColors::SUBTLE` 1px 邊框，讓空專案能看出畫布範圍。

### welcome overlay 置中 clamp

welcome 卡片由目前的絕對偏移改為 flex 置中（水平垂直皆置中）、寬 520px、`max_h` 為視窗高減邊距，hero 圖區允許在高度不足時縮小（`min_h(0)` + `flex_shrink`），按鈕列永遠貼卡片底部可見。

### Windows 選單快捷鍵走 gpui actions

照 `global_shortcuts.rs` 既有模式：為每個帶快捷鍵的 MenuAction 宣告 gpui action、在 boot 綁 keybinding，`app_root` 以 on_action 轉發 `perform_menu_action`。Swift 的 cmd 修飾鍵在 keybinding 字串中寫成 ctrl（Windows/Linux 主修飾鍵）；這批 ctrl 綁定以 `cfg(not(target_os = "macos"))` 限制平台（macOS 的 cmd 綁定見下方「macOS 原生選單註冊」決策 — 原「macOS 由系統選單處理」的假設經 2026-07-17 實測不成立）。與文字輸入衝突的組合（ctrl+a/c/v/x/z/y）不在 menu 綁定清單內 — 這些屬 text field 的編輯操作，選單對應項（若有）僅由選單列點擊觸發。

### title bar 選單列（非 macOS）

`titlebar_view.rs` 左側（app 圖示右邊）渲染 File / Edit / View / Window / Help 五個文字按鈕，點擊以 `context_menu.rs` 的既有選單元件在按鈕下方彈出該選單的 MenuAction 項目（含快捷鍵提示文字與分隔線），選擇後轉發 `perform_menu_action`。選單結構資料由 `menu.rs` 提供單一來源（新增一個回傳 (標題, 項目清單) 的函式），避免 titlebar 內硬編碼第二份選單表。

### media rail SVG 圖示

沿用既有 `icons/` 資產與 `transport_btn_svg` 的 svg 渲染模式，新增三顆圖示（media / captions / music，視覺對照 Swift SF Symbols photo.on.rectangle、captions.bubble、music.note 的輪廓風格），`tab_btn` 由文字字母改為 svg path 參數。

### desktop-app 啟用 gpui_platform/font-kit

gpui-ce 官方 examples 透過其開發依賴圖啟用 `gpui_platform/font-kit`；Fronda 的 `desktop-app` feature 只開了 `dep:gpui` 與 `dep:gpui_platform`，平台字型後端是關的，macOS 上能畫幾何但無法 rasterize 字形（文字全部缺失，向量、底色、圖示正常）。修法是一行 feature 宣告：`desktop-app = ["dep:gpui", "dep:gpui_platform", "gpui_platform/font-kit"]`（`Cargo.lock` 隨之把 `zed-font-kit` 加入 `gpui_platform` 的解析依賴）。以回歸測試 `desktop_app_enables_macos_font_backend`（`lib.rs`，include_str! 讀 Cargo.toml 斷言 feature 行包含 `gpui_platform/font-kit`）釘住宣告，先紅後綠驗證過。

### macOS 原生選單註冊

從 `menu.rs` 既有單一來源翻譯成 GPUI 原生選單，boot 時（macOS）呼叫 `App::set_menus`。不新增第二份獨立選單模型、不直接呼叫 AppKit（GPUI 的 `Menu`/`MenuItem` 已足夠）。

- **翻譯層**：新增 `menu.rs` 的 sections 查詢（每組選單依 Swift `MainMenu.swift` 的分隔線分段，`menu_sections(group) -> Vec<Vec<MenuAction>>`，並以測試斷言攤平後與 `all_menus()` 逐項一致，保證單一來源不分岔），與 View 選單的 Layout 子選單短標籤（Swift 是 Layout 子選單，項目標籤 Default / Media / Vertical；titlebar 平鋪版沿用現有 "Layout: Default" 標籤不動）。翻譯函式把 sections 轉成 `gpui::Menu` 樹：組間 `MenuItem::separator()`、一般項 `MenuItem::action(label, RunMenuAction { action })`、Layout 為 `MenuItem::submenu`。頂層五組 = Fronda（App 組，含 About / Check for Updates / Settings / Quit）/ File / Edit / View / Help，對齊 Swift `MainMenu.swift`（Swift 沒有 Window 選單）。
- **dispatch 同路徑**：原生選單觸發走 gpui `on_app_menu_action` → `cx.dispatch_action(RunMenuAction)` → app_root 既有的 `on_action::<RunMenuAction>` → `perform_menu_action`，與非 macOS title bar 選單、Windows Ctrl 快捷鍵同一終點。About / CheckForUpdates / Settings / Quit / EnterFullScreen 等目前為 no-op 的 arm 維持現況（各平台一致的既有缺口，不在本次補）。
- **Command 快捷鍵綁定（macOS）**：`bind_menu_shortcut_keys` 移除 macOS 早退，改為在 macOS 綁 `cmd-` keystroke（`Shortcut` 新增 macOS 字串產生，command → `cmd-`；非 macOS 維持 ctrl 映射不動）。綁定集合 = `menu_keybinding_shortcuts()`（同 Windows 排除條款）以 `None` context 綁定，加上 text-input 擁有的五個組合（cmd+a/c/v/x/z）以 `"!input"` predicate 補「顯示用」綁定 — 這讓 Edit 選單顯示標準 ⌘ key equivalent（keymap 是原生選單 key equivalent 的資料來源），同時文字輸入內 text_field 自己的 `input` context 綁定優先、輸入框外才 dispatch RunMenuAction。
- **無雙重觸發**：gpui_macos 視窗的 `performKeyEquivalent` 先走 GPUI keymap dispatch，命中即回 YES，選單的 key equivalent 不會再觸發；未命中的 chord 照舊落到 `app_root.handle_key_down` 的 route_shortcut（既有路徑，行為不變）。
- **註冊位置**：`main.rs` 的 run closure 內、keybinding 綁定之後（key equivalent 反查依賴 keymap 已就緒）、開窗之前，`cfg!(target_os = "macos")` 時呼叫。翻譯函式放 `global_shortcuts.rs` 旁的新模組（`native_menu.rs`，`#[cfg(feature = "desktop-app")]`），結構測試（組數、組名、項目順序、Layout 子選單、action payload 為對應 RunMenuAction）跟著模組走，以 `--features desktop-app` 執行。
- **boot 時聚焦 root（2026-07-17 macOS 實測發現，跨平台修復）**：gpui 的 keystroke dispatch 與選單動作 availability（`is_action_available`，決定 NSMenuItem enabled）都沿 focus path 解析；`open_main_window` 原本從不 focus 任何元素，dispatch 從 synthetic root 出發、不含 root div 的 listeners — 實測結果是 macOS 全部選單項呈灰色停用、⌘ 快捷鍵與 raw key_down 全部無效。修法：開窗 closure 內 `window.focus(&root.focus_handle, cx)`。此修同時讓各平台「開機第一次點擊前」的快捷鍵生效。實證：修前 System Events 讀 menu item `enabled=false`、修後 `true`，選單點擊與 ⌘1/⌘2 均單次 dispatch（debug log 各一行，無雙重觸發）。

### 5.2 macOS 實測發現的兩個既有視覺缺陷

- **preview 空狀態畫布被透明合成 PNG 蓋掉**：空 timeline 的合成結果是全透明 PNG，`frame_png` 為 `Some` 使空狀態分支永不執行，SURFACE 直接透出 — spec「Preview empty-state canvas」情境在真機不成立（3.1 當時的驗證結論有誤）。修法：把 timeline-aspect 畫布矩形（BASE + SUBTLE 邊框）改為恆繪於合成影像下層並隨 canvas_zoom 縮放；有內容時被影像蓋住、空/透明時可見。實證：修前 preview 內部整列 SURFACE（r=22），修後可測得邊框（r=40）+ BASE 內部（r=10）帶狀樣式。
- **assets.rs 漏嵌四顆 SVG**：`FrondaAssets` 用顯式 `include_bytes!` 清單，`captions.svg`、`music_note.svg`（media rail tabs）與 `eye_slash.svg`、`speaker_slash.svg`（timeline 靜音/隱藏徽章）在磁碟上存在但未列入，`svg().path()` 靜默不畫 — 所有平台皆壞，spec「Media rail tab icons」的「三顆 tab 圖示」實際只有 active 的 folder 可見。修法：補嵌四顆 + 回歸測試 `every_icon_on_disk_is_embedded`（列舉 assets/icons/*.svg 逐一斷言可載入，未來新增圖示漏嵌會直接紅）。

## Implementation Contract

- **骨架**：Editor 開啟（Default preset、全部面板可見）時，由上而下依序是 title bar、上半區（左→右：Agent 卡、Media 卡、Preview 卡、Inspector 卡）、下半區（Toolbar + Timeline 卡橫跨 Agent 右緣到視窗右緣的全寬）。下半區高度初始為視窗高（扣 title bar）的 30%。Media preset 時 Media 卡佔左 30% 寬、右側上下為（Preview | Inspector）與（Toolbar + Timeline）；Vertical preset 時左半是（Media | Inspector）在上、（Toolbar + Timeline）在下，右半整欄為 Preview。三個 preset 下 Agent 欄皆為最外層左欄。驗證：pure 測試斷言各 preset 的結構樹（新增可測的 layout 描述函式），加上實跑截圖目視比對。
- **卡片外殼**：每個面板內容外包 surface 圓角卡片，面板間可見 5px base 底色縫。驗證：截圖目視 + pane_card 單元測試（結構性 smoke）。
- **divider**：滑鼠移到面板間縫隙時游標變 resize 樣式，按住拖動即時改變該側欄寬（或 timeline 高），放開後保持。agent 不可小於 240 或大於 640；media 不可小於 280 + rail 寬；inspector 不可小於 150；任何拖動不得使 preview 寬低於 400（clamp 計算函式以 pure 測試覆蓋邊界值）。
- **preset 與 visibility**：`apply_preset` 前後 `visibility` 欄位值不變（pure 測試）。切 Media preset 後五個面板 visibility 維持切換前狀態。
- **preview 空狀態**：無合成 PNG 時 preview 卡內可見一個依 timeline 寬高比置中的畫布矩形（BASE 底 + SUBTLE 邊框），面板其餘區域為 SURFACE。
- **welcome overlay**：任何視窗尺寸（含 1280×720 小視窗）下卡片完整可見、三顆按鈕可點。驗證：實跑改變視窗大小目視。
- **Windows 選單**：非 macOS 平台，editor 或 home 任一畫面按 Ctrl+N 觸發 NewProject（與 Home 按鈕同路徑）；title bar 可見 File / Edit / View / Window / Help，點開後項目與 `menu.rs` 選單表一致，點擊項目執行對應動作。焦點在文字輸入框時，帶 ctrl 修飾的選單快捷鍵仍觸發（ctrl+a/c/v/x/z/y 除外，這些不進 menu 綁定）。
- **rail 圖示**：media rail 三個 tab 顯示 SVG 圖示而非字母，active tab 沿用既有 active 樣式。
- **macOS 字型**：`cargo build -p fronda-app-shell-gpui --features desktop-app --bin fronda` 產出的 binary 在 macOS 上所有 UI 文字可見。驗證：回歸測試 `desktop_app_enables_macos_font_backend` 綠 + 實跑截圖。
- **macOS 原生選單**：macOS 實跑後，menu bar 出現 Fronda / File / Edit / View / Help 五組（與 Swift `MainMenu.swift` 相同的組與分段；無 Window 選單）；點選 View → Layout 的 Default / Media / Vertical 切換佈局骨架；File → New Project 等項目與非 macOS title bar 選單行為一致；⌘N / ⌘1 / ⌘2 / ⌘3 等 Command 快捷鍵觸發相同動作；Edit 選單顯示標準 ⌘ key equivalent 且文字輸入框內 ⌘C/⌘V 等仍由輸入框處理。驗證：`native_menu` 結構測試（`--features desktop-app`）+ macOS 互動實測。
- **範圍邊界**：本 change 不修改 gpui-ce 依賴、不動 preview 合成管線、不動 toolbar_view / timeline_view / chat_view / inspector_view 的內部內容，僅動其容器與底色層。macOS 選單不引入動態 checked / disabled 狀態；不動非 macOS 的 titlebar 選單與 ctrl 綁定行為。

## Risks / Trade-offs

- [gpui-ce Windows 點擊事件偶發丟失會讓 divider 拖動看起來「有時抓不到」] → divider hitbox 全高/全寬 5px 已是 Swift 同等大小；平台層問題另案追蹤，本 change 不因此加大縫隙破壞視覺。
- [menu 快捷鍵與面板內既有 key handler 衝突（例如 app_root.handle_key_down 的攔截）] → 綁定走 gpui action dispatch（優先序明確），逐一實測 Ctrl+N / Ctrl+O / Ctrl+S / Ctrl+E 等主要鍵；發現衝突以 text-input 優先原則裁決。
- [timeline 初始高 30% 在極矮視窗下可能小於 TIMELINE_MIN_HEIGHT] → 初始計算同樣過 clamp（min 100）。
- [卡片 overflow_hidden 可能裁掉面板內既有的貼邊陰影或 badge] → 實跑逐面板目視檢查，發現裁切問題時把該面板的浮層改 deferred。
- [Media preset 原本會隱藏三個面板，改為不動 visibility 後，使用者切到 Media preset 的視覺變化幅度變大（五卡全開）] → 這正是 Swift 行為；EDT-002 spec 同步更新說明。
- [macOS cmd 綁定新增後，原本經 `handle_key_down` raw 路徑觸發的 chord 改由 binding 系統先吃掉] → 兩條路徑終點同為 `perform_menu_action`，行為不變；逐一實測 ⌘N / ⌘S / ⌘1-3 確認無重複觸發（`performKeyEquivalent` 命中 binding 即回 YES，選單不再收到）。
- [cmd+a/c/v/x/z 的 `!input` 顯示用綁定在輸入框外觸發 RunMenuAction，其中 Cut / Copy 目前是 no-op] → 與非 macOS title bar 選單點擊 Cut / Copy 的現況一致（同為 no-op），非本次引入的行為差異。
- [gpui-ce 為未 pin 的 git 依賴，`set_menus` / key equivalent 反查行為可能隨上游變動] → 結構測試釘住翻譯層輸出；`desktop_app_enables_macos_font_backend` 釘住 feature 宣告；上游若破壞性變動會在 `cargo check --features desktop-app` 或結構測試先爆。

## Migration Plan

純 UI 行為變更，無資料格式、無持久化 schema 變動。單一 change 一次落地；回滾即 revert 該 commit。AppRoot 新增的尺寸狀態皆 session 內有效，不寫入任何檔案。

## Open Questions

（無 — 面板尺寸持久化、focus ring、gpui-ce 平台修復皆已明確列為 Non-Goals / 後續 change。）

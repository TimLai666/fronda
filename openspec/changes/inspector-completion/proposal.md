## Why

UI parity audit rows 5/8/9/13：Inspector 有四個缺口——Text 分頁完全沒內容（Swift TextTab 225 行 + ColorField/FontPickerField 元件不存在）、Crop 與 Flip 是死文字、Source 資產詮釋資料硬編碼、數值 scrub 列疑似讀本地預設而非選中 clip 的真實值。Inspector 是屬性編輯的主面，這些讓它半殘。

## What Changes

- 共用欄位元件：ColorField（色票 + hex 輸入）與 FontPickerField（字族選單）——Text 分頁與 Captions 分頁共用
- Text 分頁內容：Content 多行欄（text_area）、Font、Size、Opacity、Color、Alignment segmented（L/C/R）、Background color+toggle、Shadow、Stroke、Position 列——綁選中 Text clip 的 TextStyle（core_model 既有）經 set_text_style/update_text 工具寫回
- Crop 列：on/off toggle、aspect 選單（Free/Original/1:1/16:9/...）、數值列綁 clip.crop；Flip 列：H/V toggle 綁 flip_horizontal/vertical（set_clip_properties 既有）
- Source 詮釋資料：真實 File 區（尺寸/大小/路徑自 manifest）、AI badge、Generated 區（generation_input 的 model/aspect/resolution/duration）、Prompt 區 + copy
- 數值 binding 修正：scrub_values 改由選中 clip 的 transform/volume/speed 即時派生（hub snapshot），scrub 寫回經既有工具；每區 reset 按鈕

## Non-Goals

- References strip 縮圖列（依賴 generation references 資料流，隨 generation-panel 後續）
- crop 的畫布上互動編輯（crop_overlay_view 既有，僅接通開關）

## Capabilities

### New Capabilities

- `inspector-text-tab`: Text clip 的完整屬性編輯
- `inspector-binding`: Inspector 讀寫選中 clip 真實屬性（含 Crop/Flip/Source 區）

### Modified Capabilities

(none)

## Impact

- Affected specs: inspector-text-tab、inspector-binding（新增）
- Affected code:
  - New: crates/app_shell_gpui/src/field_components.rs
  - Modified: crates/app_shell_gpui/src/inspector_view.rs, crates/app_shell_gpui/src/lib.rs
  - Removed: (none)

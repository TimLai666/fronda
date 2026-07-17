## Decisions

parser：註解/空行容忍、TITLE 忽略、LUT_3D_SIZE n（2..=128，#296 上限）、DOMAIN_MIN/MAX 預設 0/1、資料列 r g b（紅最快變——.cube 標準序）、列數驗證 n^3、壞檔明確 Err。取樣：輸入 clamp 進 domain、正規化 × (n−1)、三線性 8 鄰插值。compositor：apply_color 之後、blur/vignette 前的效果鏈位置照 Swift ColorVideoCompositor 順序（讀 in-tree Swift 確認）；lut path 由 fetch 側 host 解？——compositor 是純函式，LUT 檔內容以 `Arc<CubeLut>` 進 effect 解析層：executor/render 前置把 path 讀成 LUT（render_sequence 的資源解析點），cache by path。strength：out = lerp(in, lut(in), strength)。

## Implementation Contract

- parser 單元測試：識別 65 點（#296 迴歸）、129 拒絕、domain、壞列數；取樣測試：identity LUT 恆等、已知小 LUT 手算值、strength 0/0.5/1。compositor e2e：合成 frame 過 identity LUT 位元不變、反相 LUT 生效。
- `cargo test -p fronda-render-core` 全綠。

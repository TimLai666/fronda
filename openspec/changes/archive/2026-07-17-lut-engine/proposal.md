## Summary

決策 D5：純 Rust LUT 引擎——render_core 新 `.cube` parser（LUT_3D_SIZE ≤ 128 含 #296 的 65 點、DOMAIN_MIN/MAX、3D 必做/1D 可選）＋ compositor 對 `color.lut {path, strength}` effect 三線性取樣套用（strength 線性混合）。`color.lut` 從存而不用變成真實效果。

## Impact

- Affected specs: 無 delta（03 spec 的 color.lut 註記隨手更新）
- Affected code: crates/render_core/src/{lut.rs(新),compositor.rs}

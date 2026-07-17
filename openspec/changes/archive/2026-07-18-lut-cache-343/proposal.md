## Summary

移植上游 #343（`60d2f525`）：LUT 快取由 path+mtime 改為 path-only 記憶體快取（去掉 compositor 熱路徑上每次 load 的 stat；編輯中的 .cube 需重啟才刷新——上游明確接受的取捨），cache-if-absent 防 stale 並發寫。transplant 上游 `loadUsesMemoryCacheAfterFirstRead` 測試（刪檔後仍從快取供應）。

## Impact

- Affected code: crates/render_core/src/lut.rs（load_cached）＋測試

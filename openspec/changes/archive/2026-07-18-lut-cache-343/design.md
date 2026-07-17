## Decisions

`load_cached`：key 改 path-only；命中直接回；未命中讀檔 parse 後 cache-if-absent（已有人寫入則回既有值——鏡射 cacheIfAbsent）。原 mtime 失效測試改寫為快取存續測試（Swift 同款：寫檔→load→刪檔→load 仍成功且同值）。

## Implementation Contract

- 刪檔後第二次 load 成功且與第一次相同；壞檔第一次失敗不落快取（重試可恢復）。`cargo test -p render_core` 全綠。

## Context

`pane.rs` 的 `PaneLayout.visibility`（HashMap<PaneId, bool>）在 AppRoot 內存活；`preferences.json` 由 `mcp_service` 以自有 key 讀寫（`project_registry_store::fronda_config_dir()`）。EDT-003 spec 已打勾但無實作。

## Goals / Non-Goals

**Goals:** media/inspector/agent 三面板 visibility 跨啟動存續；不影響 preferences.json 其他鍵。
**Non-Goals:** 見 proposal。

## Decisions

### pane_prefs 模組（pure I/O helper）

`load_pane_visibility(path) -> Option<HashMap<PaneId,bool>>`（缺檔/缺鍵/壞 JSON → None，開機用預設）；`save_pane_visibility(path, &map)`：讀既有 JSON object（壞檔視為空 object）、只覆寫 `paneVisibility` 鍵（`{"media":bool,"inspector":bool,"agent":bool}`）、temp+rename 原子寫。鍵名 camelCase 對齊 repo 慣例。timeline/preview 不寫入；載入時只套用三鍵，其餘面板維持預設。

### AppRoot 接線

`open_main_window` 建 AppRoot 後載入套用（在 FRONDA_OPEN_EDITOR seam 之前，seam 覆寫不受影響）；`toggle_pane` 與 unmaximize 恢復 visibility 後呼叫 save（同步、小檔，無需 debounce）。預設路徑 `fronda_config_dir().join("preferences.json")`，測試以臨時路徑注入。

## Implementation Contract

- round-trip：toggle 後存檔、重建 AppRoot 載入 → 三面板狀態一致（pure 測試過 pane_prefs + 注入路徑）。
- preferences.json 內既有他鍵（如 mcp enabled）在 save 後原值保留（測試釘住）。
- 壞 JSON / 缺檔 → 載入回 None、儲存重建檔案，不 panic。
- `cargo test -p fronda-app-shell-gpui` 全綠。

## Risks / Trade-offs

- [與 mcp_service 同檔並寫競態] → 兩者都是主執行緒少量寫；read-modify-write + 原子 rename 已足，不引入鎖。

## Migration Plan

additive 鍵；舊 preferences.json 無鍵 → 預設行為不變。revert 即回滾。

## Open Questions

（無）

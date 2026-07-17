## Summary

決策 D11 清理線：timeline_core `ClipPropertyUpdate` 死 text 欄位移除；#284-leftover 查明（audit 稱「minor picker polish」）並消化或記錄無事可做；`native_menu.rs` unused import 警告清除；本機 clippy 對既有碼的其餘可安全消化警告順帶清（僅限機械性、零行為變更）。

## Impact

- Affected specs: 無 delta
- Affected code: crates/timeline_core/src/clip_properties.rs、crates/agent_contract（呼叫端同步）、crates/app_shell_gpui/src/native_menu.rs、（#284 視查明結果）

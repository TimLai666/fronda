## Summary

決策 D8：AppRoot 追蹤 focused pane（點擊面板→pane id），PanelFocusRing 照 Swift（focused 面板卡片 accent 邊框）；pane_prefs 擴充 `paneSizes` 鍵持久化 agent/media/inspector 寬與 timeline 高（Swift NSSplitView autosave parity），載入時經既有 clamp。

## Impact

- Affected specs: 03 spec EDT-007 focus ring 註記、EDT-003 sizes 註記
- Affected code: crates/app_shell_gpui/src/{app_root.rs,pane_prefs.rs,editor_view.rs,pane.rs}

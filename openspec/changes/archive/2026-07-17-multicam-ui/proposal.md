## Summary

決策 D7：multicam UI 功能化——timeline 的 multicam clip 視覺（camera lane 徽章/紅色 TrackColor::MULTICAM 標記，依 in-tree Swift TimelineView 的 multicam 呈現）與 inspector Multicam tab 從唯讀升級（switch camera＝change_cam、cut＝manage_multicam 的對應 action——經既有共享工具 dispatch，engine/工具已全數就位）。

## Impact

- Affected specs: 無 delta
- Affected code: crates/app_shell_gpui/src/{timeline_view.rs,inspector_view.rs}

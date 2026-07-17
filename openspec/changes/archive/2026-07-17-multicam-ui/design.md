## Decisions

以 in-tree Swift v0.6.10 的 multicam 呈現為準（TimelineView 的 multicam clip 繪製與 MulticamTab 的操作面）：先讀 Swift 列出可移植面，凡 Rust 端 engine/工具已支撐的操作（change_cam、manage_multicam 的 cut/switch）接上；無支撐的（如 audio-correlation sync map 生成）不造殼。操作經 run_tool 走共享 executor（與其他 inspector 操作同模式），undo 天然生效。

## Implementation Contract

- timeline multicam clip 有可辨識視覺（對照 Swift）；Multicam tab 的 switch/cut 操作 e2e（mock 專案含 multicam group，執行後 timeline 變化與 engine 測試預期一致）。
- 兩相全綠、desktop check。

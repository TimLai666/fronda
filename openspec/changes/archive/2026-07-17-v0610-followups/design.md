## Context

四項皆為既有 change 回報中點名的 follow-ups，證據與位置已知。

## Decisions

- capture_date：ffmpeg metadata 讀 `com.apple.quicktime.creationdate`（fallback `creation_time`），ISO8601 → epoch 秒；讀不到回 None；pure 解析函式＋測試，ffmpeg 讀取走既有 ProjectAudioSource 慣例。
- sidebar：render_home 的列改 `sidebar_row_button`（視覺變更僅樣式，行為不變）。
- ai_edit_tab：Scope/AI Enhance/AI Audio 三群組照 Swift AIEditTab，重用 panel_components；綁定不變。
- set_clip_properties：legacy `fontName/fontSize/color/alignment/background/border` 鍵拒絕（unknown-key 路徑），schema/描述確認與上游 v2 一致（上游本就無這些鍵——確認後對齊；若上游 schema 有，如實回報並停）。

## Implementation Contract

- capture-date 解析測試（兩鍵、壞值 None）；sidebar/ai_edit 編譯＋結構測試；set_clip_properties 拒絕測試＋既有測試不回歸。
- `cargo test --workspace` 全綠、desktop check 通過。

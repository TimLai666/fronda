## Summary

移植上游 #298（`03883436`）可取消的 FIFO export queue：app 級任務佇列狀態機（waiting/preparing/exporting/canceling/completed/failed/canceled）、目的地保留（同一輸出路徑不可重複佇列）、取消（進行中與等待中皆可）、staged output（先寫暫存、成功才原子替換，取消/失敗不留半成品）。2026-07-17 audit 判定 PORT tier2（L）。

## Motivation

Rust 現況：Export 按鈕單發、無佇列、無取消；export_project_with_audio 直寫目的檔，中斷會留半成品。上游已以 ExportQueue 取代 busy-flag。純狀態機高度可測，符合 repo「狀態機入 generation_core 類 pure crate」慣例。

## Proposed Solution

(1) 純狀態機 `export_core`（新模組或併入既有 crate——實作時依 workspace 慣例決定，傾向 `generation_core` 旁新 `crates/export_queue` 或 export_model 內純模組）：job 狀態轉移、FIFO 排程（同時最多 1 個 exporting）、目的地保留、取消語意，逐一 transplant 上游 ExportQueueTests。(2) staged output：video_export/audio_export 寫 `<dest>.partial`（或 temp dir）成功後原子 rename，取消檢查點在逐 frame loop（沿用既有 progress callback seam）。(3) UI：export_view 掛佇列（排隊/進度/取消按鈕），依 Swift ExportQueue UI 形。

## Non-Goals

- 多並行 export（Swift 同為序列）
- export 佇列跨啟動持久化（Swift 無）

## Impact

- Affected specs: `upstream-v0610-compat`（ADDED：export queue requirement）
- Affected code: 新純模組（export queue 狀態機）；crates/app_shell_gpui/src/{export_model.rs,export_view.rs,video_export.rs,audio_export.rs}

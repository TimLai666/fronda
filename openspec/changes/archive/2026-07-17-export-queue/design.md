## Context

上游 `03883436`：ExportQueue（@MainActor 單例）+ ExportService 取消 + withStagedOutput。Rust：export_model.rs（純 ExportSettings/編碼選擇）、export_view.rs（UI）、audio_export::export_project_with_audio（逐 frame loop，有 progress seam）。取消需要跨執行緒旗標（AtomicBool）。

## Goals / Non-Goals

**Goals:** 狀態機語意與上游一致（transplant ExportQueueTests）；取消不留半成品；UI 有佇列與取消。
**Non-Goals:** 見 proposal。

## Decisions

### 純狀態機

`ExportQueue`（pure，injected id/clock 不用 Date::now）：`enqueue(dest, settings) -> Result<JobId, DestinationReserved>`、`next_ready()`、`mark_*` 轉移、`cancel(id)`（waiting→canceled 直接；exporting→canceling，宿主確認後 mark_canceled）、`jobs()` 快照。狀態轉移表逐一對齊上游測試（非法轉移 → Err，不 panic）。

### staged output 與取消旗標

export 寫 `dest.with_extension("partial.mp4")`（同目錄保原子 rename 可行），成功 rename 覆蓋；取消/失敗刪暫存。`Arc<AtomicBool>` cancel flag 進 export loop（frame 迴圈與 audio mix 段落各檢查），取消回明確 Err。app 層：gpui background executor 跑 job，完成/取消回主緒 mark。

### UI

export_view：佇列列表（狀態、進度、取消鈕）、目的地衝突錯誤顯示。視覺依 in-tree Swift ExportQueue UI，AppTheme 常數。

## Implementation Contract

- 狀態機：上游 ExportQueueTests 意圖全數移植（FIFO 順序、目的地保留與釋放、waiting/exporting 取消、非法轉移 Err）。
- 取消的 export：目的檔不存在、暫存已清（temp-dir e2e 測試）。
- 成功 export：目的檔存在且完整（沿用既有 re-decode 驗證測試模式）。
- `cargo test --workspace` 全綠；gpui UI 以編譯+模型測試驗證（互動人工後補）。

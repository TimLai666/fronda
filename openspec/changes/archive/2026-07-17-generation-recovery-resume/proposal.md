## Why

上游 #216 讓生成任務可跨啟動恢復。資料模型已完成（audit 2026-07-04：GenerationInput.backend_job_id/output_index/result_urls 與 MediaManifestEntry.generation_status 均已 round-trip），剩恢復邏輯：啟動時掃描 in-flight 任務並重新訂閱後端。後端訂閱需要 GenerationBackend adapter（host-gated），但掃描與狀態機轉移是純邏輯，可先實作到 seam 邊界。

## What Changes

- `generation_core` 新增純恢復規劃：`plan_generation_recovery(manifest) -> Vec<RecoverableJob>`——列出 generation_status 為進行中且有 backend_job_id 的資產，含應採取的動作（重新訂閱、逾時標記失敗）
- `agent_contract`/app 層新增 `GenerationBackend` host seam trait：`resume(job_id) -> JobSubscription` 之類的最小介面（poll 或 callback 由 host 決定，seam 只約定結果回報：成功帶 result_urls、失敗帶原因）
- 結果回報的純套用函式：把後端回報寫回 manifest（status 轉移、result_urls 落地），與 get_media 的 "poll until none" 提示一致
- 啟動接線與實際後端（上游為 Convex 任務）為 host-gated

## Non-Goals

- 不做新生成請求的送出（generate 工具現狀不變）
- 不決定後端傳輸（Convex/輪詢 REST 是 host 決策）

## Capabilities

### New Capabilities

- `generation-recovery`: 專案重開後接續進行中的生成任務——純恢復規劃、狀態轉移套用與 GenerationBackend seam

### Modified Capabilities

(none)

## Impact

- Affected specs: generation-recovery（新增）
- Affected code:
  - New: (none)
  - Modified: crates/generation_core/src/lib.rs, crates/agent_contract/src/tool_exec.rs
  - Removed: (none)

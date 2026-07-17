## 1. 純恢復規劃

- [x] 1.1 crates/generation_core/src/lib.rs：RecoverableJob{asset_id, backend_job_id, action} 與 plan_generation_recovery(&MediaManifest) -> Vec<RecoverableJob>；單元測試覆蓋 in-flight 入列、none/無 job_id 排除、多資產排序穩定；實作 spec「Recovery planning is pure」
- [x] 1.2 outcome 套用：apply_generation_outcome(manifest, asset_id, Outcome::Success{result_urls}|Failure{reason})——status 轉移 + result_urls 寫入；與 MediaManifestEntry.generation_status 既有 serde 欄位一致，round-trip 測試；實作 spec「Backend outcome application」

## 2. Seam

- [x] 2.1 GenerationBackend trait（agent_contract 或 app_contract，依既有 seam 慣例放置）：resume_job(job_id) 的最小介面與 Outcome 型別；executor/hub 掛載點與 setter；無 backend 時恢復規劃仍可執行但動作僅記錄（不報錯——與啟動流程解耦）
- [x] 2.2 mock backend 測試：recovery plan → resume → outcome 套用全鏈路

## 3. Host 接線（gated）

- [x] 3.1 app 啟動（專案載入後）呼叫 plan_generation_recovery 並對每個 job 經 host backend 重新訂閱——實際後端（上游 Convex 任務）待後端選型，本 change 完成至 seam 邊界
- [x] 3.2 cargo test --workspace EXIT=0

## Context

`GenerationBackend`（tool_exec.rs:292）僅 `resume_job`。executor 有 `generation_backend: Option<Arc<dyn GenerationBackend>>`＋`set_generation_backend`。#216 的 `apply_generation_outcome(manifest, asset_id, Outcome)` 把 Success{result_urls}/Failure{reason} 套進 manifest;`plan_generation_recovery` 掃 generation_status=="generating" 且有 backend_job_id 的 entry。anthropic_transport 是 reqwest-blocking-rustls＋config-from-env 的既有範本。

## Decisions

### Seam 擴充（agent_contract + generation_core）

generation_core 新純型別：
- `GenerationRequest { kind: ModelKind, model: String, prompt: String, duration_seconds: Option<f64>, source_url: Option<String>, target_language: Option<String>, params: serde_json::Value }`
- `GenerationSubmission { backend_job_id: String }`

`GenerationBackend` 加 `fn submit(&self, req: &GenerationRequest) -> Result<GenerationSubmission, String>;`（保留 resume_job）。既有 MockGenerationBackend 補 submit。

### cmd_generate_* 送出（複用 #216,不偽造成功）

有後端時：跑既有驗證/gating（audio 已全備;video/image 補齊必要驗證）→ 組 GenerationRequest → submit → 取 backend_job_id → 建一筆 MediaManifestEntry（新 asset id、type 由 kind、generation_input 帶 prompt/model/params/backend_job_id、generation_status="generating"、無本地檔）→ 回非 error 結果含 mediaRef 與「已開始,完成後出現」。無後端時：維持逐字既有 honest error。資產完成由 host 既有 recovery/poll（resume_job）路徑處理——這是誠實的:資產真的 pending,不假成功。

### HTTP adapter（app_shell）+ Protocol v1

`http_generation_backend.rs`：reqwest blocking rustls。config 取 `FRONDA_GENERATION_URL` + `FRONDA_GENERATION_TOKEN`（或 prefs 鍵,沿 pane_prefs 慣例）;缺任一 → 不安裝後端（honest error 維持）。
- submit：`POST {base}/v1/generate` bearer,body `{kind,model,prompt,durationSeconds?,sourceUrl?,targetLanguage?,params?}` → `200 {jobId,status}`;非 2xx/壞 body → Err。
- resume_job：`GET {base}/v1/jobs/{jobId}` bearer → `{status: queued|running|succeeded|failed, resultUrls?, error?}`;succeeded→Success{result_urls};failed→Failure{reason};queued/running→Err（未完成,manifest 保持 generating,下輪 recovery 重試——契合 #216 "Err=無定論則重試"）;不可達→Err。
- request/response 組裝與解析 factored 出純函式,fixture 單元測試;live round-trip 註記需設定端點,不自動測。
Protocol v1 寫入 `specs/rust-rewrite/98-generation-protocol.md`（端點、body、狀態機、錯誤語意）,讓自建服務有明確目標。

### hub 安裝

editor_state_hub 的 install 點（比照 audio_source/matte）:config 齊備時 `exec.set_generation_backend(Arc::new(HttpGenerationBackend::from_config(...)))`;否則不設(既有行為)。

## Implementation Contract

- 無後端:三個 generate 工具回既有 honest error 逐字不變（既有測試不回歸）。
- 有後端(mock):generate_video/image/audio 送出後 manifest 多一筆 generation_status="generating"＋backend_job_id 的 entry,工具回非 error 含 mediaRef;plan_generation_recovery 找得到它;mock resume_job Success 後 apply_generation_outcome 落 result_urls。
- adapter:submit/poll 的 request 組裝與 response 解析各有 fixture 單元測試(2xx/4xx/壞 body/各 status);config 缺失 → from_config 回 None。
- `cargo test --workspace` 全綠;desktop check 通過。

## Risks / Trade-offs

- [live 端點無法自動測] → 同 anthropic_transport 既有取捨;request/response 純函式全測,live 註記。
- [Protocol v1 是本專案自定,非任何現存服務] → 這正是「自建相容服務」的定義;誠實記錄於 spec,不假稱相容 palmier。
- [送出後 host 未跑 recovery tick 則資產永遠 generating] → recovery tick 是 #211/#216 既有 host 責任;文件註記。

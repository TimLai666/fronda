## 1. 目錄資料模型

- [ ] 1.1 對照上游 Swift（git show upstream/main 下的 VideoModelConfig/ImageModelConfig/AudioModelConfig 原始檔）抄錄真實模型清單與欄位到 crates/generation_core：ModelConfig 結構 + 靜態 catalog()；欄位含 paid_only；snapshot 測試釘住清單內容
- [ ] 1.2 gating 純函式：model_available(is_paid, paid_only) 與 require_plan 訊息組裝；單元測試四象限

## 2. 工具接線

- [ ] 2.1 crates/agent_contract/src/tool_exec.rs：cmd_list_models 改讀 generation_core::catalog()，移除占位清單；回應每項帶 available 與升級提示；AccountState seam trait（is_paid() -> bool）+ executor optional 掛載，未掛載視為 free
- [ ] 2.2 generate 工具（cmd_generate 系列）對 gated 模型回明確錯誤（含 require_plan 訊息）
- [ ] 2.3 tools.rs 的 list_models 描述更新（提及 gating 欄位）；相關 snapshot 測試更新

## 3. 驗證

- [ ] 3.1 mock seam 測試：free/paid 兩態的 list_models 與 generate 行為
- [ ] 3.2 cargo test --workspace EXIT=0

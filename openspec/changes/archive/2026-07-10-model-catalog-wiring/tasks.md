## 1. 目錄資料模型

- [x] 1.1 對照上游 Swift（git show upstream/main 下的 VideoModelConfig/ImageModelConfig/AudioModelConfig 原始檔）抄錄真實模型清單與欄位到 crates/generation_core：ModelConfig 結構 + 靜態 catalog()；欄位含 paid_only；snapshot 測試釘住清單內容
  - 註：upstream/main HEAD 的 catalog 已改為 server-driven（Convex `models:list`，commit 9dfde8d 起），client 端不再有寫死清單。實際抄錄來源為 repo 內最後一份完整寫死目錄：`9dfde8d^` 的 `Sources/PalmierPro/Generation/Fal/{Video,Image,Audio}ModelConfig.swift`（10 video + 5 image + 4 audio）。paid_only 鏡射 #249 的 `CatalogEntry.paidOnly`；後端的 paid_only 值不在 repo 內，全部條目為 false（等同 Swift decode 預設）。Upscale 目錄未抄（proposal kind 僅 video/image/audio）。
- [x] 1.2 gating 純函式：model_available(is_paid, paid_only) 與 require_plan 訊息組裝；單元測試四象限

## 2. 工具接線

- [x] 2.1 crates/agent_contract/src/tool_exec.rs：cmd_list_models 改讀 generation_core::catalog()，移除占位清單；回應每項帶 available 與升級提示；AccountState seam trait（is_paid() -> bool）+ executor optional 掛載，未掛載視為 free
- [x] 2.2 generate 工具（cmd_generate 系列）對 gated 模型回明確錯誤（含 require_plan 訊息）
- [x] 2.3 tools.rs 的 list_models 描述更新（提及 gating 欄位）；相關 snapshot 測試更新

## 3. 驗證

- [x] 3.1 mock seam 測試：free/paid 兩態的 list_models 與 generate 行為
  - 註：抄錄的目錄沒有 paid_only=true 條目（見 1.1），gated 路徑以合成 ModelConfig 直接測 executor 的 gate_model/model_entry_json（與 cmd 路徑共用）。
- [x] 3.2 cargo test --workspace EXIT=0

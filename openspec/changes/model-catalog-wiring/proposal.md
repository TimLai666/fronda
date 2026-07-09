## Why

`cmd_list_models` 目前回傳寫死的占位模型清單（audit 2026-07-04 記錄：gen-3/kling/sd3 等假資料），上游的真實目錄（VideoModelConfig.allModels 等）從未載入 agent。上游 #249 的 paid_only 免費層 gating 因此被擋——先 gating 假資料沒有意義。需要先把真實模型目錄接進 agent。

## What Changes

- `generation_core` 定義模型目錄資料模型：ModelConfig{id, display_name, kind(video/image/audio), paid_only, …}與靜態目錄來源（鏡射上游 VideoModelConfig/ImageModelConfig/AudioModelConfig 的欄位與清單內容）
- `cmd_list_models` 改讀真實目錄（依 kind 過濾），移除占位清單
- 加入 #249 的 gating：`model_available = is_paid || !paid_only`；is_paid 來自 host seam（帳號狀態），未接 host 時預設 free tier（保守：paid_only 模型標示為升級可用而非隱藏，依上游行為決定）
- generate 工具對不可用模型回明確錯誤

## Non-Goals

- 不做帳號系統本身（is_paid 的來源是 seam）
- 不做模型呼叫（generation backend 是另一 change）

## Capabilities

### New Capabilities

- `model-catalog`: agent 可見的真實生成模型目錄與 free/paid gating

### Modified Capabilities

(none)

## Impact

- Affected specs: model-catalog（新增）
- Affected code:
  - New: (none)
  - Modified: crates/generation_core/src/lib.rs, crates/agent_contract/src/tool_exec.rs, crates/agent_contract/src/tools.rs
  - Removed: (none)

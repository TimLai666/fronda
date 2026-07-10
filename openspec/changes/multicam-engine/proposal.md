## Why

上游 #283（multicam v2，2,627 行）是最後一個 XL 功能缺口。資料透傳已完成（multicamGroups + Clip.multicamGroupId opaque），tool-surface-v2 為三個 multicam 工具留了位（53→56）。缺：MulticamEngine 純邏輯（284 行 + 570 行測試）、manage_multicam/change_cam/get_multicam 工具、move_clips 的群組移動、VideoLayout 增項。

## What Changes

- multicamGroups 從 opaque Value 升級為型別化 `MulticamSource`（id/name/members{mediaRef, kind angle|mic|both, angleLabel, sync{offsetSeconds, confidence, locked}}/masterMemberId）——serde 鍵照上游、round-trip 與既有 opaque fixture 相容
- `timeline_core::multicam`（或 core_model）：MulticamEngine 移植——群組建立/成員同步偏移套用/切換角度的 clip 替換數學（對照上游 MulticamEngine.swift 與其 570 行測試逐案移植）
- 三工具依 tool-surface-v2 design.md 的保留位契約：manage_multicam（建立/編輯/刪除群組）、change_cam（切角度）、get_multicam（讀群組）；工具數 53→56，host split 同步
- move_clips 移動 multicam 群組成員時整組移動（上游語意）
- manage_tracks 的 multicam guard 從 vacuous 轉真實

## Non-Goals

- Multicam UI（角度檢視器面板）——後續 change
- 錄音同步的音訊相關分析（sync_clips 已有 correlator；multicam sync 欄位由工具寫入）

## Capabilities

### New Capabilities

- `multicam`: 多機位群組——引擎、三工具、群組移動語意

### Modified Capabilities

(none)

## Impact

- Affected specs: multicam（新增）
- Affected code:
  - New: crates/timeline_core/src/multicam.rs
  - Modified: crates/core_model/src/project_file.rs, crates/core_model/src/timeline.rs, crates/agent_contract/src/tool_exec.rs, crates/agent_contract/src/tools.rs, crates/agent_contract/src/mutation.rs, crates/agent_contract/tests/spec_tool_snapshots.rs, crates/mcp_server/tests/spec_mcp_contract.rs
  - Removed: (none)

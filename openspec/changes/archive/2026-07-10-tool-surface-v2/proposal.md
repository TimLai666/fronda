## Summary

依使用者 2026-07-10 決策，把 Rust agent/MCP 工具面收斂到上游 #263 的 v2 契約（48 個合併工具），Rust-native 擴展保留其上。

## Motivation

上游 #263（3,155 行）重構了整個工具面：organize_media 取代 6 個 folder/media 工具（以 path 定位資料夾）、manage_tracks 取代 remove_tracks（reorder/mute/hide/syncLock/remove）、close_project 新增、import_media 吸收 create_matte、create_timeline 吸收 duplicate_timeline、每個 clip 工具回 mutation envelope、get_timeline 改 relationship-first（frames [start,end)、折疊 A/V、caption-group 摘要、gaps）、get_transcript 精簡、instructions 縮 46%。Rust 目前是舊面 64 工具——不收斂則 agent 品質與上游 prompt 工程脫節，且未來每個上游工具改動都要雙軌維護。

## Proposed Solution

分七階段（tasks 對應）：(1) 從 upstream/main@141c69b 的 ToolDefinitions.swift 與 ToolExecutor+*.swift 抄錄 48 工具的精確 schema、mutation envelope 格式、get_timeline v2 輸出形狀與新 SYSTEM_INSTRUCTION 全文——落地為本 change 的 design.md 附錄（契約為王）；(2) 新工具 organize_media/manage_tracks/close_project；(3) 吸收合併與舊工具退場（import_media+matte、create_timeline+duplicate、organize_media 併掉的 6 個）；(4) mutation envelopes；(5) get_timeline/get_transcript v2；(6) SYSTEM_INSTRUCTION 重寫（上游全文 + Rust 擴展段）；(7) 工具數斷言/snapshot/MCP 契約測試全面更新。Rust-native 擴展（compound、clip presets、add_shapes、set_blend_mode、timeline 工具等上游沒有的）保留並在 instructions 中明確分段。

## Non-Goals

- multicam 三工具（manage_multicam/change_cam/get_multicam——multicam 引擎 change 的範疇，但本 change 的面為其留位）
- 上游 server-side 模型目錄化（我們保留本地 catalog）

## Alternatives Considered

維持現面 + 僅 port 上游 bug fix：被使用者否決——雙軌維護成本與 agent 品質脫節。

## Impact

- Affected specs: agent-tool-surface（新增 v2 契約 spec）
- Affected code:
  - Modified: crates/agent_contract/src/tools.rs, crates/agent_contract/src/tool_exec.rs, crates/agent_contract/src/mutation.rs, crates/agent_contract/tests/spec_tool_snapshots.rs, crates/mcp_server/src/server.rs, crates/mcp_server/tests/spec_mcp_contract.rs, crates/app_shell_gpui/src/media_panel_view.rs
  - New: (none)
  - Removed: (none)

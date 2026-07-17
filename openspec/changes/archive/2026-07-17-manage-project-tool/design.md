## Context

Rust 現況：4 個 MCP-only 工具（`mcp_tools()` 56 = shared 52 + 4）。上游 `b8a1491d` 併為 manage_project：action enum 必填、逐 action 的 key 白名單、selector 三選一。`ProjectNavigator`（app_shell_gpui）是 host seam：open(id/path)、close 已有 name 解析（大小寫不敏感 + 重名 ambiguity 錯誤）。Fronda 單一開啟專案模型（project_navigator.rs 註明），session/visible 分歧不可能發生。

## Goals / Non-Goals

**Goals:** 工具面與上游逐字一致；MCP 56→53；e2e 測試覆蓋每個 action 的成功與驗證失敗路徑。
**Non-Goals:** 見 proposal。

## Decisions

### manage_project 契約逐字對齊 b8a1491d

- schema：action required enum [list, open, create, close]；open 收 name|id|path 三選一（多給或不給 → 錯誤）；create 收 name（型別檢查）；close 收 name|id 或無參數（關目前專案，沿用現行 close 語意）；逐 action unknown-key 驗證（Swift 對 manage_project 有做——與 Rust validator 一貫「不做 unknown-key」慣例衝突時，此工具特例照 Swift 做，註解引 b8a1491d）。
- id 驗證：UUID 格式檢查（Rust registry id 也是 UUID 字串，直接對映）。
- open-by-name：`ProjectNavigator` trait 加 name 定址（借用 close 的 case-insensitive + ambiguity 實作模式）；trait 變更需同步 mock 與 app 實作。
- list payload：每項含 visible 欄位——單專案模型下輸出恆等於 active（寫入 spec scenario，Swift 多視窗語意的 session 隔離為 #250 follow-up）。
- projectNavigation 指示（mcp_instructions）改寫為上游新文。
- 工具數：shared 52 不變；`mcp_tools()` 56→53；in-app 53 不變（原四工具是 MCP-only）。4 處 count 斷言同步。

### 錯誤訊息與回傳形

錯誤訊息逐字對齊上游；open/create/close 的成功回傳沿用現行文字形（不補 snapshot——pre-existing 分歧）。get_projects 的列表形狀改為 manage_project list 的上游形。

## Implementation Contract

- `mcp_tools()` 含 manage_project、不含四舊工具；in-app 面不變。
- e2e（過 executor.execute + mock navigator）：list 輸出含 visible==active；open by name 大小寫不敏感、重名 → ambiguity 錯誤、unknown key → 錯誤、id 非 UUID → 錯誤；create/close 成功與驗證失敗各一。
- MCP server 的 initialize instructions 含新 projectNavigation 文。
- `cargo test --workspace` 全綠。

## Risks / Trade-offs

- [MCP 客戶端若寫死舊工具名會壞] → 上游已定案同名遷移；鏡射政策優先，porting table 記錄 56→53。
- [unknown-key 特例與 validator 慣例分歧] → 侷限在 manage_project 一支，註解標明。

## Migration Plan

工具面契約變更（跟隨上游），無資料變更；revert 即回滾。

## Open Questions

（無）

## Why

`xml_import.rs` 的 FCPXML 分支目前回 `NotImplemented`——那是 XMEML 匯入 change（已封存 `2026-07-11-xml-import-xmeml`）明列的「另案」。FCPXML 是 Fronda 自己的匯出格式之一（`fcpxml_export.rs`），可用 round-trip（export→parse）當黃金驗證，是把「匯入」從半套變成兩種格式都能吃的自然延伸，也是超越 Swift 的一項（export 兩邊都有，import 兩邊都沒有）。

## What Changes

- `render_core/src/xml_import.rs` 新增 `parse_fcpxml(content) -> Result<ImportedTimeline, XmlImportError>`，反轉 `fcpxml_export.rs`：
  - `<resources>` 的 `<format>`（由 `frameDuration` 反推 fps）與 `<asset>`（id/name/`start` 時間碼原點/hasAudio + `<media-rep src>`）建成查表。
  - 範圍限定在 `<project>` 內的 `<sequence>`，避免 `<resources>` 內巢狀 media 的 `<sequence>` 搶先命中。
  - `<spine>` 的錨定 `<gap>` 內每個 lane 連接的 `<asset-clip>`（ref/lane/offset/duration/start）→ Clip。
  - lane 反轉還原軌道：video 正 lane 由高到低即 tracks[0..]（頂層在前）、audio 負 lane 依序 -1/-2/…，精確對上 `lane_of_track` 的分配。
  - rational time 解析器 `parse_rational_seconds`（`"N/Ds"`/`"Ns"`/`"0s"`）→ 秒 →（×fps 四捨五入）frames。
  - 1× 片段：`trim_start = clip.start − asset 時間碼原點`；retimed（有 `<timeMap>`）以 1× 匯入並記 note。
- `import_xml` 的 `Fcpxml` 分支由 `NotImplemented` 改為呼叫 `parse_fcpxml`。
- 沿用 XMEML 既有的無依賴 tag scanner（`xml_blocks`/`attr`/`first_inner`）；不新增 XML crate。

## Non-Goals

- Premiere/Resolve XML 解析（維持 `NotImplemented`）。
- 媒體實體 relink（回傳 `ReferencedFile` 清單供 host 依檔名解析；純核心不碰檔案系統）。
- retimed 片段的精確 timeMap 反轉、關鍵影格/濾鏡/`<title>` 文字覆蓋/巢狀 `<ref-clip>` 還原（記 note，先做計時+結構+檔案參考）。
- App 選單「Import Timeline」入口與把 `ImportedTimeline` 併入專案（host 整合，另案）。

## Capabilities

### New Capabilities

- `xml-timeline-import`：把 FCP7 XMEML 與 FCPXML 匯出檔還原成 `Timeline` + 檔案參考清單的純核心解析能力（本 change 補上 FCPXML 分支，與既有 XMEML 分支共用契約）。

## Impact

- Affected code:
  - Modified: `crates/render_core/src/xml_import.rs`（新增 `parse_fcpxml` + rational-time helper + `import_xml` 分派；6 個新測試）
- 無 on-disk 契約變更、無新依賴、無公開 API 破壞（`ImportedTimeline`/`XmlImportError` 型別不變）。

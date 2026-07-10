## Summary

實作 XMEML（FCP7 XML）匯入解析器——把 render_core 既有的 xml_import 模型（格式偵測/request/error/validate，無解析器）補上真正的 parse，反轉 xml_export 的輸出成 Timeline。這是 #154 的核心，也是超越 Swift 項（export 兩邊都有，import 兩邊都沒有）。

## Motivation

xml_import.rs 目前只有型別與驗證，任何實際匯入都回 NotImplemented。XMEML 是 Fronda 自己的匯出格式，可用 round-trip（export→parse）當黃金驗證，風險最低，價值最高。FCPXML/Premiere/Resolve 維持誠實 NotImplemented（各自 namespace/CDATA 複雜，另案）。

## Proposed Solution

render_core/src/xml_import.rs 新增：(1) 極簡內部 tag scanner（`<tag attr>`/`<tag>text</tag>`/`<tag/>`/巢狀、XML entity 反轉義——xml_escape 的逆），不加 XML crate 依賴（僅解析我們自己格式良好的輸出 + 平直 FCP7 匯出）；(2) `parse_xmeml(content) -> Result<ImportedTimeline, XmlImportError>`——sequence name、fps（timebase）、video/audio 軌（反轉 export 的 .rev()）、clipitem（name/start/in/out/speed/link/file pathurl）→ Timeline + referenced files 清單（供 host relink-by-filename）。round-trip 測試：已知 timeline export → parse → 斷言 track 結構/clip 計時/fps/連結。

## Non-Goals

- FCPXML/Premiere/Resolve 解析（維持 NotImplemented）
- 媒體實體 relink（回傳 file 清單供 host 依檔名解析；純核心不碰檔案系統）
- 關鍵影格/濾鏡/文字覆蓋還原（export 的 XML-012/013 已知不完整；先做計時+結構+檔案參考）

## Impact

- Affected code:
  - Modified: crates/render_core/src/xml_import.rs
  - New: (none)
  - Removed: (none)

## 1. Scanner

- [x] 1.1 內部 tag scanner（純函式）：找下一個元素、取元素文字內容、取屬性、取直接子元素列；XML entity 反轉義（&lt;&gt;&amp;&quot;&apos;）；容忍空白/換行/自閉標籤；單元測試涵蓋巢狀、屬性、entity、自閉

## 2. XMEML parser

- [x] 2.1 parse_xmeml：sequence name、rate/timebase→fps；video/audio media 分區逐 track（反轉 export 的 video .rev()）；enabled→hidden、locked；clipitem→Clip（start_frame/trim_start/trim_end/duration、speed value、link linkclipref→link_group_id、media_type 由分區定）；file id/name/pathurl 收集成 ReferencedFile 清單；ImportedTimeline{timeline, files}
- [x] 2.2 dispatch：validate 後依 format 呼叫；Xmeml→parse_xmeml，其餘→NotImplemented（誠實）

## 3. 驗證

- [x] 3.1 round-trip 測試：建構含多軌/trim/speed/link 的 Timeline → XmlExport::export → parse_xmeml → 斷言 track 數與型別、每 clip 的 start/trim/duration/speed/link、fps、file pathurl；邊界（空 sequence、單 clip、自閉 file 參考的 dedup 情形）
- [x] 3.2 cargo test -p render_core 與 cargo test --workspace exit code 全綠

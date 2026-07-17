## Decisions

死欄位：ClipPropertyUpdate 移除 content/font_name/font_size/font_weight/color/alignment/background/border（全 workspace 僅 tool_exec 兩處建構皆 None——先 grep 再刪，呼叫端同步）。#284-leftover：讀 audit journal 原文（97 spec 的決策清單行）與 d76a5ebb/17a2733b diff，查 Rust generation_view 的 aspect picker 是否還有可移植 delta；有就做（S），沒有就在 AGENTS.md 記「查明無事」。警告：native_menu unused import；其他警告逐一判斷，非機械性的不動。

## Implementation Contract

- 零行為變更（純刪死碼/警告）；全部相關 crate 測試綠。

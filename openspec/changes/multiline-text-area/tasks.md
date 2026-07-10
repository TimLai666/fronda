## 1. 整合 worktree 實作

- [x] 1.1 將 worktree branch worktree-agent-aab4aaef459a8c03e 的 commit f2f9732 合併進 main：新檔 crates/app_shell_gpui/src/text_area.rs 與 lib.rs 註冊直接採用；generation_view.rs 與 feedback_view.rs 與主線的 !input 快捷鍵重構（perform_menu_action、key_context("input")、focused guard）有衝突，逐塊手動解——以 TextArea 遷移後的結構為準，容器層暫時性 key_char 防護不再需要的部分移除
- [x] 1.2 在 crates/app_shell_gpui/src/main.rs 的 boot closure（bind_text_field_keys 呼叫旁）加上 text_area::bind_text_area_keys(cx)
- [x] 1.3 TextArea 對齊全域快捷鍵約定：key context 由 "FrondaTextArea" 改為含 input 標記（"FrondaTextArea input"），並刪除其 raw key_down swallow handler（與 TextField 相同理由——!input predicate 已讓全域綁定避讓，swallow 是 macOS 打字疑慮來源）；escape/tab bubble 行為保留

## 2. 收斂與清理

- [x] 2.1 確認 crates/app_shell_gpui/src/text_input.rs 的 apply_editing_keystroke 於整合後已無任何呼叫者；若是，刪除該模組（含其單元測試）與 lib.rs 的註冊
- [x] 2.2 generation_view 與 feedback_view：遷移後其根 div 的 key_context("input") 僅在仍存在 key_char 輸入路徑時保留，否則移除；同時檢查 focused guard 是否仍有保護對象（feedback 的 Tab 欄位切換 handler 保留）
- [x] 2.3 檢查 generation「送出後清空 prompt」與 feedback「送出後清空 message」的既有流程：若有程式化清空 model 的路徑，補上對應的 TextArea set_text 同步（鏡射是單向的）

## 3. 驗證與記錄

- [x] 3.1 三個 gate 以 exit code 驗證全綠：cargo test --workspace、cargo test -p fronda-app-shell-gpui --features desktop-app、cargo check -p fronda-app-shell-gpui --features desktop-app --bin fronda 皆 EXIT=0
- [x] 3.2 對整合後的 diff 做一輪對抗審查（重點：合併衝突解法、TextArea 幾何數學、事件鏡射一致性、max_lines 溢出繪製的可見影響），確認發現並修復
- [x] 3.3 已知限制記錄於 AGENTS.md 或 change 附註：互動行為（IME、軟換行游標、滑鼠選取）僅由編譯與純測試驗證，無 gpui 互動測試；超過 max_lines 時內容繪製溢出（捲動為後續工作）；feedback message 高度由固定 160px 改為內容驅動 5–10 行

## 1. 實作

- [x] 1.1 依 design「純狀態機」實作 ExportQueue pure 模組（transplant 上游 ExportQueueTests：FIFO、目的地保留/釋放、兩型取消、非法轉移 Err），實作 spec「Cancellable FIFO export queue with staged output」狀態機半邊；先紅後綠。驗證：新模組測試全綠。
- [x] 1.2 依 design「staged output 與取消旗標」接 video/audio export（partial 檔+原子 rename、AtomicBool 取消檢查點、取消清暫存），temp-dir e2e 測試取消不留半成品、成功照舊 re-decode 驗證；export_view 掛佇列 UI（列表/進度/取消，AppTheme）。驗證：`cargo test -p fronda-app-shell-gpui` 全綠、desktop check 通過。
- [x] 1.3 `cargo test --workspace` 全綠；AGENTS.md porting table 增列 #298。驗證：內容審查。

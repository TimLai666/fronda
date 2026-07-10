## 1. 已驗證 bug（TDD）

- [x] 1.1 [P] #124 stranded audio：重現測試（linked V+A pair 被 overwrite-place 蓋掉 → audio 軌殘片 + 新 clip audio 排錯軌）→ 修 overwrite 路徑同步清 partner 軌範圍（對照 git show upstream PR #124 diff 的 Swift 修法語意）
  - BUG-FIXED。RED 證據：`place_clips` 蓋掉 linked V+A 後 audio 留 `[(0,90)]` 整片；executor 層 add_clips 蓋掉後出現第 3 條 spurious audio 軌。
  - 修法：`timeline_core::edit::place_clips` 在清目標軌前收集重疊 linked clips 的 partner 軌（僅當目標軌為 Video，鏡射 Swift `4b776e1` 的 `.type == .video` gate），再對每條 partner 軌 `clear_region` 同範圍。Swift fix 語意來源：`git show upstream/pr-124`（fetch 自 `refs/pull/124/head`）。
  - 測試：`spec_clip_mutations.rs::clp_124_place_clips_clears_linked_partner_range`、`clp_124_place_clips_on_audio_track_leaves_video_partner_intact`；executor 層 `tool_exec.rs::add_clips_overwrite_linked_pair_clears_stranded_audio_no_extra_track`（鏡射 Swift `addClipsClearsLinkedAudioWhenOverwritingLinkedPair`）。既有 `add_clips_linked_audio_does_not_clobber_existing_audio`（未連結音樂不被清）持續綠。
- [x] 1.2 [P] #263 ripple fixpoint：重現測試（anchor 軌 ripple 清 sync-locked 軌，被清 clips 的 partner 在 lock-off 軌 → 殘留 desync）→ compute_ripple_delete 的 partner 傳播改 fixpoint 掃全部 cleared 軌；既有 ripple 測試（含 200k fuzz 若在）不得回歸
  - BUG-FIXED。RED 證據：兩情境 cleared 皆為 `[0,1]`、partner 軌 2 殘留（sync-locked 軌上 linked clip 的 lock-off partner；以及 anchor→軌1(group A)→軌2(group B) 的鏈式傳播）。
  - 修法：`compute_ripple_delete` 先收 anchor + 全部非 ignore 的 sync-locked followers（#227 語意不變），再以 worklist 對「所有 cleared 軌上與 merged ranges 重疊的 linked clips」做 partner 傳播直到 fixpoint。#207 ignore 集合僅豁免 sync-lock 清除，link 傳播不受豁免（與既有 anchor-scan 行為一致）。
  - 測試：`spec_workflow.rs::rpl_263_partner_on_lockoff_track_of_cleared_sync_locked_track_clears_too`、`rpl_263_partner_propagation_chains_to_fixpoint`、`rpl_263_partner_outside_ranges_does_not_propagate`（負向界限）。既有 rpl_005/006/227/#207 測試與 tool_exec ripple 回歸全綠。

## 2. UNVERIFIED 查證（測試先行，有 bug 才修）

- [x] 2.1 [P] #139：render_core format_timecode 測 frame 1800 @ 29.97 drop-frame 應 00;01;00;02（與 10 分鐘邊界 17982 除數）；錯則修
  - VERIFIED-OK。Rust `format_timecode` 已用 drop-adjusted 除數（fpm=1798、fp10m=17982），frame 1800 → `00;01;00;02` 的測試本已存在且通過。補釘 10 分鐘邊界斷言：`17982 → 00;10;00;00`、`17982+1800 → 00;11;00;02`（第 10 分鐘為非 drop 分鐘）、`1799 → 00;00;59;29`。無需修碼。
- [x] 2.2 [P] #264：mutation/tool args 以接近 i64::MAX 的 frame 值打 add/insert/move/split → 溢位 panic 或 wrap 則加共用 clamp（上游用 1e9 ceiling）
  - BUG-FIXED。RED 證據（debug build 真實 panic）：insert_clips/move_clips/apply_layout/add_clips/add_texts 以 `i64::MAX` 觸發 `attempt to add with overflow`（edit.rs:238/341 等）；set_clip_properties 則默默接受 `durationFrames=i64::MAX`（毒化狀態，後續 end_frame() 才炸）。註：mutation.rs 驗證器不在 executor 的即時路徑上（僅 library surface），故 ceiling 同時佈到兩層。
  - 修法：`mutation.rs` 新增共用 `MAX_TOOL_FRAME = 1_000_000_000` + `require_frame_in_bounds`（錯誤訊息鏡射上游 PR #265 `maxToolFrame`），佈線至（a）mutation 驗證器 split/insert/move/set_clip_properties/add_texts；（b）executor 即時路徑 `resolve_placement`（trim/duration，覆蓋 add_clips+insert_clips）、insert_clips frame（含補上游的 `frame >= 0` guard）、move_clips toFrame、split_clips atFrame/frames、set_clip_properties duration/trims、apply_layout startFrame/durationFrames、add_texts startFrame/durationFrames。ripple_delete_ranges 維持 clamp-tolerant（上游同樣刻意不動）。
  - 測試：executor 層 7 支 `*_rejects_*overflow*`/`*_beyond_ceiling` + mutation 層 3 支 ceiling 測試。
- [x] 2.3 [P] #212：set_clip_properties speed 0.1 的接受度；下限過緊則放寬並測慢速 duration 換算
  - VERIFIED-OK（下限不緊）+ 一項對齊修正。speed 0.1 全路徑合法：mutation 驗證器僅拒 `<= 0`，`timeline_core::set_clip_properties` 以 source coverage 換算（150f @1.0 → 1500f @0.1，測試 `set_clip_properties_speed_0_1_slows_clip_10x` 釘住）。發現並修正：executor 即時路徑原本「默默接受」`speed <= 0` 並寫入 `clip.speed=0`（上游會 throw `speed must be > 0`）— 已加 guard + 測試 `set_clip_properties_rejects_non_positive_speed`。

## 3. S 級 port

- [x] 3.1 [P] #36：AnthropicConfig 支援自訂 base URL（ANTHROPIC_BASE_URL env 覆蓋，預設不變）；transport 測試
  - PORTED。`AnthropicConfig::from_env(api_key)` 讀 `ANTHROPIC_BASE_URL`（trim、空白 fallback 預設），`chat_view::spawn_agent_turn` 改用 from_env。URL 組裝以純函式 `resolve_base_url` 測試（避免測試間 env 競態）：unset/blank → `https://api.anthropic.com`；override 會 trim 且 `/v1/messages` 正確拼接。
- [x] 3.2 [P] #268：request builder 對 sonnet5 系模型加 output_config effort low（對照 git show upstream/main 對應 Swift 請求組裝檔的確切 JSON 形狀）；snapshot 測試
  - PORTED。上游形狀確認自 `upstream/main:Sources/PalmierPro/Agent/Clients/AgentClientTypes.swift:20`：`case .sonnet5: ["output_config": ["effort": "low"]]`。Rust 實作 `prompt_caching::model_request_extras`（`claude-sonnet-5` 前綴族）+ `apply_model_request_extras`，同時佈進 `build_agent_request` 與即時路徑 `agent_loop::run_agent_turn`（後者原本不經 build_agent_request，只加前者會是死 code）。測試：prompt_caching snapshot（sonnet5 + dated variant 有、opus/haiku 無）+ agent_loop ScriptedTransport 驗證實際 request body。

## 4. 收尾

- [x] 4.1 三 gate exit code 全綠；97-audit 附錄各項標注結論
  - Gate 結果見 change 收尾 commit message 與回報（cargo test --workspace；cargo test -p fronda-app-shell-gpui --features desktop-app；cargo check -p fronda-app-shell-gpui --features desktop-app --bin fronda，皆以 exit code 驗證）。97-audit 附錄已標注：#124/#263-ripple/#264(#265)/#36/#268 PORTED、#139/#212 VERIFIED-OK。

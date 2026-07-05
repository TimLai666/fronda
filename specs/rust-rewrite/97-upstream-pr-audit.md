# Upstream PR Audit — Rust Rewrite Applicability

Latest re-audit: 2026-07-03
Upstream HEAD: `9a3ae50` (palmier-io/palmier-pro main, v0.5.2)
Previous audit: 2026-06-25 at `b9b4ad9`

This file catalogs every upstream PR (from the Swift `palmier-pro` repo), its
current porting status in Fronda, and any action items.

The **2026-07-03 Re-audit** section below covers the 86 new commits in
`b9b4ad9..9a3ae50` (PRs roughly #148–#254). The historical tables further down
remain valid for pre-`b9b4ad9` PRs.

## Legend

- **DONE** — Ported to Rust and verified with tests.
- **NOT STARTED** — Applicable to Rust, not yet ported.
- **N/A (Swift-specific)** — Relies on AVFoundation, Metal, AppKit, or other
  Apple-only APIs. No Rust equivalent needed.
- **N/A (Reverted)** — Was merged then reverted upstream; not applicable.
- **DEFERRED** — Applicable but blocked on larger infra or spec work first.

---

# 2026-07-03 Re-audit (b9b4ad9 → 9a3ae50, 86 commits)

Method: each commit's real diff was read with `git show` and compared against
current Rust source. Rust-relevance judged per subsystem (not from title alone).

Key cross-cutting judgment: **Swift `Int(Double)` traps on overflow; Rust `as`
casts saturate and `serde_json::Number::as_i64()` returns `None` out-of-range.**
So Swift crash-hardening PRs (e.g. #201) are SWIFT-ONLY for the crash itself;
only their *validation semantics* (reject vs clamp) are portable, and low value.

## Tier 0 — Already satisfied in Rust (verify-only, no port)

| PR | Item | Evidence |
|----|------|----------|
| #163 | trimEndFrame stored as tail amount, not out-point | `timeline_core/src/lib.rs:89-95` `source_frames_consumed`/`source_duration_frames` bake the invariant `consumed + trim_start + trim_end == source`. |
| #203 | Per-clip blend modes (data model) | `core_model/src/timeline.rs` `BlendMode` enum (16 modes) + `Clip.blend_mode`. Rendering application still TODO (Tier 3). |
| #151 | Timeline zoom buttons clickable | `app_shell_gpui/src/toolbar_view.rs:242-296`. Only mismatch: step factor 1.5 vs Swift 1.25 — trivial parity tweak. |
| #232 | Deterministic transcripts (filtering half) | `search_core` `Transcript::filter_range` (TRN-005/006). Full-file cache identity is a platform-layer concern. |
| #189 | Caption phrase timing from per-word timestamps | `search_core/src/caption.rs:279` `phrases_from_words` already times each phrase by its first word's `start_seconds` and last word's `end_seconds`, and builds phrase text from the words themselves — so the Swift char-distribution→per-word-timestamp change and the phrase/word alignment concern are structurally absent. Verified 2026-07-03. Minor divergence: Rust drops words with invalid (negative) timestamps rather than char-distributing them. |

## Tier 0 — Not applicable to Rust (SWIFT-ONLY)

| PR | Reason |
|----|--------|
| #201 | `Int(Double)` overflow trap. Rust saturates / returns `None`; no crash path. Validation-reject semantics optional, low value. |
| #253 | AVFoundation audio-extraction concurrency gate. No Rust audio extraction pipeline. |
| #184 | Swift main-actor offloading. Rust backend is already async; gpui shell does not hang. |
| #148 | Off-main image decode is Swift actor isolation. Minor: `id_short` micro-opt could still be applied if it is O(n²). |
| #66c1e10 | Restore manifest before window mount — Rust load order already correct; verify only. |
| #217 | "generating" (not offline) preview overlay — pure UI, unblocks after #216 data model. |

## Tier 1 — High-value pure-logic ports (testable, low risk) — DO FIRST

| PR | Port | Target | Effort | Status |
|----|------|--------|--------|--------|
| #236 | Symmetric trim `resolvePlacement` for add_clips/insert_clips | `agent_contract/tool_exec.rs` cmd_add_clips/cmd_insert_clips | M | **DONE 2026-07-03** — `resolve_placement` helper; type/source-length from manifest; symmetric trim + mutual-exclusivity; also fixed insert_clips `asset_id` (was a random UUID). 4 tests. |
| #233 | add_clips keeps project fps fixed; warn on source-fps divergence | same code path as #236 | S | **DONE 2026-07-03** — source seconds scaled by project fps; divergent source fps warns and points at set_project_settings; project fps never changed. Test covers it. |
| #224 | Open project with corrupt media.json (degrade to empty, not fatal) | `project_io/src/lib.rs` | S | **DONE 2026-07-03** — `read_optional_json_defaulting_on_decode_error`; corrupt manifest → empty, original file preserved. Test flipped. |
| #218 | Aspect-ratio distortion: refit auto-fitted clips on canvas change | `timeline_core::refit_transforms` (exists) + trigger | S-M | **Core existed + now wired**: `refit_transforms` resets auto-fit clips on resolution change and is invoked by `set_project_settings` (#177). Remaining refinement: Swift's aspect-preserving proportional scale + active-scale-keyframe scaling (Rust currently resets auto-fit to full-canvas default). |
| #207 | ripple_delete_ranges: per-call sync-lock exemption (`ignoreSyncLockedTracks`) | `timeline_core/src/workflow.rs` RippleDeleteConfig | M | **DONE 2026-07-04** — `RippleDeleteConfig.ignore_sync_lock_track_indices` (BTreeSet); `compute_ripple_delete` skips ignored tracks when collecting sync-locked followers into the clear-set; executor shift loop changed to `if !cleared.contains(&ti) { continue }` (shift only cleared tracks — post-#227 every non-ignored sync-locked track is already cleared, so ignored ones stay in place). Tool arg `ignoreSyncLockTrackIndices` (list of indices, matching the index-based `trackIndex` param, not Swift's id list). NO refuse (left-in-place can't collide). 2 executor tests (ignored → untouched; not-ignored → cut+rippled). Also fixed the schema `ranges` doc that lied `{startFrame,endFrame}` — the executor reads `{start,end}`. |
| #227 | Master audio: sync-locked follower tracks are CUT (cleared), not just shifted | `timeline_core/src/workflow.rs` compute_ripple_delete | M | **DONE 2026-07-04** — `compute_ripple_delete` adds every sync-locked follower to `cleared_track_indices`, so the executor clears the range on them + shifts. Cut+ripple always absorbs its own gap, so the old shift refuse was dead code (verified: the Swift refuse loop is provably vacuous for empty ignore-set; 200k-timeline fuzz, 0 broken). 2 tests. |
| #243 | Default agent model → Sonnet 5 | `app_contract/settings_storage.rs` AGENT_DEFAULT_MODEL, `chat_view.rs` model list | XS | **DONE 2026-07-03** — `sonnet46`→`sonnet5`; chat list `claude-sonnet-4-6`→`claude-sonnet-5`. Backend accepts both during rollout. Tests updated. |

## Tier 2 — New agent tools / schema (medium) — parity features

| PR | Tool / change | Target | Effort |
|----|---------------|--------|--------|
| #177 | `set_project_settings` tool + auto-match timeline on empty add | agent_contract | H | **DONE 2026-07-03** (tool + presets + fps rescale + refit + undo; 6 tests). The rescale/refit core (`apply_fps`/`refit_transforms`/`apply_settings`) already existed in `timeline_core/src/project_settings.rs`; only the tool layer was missing. **Auto-match-on-add (`applySettingsIfNeededForAgent`) still deferred.** |
| #186 | `split_clip` → `split_clips` batch (two modes, dedup, A/V regroup) | agent_contract | M | **DONE 2026-07-03** — renamed tool; explicit `splits` + `trackIndex`/`frames` modes; validate-all-then-apply (no partial state); dedup; re-resolves each cut against current sub-clips; reuses `timeline_core::split_clip` (already does linked A/V regroup). 3 tests. Also added `object_optional` schema helper (the shared `object()` marked every prop required — wrong for exactly-one-of tools; now used by split_clips + set_project_settings for correct MCP schemas). |
| #178 | `language` (BCP-47) param threaded into get_transcript/inspect_media | agent_contract read path | M (schemas already list it) |
| #160 #245 | `remove_words` tool (word→frame, ripple linked partners) + `matches` filler tokens | timeline_core, agent_contract | M | **PURE CORE DONE 2026-07-04** — `timeline_core::word_cut`: `WordCutPlanner` (`cut_ranges`: cut each run of selected words + up to half the adjacent gap, merge), `span_frames` (source-sec→project-frame, speed/trim/visible-window clamp), `CutAggressiveness` (tight 60 / balanced 150 / loose 320 ms), `ms_to_frames`, `TimelineWord`, `plan_word_removal` (group-by-clip → ranges-by-track → **single primary track**: refuse unlinked multi-track, else min index of one link group; only the primary track's ranges are cut, the ripple carries linked partners). 16 unit tests mirror Swift `WordCutPlannerTests` exactly. `agent_contract::cmd_remove_words` + tool schema + `parse_word_spans`/`parse_word_matches`/`normalized_word_match` (index-clamp per #245 so an out-of-range span can't iterate wildly; `matches` case/punct-insensitive tokens; words XOR matches). Ripple-apply refactored into shared `apply_ripple_delete_on_track` (reused by ripple_delete_ranges). 10 executor tests incl. e2e cut+report. **Host-deferred**: the transcriber (`timelineWords`) — Rust has no SpeechTranscriber, so `set_timeline_words` is the seam (empty → "No transcribable speech", same boundary as get_transcript/#178). #245 `matches` ported; #245's cloud-provider + language-reuse (c12999a) are host-transcription concerns. |
| #152 | `send_feedback` tool (session dedup + 8/session cap + diagnostics) | agent_contract | L |
| #245 | `TranscribedWord.speaker` field + `remove_words` `matches` filler-token arg | search_core, agent_contract | L-M (cloud provider itself is SWIFT-ONLY) |
| #249 | `paid_only` on model catalog + free-tier gating in list_models | generation_core, agent_contract | M | **BLOCKED (host/data) 2026-07-04.** Gating logic is pure (`model_available = is_paid \|\| !paid_only`, `require_plan`), but Rust `cmd_list_models` returns HARDCODED placeholder models (gen-3/kling/sd3/…) — the real catalog (VideoModelConfig.allModels etc.) is never loaded into the agent. Porting `paid_only` now would gate fake data. Do #249 only after the real model catalog is wired into the agent (list_models/generate read live `ModelConfig`s). The `paid_only` FIELD + gating helpers can be added to `generation_core` any time; the TOOL-level gating waits on catalog wiring + a host `is_paid` seam. |
| #219 | `import_status` field on manifest entry (preparing/downloading/failed) | core_model, agent_contract | M | **Mostly host + speculative 2026-07-04.** Actual Swift field is `importInput: MediaImportInput? {sourceURL, sourcePath, createdAt}` (resume source), and import LIFECYCLE reuses the already-ported `generation_status` (restore skips `.failed`). Bulk of #219 (`ToolExecutor+Import` async placeholders, background download, 128 lines) is host I/O. The portable core is just an `import_input` persistence struct — low value without the async-import host logic (marginal round-trip safety). Defer unless async import is wired host-side. |
| #242 | `create_matte` tool + MatteAspect presets | core_model, agent_contract, app_shell | M | **DONE 2026-07-05 (end-to-end).** `core_model::matte`: `MatteAspect` (7 presets Project/16:9/9:16/1:1/4:3/9:14/2.4:1) + `even`/`fit`/`pixel_size`/`parse` (pure, 6 tests, mirrors Swift `Matte.even`/`fit`). `agent_contract::cmd_create_matte` + tool schema (58→59 tools): parses hex (`TextRgba::from_hex`) + aspect + name/folder, computes even pixel dims from the aspect + timeline size, hands the colour+size to a host `MatteWriter` seam, and registers the resulting `ClipType::Image` asset (matte = an image, NO `ClipType::Matte`). `app_shell::matte_writer::ProjectMatteWriter` implements the seam: renders a solid-colour PNG (`image` crate) and writes it into the open project's `media/` dir, returning a project-relative `MediaSource`; wired on project open/save-as in `editor_state_hub`. The pure executor stays FS-free (the MCP/headless path leaves the writer unset → "unavailable"). 4 executor tests (mock writer) + 2 writer tests (valid PNG on disk) + 6 sizing tests. |

## Tier 3 — Large new subsystems — need own sub-spec each

| PR(s) | Subsystem | Target | Effort |
|-------|-----------|--------|--------|
| #193 | **FCPXML export v1 baseline** | `render_core/src/fcpxml_export.rs` | XL | **v1 DONE 2026-07-03** — valid FCPXML 1.10: `<resources>` (format + deduped assets w/ media-rep src), `<library>/<event>/<project>/<sequence>/<spine>` with a full-length `<gap>` anchoring every clip as a connected `<asset-clip>` at absolute offset, lane per track (video +, audio −), rational project time. 6 tests. **Wired into the export UI + execution**: `ExportMode::Fcpxml` ("Final Cut Pro (.fcpxml)"), pure `interchange_content()` generator (also covers XMEML), and `write_interchange()`. The Export button now opens a save dialog (`prompt_for_new_path`) and writes the `.xml`/`.fcpxml` file from the live editor timeline+manifest — so both interchange formats actually produce files, and the panel shows a success ("Exported to …") or failure message (`set_interchange_result`). (Video render execution + `.palmier` bundle write are separate, still TODO.) |
| #214 | FCPXML format naming + Rec.709 colorspace | `render_core/src/fcpxml_export.rs` | L | **DONE 2026-07-03** — `format_rate_suffix`/`recognized_video_format_name`/`sequence_format_name`/`frame_duration_str` (NTSC-aware); sequence `<format>` now named (`FFVideoFormat1080p30`, else `FFVideoFormatRateUndefined`) with `colorSpace="1-1-1 (Rec. 709)"`. 3 helper tests + updated exporter tests. Per-asset formats still shared with the sequence (full per-asset formats come with #206). |
| #206 | FCPXML per-asset formats (partial) | `render_core/src/fcpxml_export.rs` | M | **Per-asset formats DONE 2026-07-03** — each visual asset now emits its own `<format>` from its source width/height/fps (`video_format_name`, `frame_duration_str`); audio assets carry no format; visual assets without source dims fall back to `r1`. Assets already dedup by media_ref (same-source A/V collapses to one asset w/ hasVideo+hasAudio). **DONE**: also fixed the frameDuration grid (project fps, not source fps, so asset duration + asset-clip in-point align) and per-clip `format` reference (own format for video, omitted for audio); and the synced-A/V-pair collapse — `redundant_audio_clip_ids` drops the audio partner of a 1-video/1-audio link group sharing source/timing/trim/speed AND enabled state. The `enabled` guard IS ported (derived from the TRACK: video `!hidden`, audio `!muted`) — a muted audio partner is NOT collapsed, or its silenced audio would become audible via the video asset-clip. Multiple tests. Only Swift's volume-from-partner emission is unported (the Rust exporter emits no per-clip volume). |
| #197 #247 #254 | FCPXML refinements on the v1 baseline | `render_core/src/fcpxml_export.rs` | L each | retime `<ref-clip>` compound wrapping (#197), source timecode (#247), per-target (Resolve/FCP) transform/crop/blend calibration + text stroke (#254). **#247 source timecode DONE 2026-07-04** — `start_timecode_frames(entry, fps) = round(source_timecode_frame/quanta*fps)` (0 when absent); each `<asset start>` now emits its embedded timecode (was `0s`), and each `<asset-clip>` in-point adds that origin (`origin + trim_start`), so Resolve doesn't flag a timecode mismatch. Uses the #136 manifest timecode fields. 3 tests (no-tc → `0s`; tc offsets asset+clip; quanta≠fps rescale). **#197 retiming DONE 2026-07-05** — a clip with `speed != 1` now emits a `<timeMap frameSampling="floor">` (two `<timept>`s mapping the output span to `[origin, origin+mediaFrames)` of the source) and its in-point `start` is on the retimed axis (`rational_time(trimStart*q, fps*p)`); keyframe `<param>` times also move to the retimed output axis (`keyframe_time_str` = `(trimStart*q + frame*p)/(fps*p)`). Current Swift emits `<timeMap>` on the asset-clip (not a `<ref-clip>` wrapper), which this mirrors: `rational_speed` (best p/q ≤1000), `rational_time`, `build_time_map`. 5 tests (rational helpers, timeMap span, 1× → no timeMap, retimed start `1/4s`, retimed keyframe time on output axis). **Relink-by-filename DONE 2026-07-04** — the asset + asset-clip `name` attributes now emit the on-disk filename (source path's last component, extension preserved) instead of the display label, so Resolve relinks by name (`file_name` helper; 1 test). **#247 fully ported.** **#254 scalar + static geometry DONE 2026-07-04** — the asset-clip carries, in Swift's child order: `<adjust-crop mode="trim">` (Resolve trim-rect: `top/bottom = crop*100/fit`, `left/right = crop*sw*100/seqH`), `<adjust-conform type="fit">`, `<adjust-transform>` (scale `= size/fitFraction`, sign-flipped per mirror axis; rotation `= -model rotation`; anchor `0 0`; position in FCP points `= (centre-0.5)*seqDim/(seqH/100)/fit`, y-up-negated), `<adjust-blend amount>` (opacity), `<adjust-volume amount>` (dB `20*log10`). `fit_fractions`/`scale_value`/`position_value`/`clip_adjustments`/`format_number` mirror Swift's Resolve target. Emitted only when non-default (default clip → self-closing, backward-compat verified); conform accompanies any geometry so the fit-relative scale/position stay consistent. 8 geometry/scalar tests with hand-computed values (FCPXML transform/crop math independently adversarially audited — no divergence). **Keyframed opacity + transform DONE** — `<adjust-blend>` and `<adjust-transform>` carry `<param>/<keyframeAnimation>` children (clip-relative keyframe time, `curve="linear"` for linear segments). Transform position/scale/rotation values are sampled through `timeline_core::resolved_transform_at` per keyframe, so the top-left→centre + size coupling is handled correctly (hand-computed test: top-left ramp → centre positions). Shared `write_kf_param` helper. **`enabled` DONE** — a hidden video / muted audio track's clips export `enabled="0"`. **Text `<title>` DONE** — text overlays (previously skipped entirely) now emit a `<title>` referencing a once-declared `titleBasic` effect resource, with a `<text>`/`<text-style-def>` pair (font family from the name, face from weight, size, `fontColor` r g b a, alignment), a fit-conform + position transform, and static opacity (`write_title`/`font_family_fallback`/`color_string`). **Title border stroke DONE** — `strokeColor`/`strokeWidth` (`0.04·fontSize`) when the border is enabled. The keyframe + title output was independently adversarially audited (2026-07-05): keyframe time axis is clip-relative and matches Swift's `keyframeFrames`(absolute)+`keyframeTime`(−startFrame) exactly (regression test at a non-zero start frame confirms it stays clip-relative while the clip `offset` is absolute), the position top-left→centre coupling is faithful, and the title node matches byte-for-byte incl. the effect `uid`. **Collapsed-pair linked-audio volume DONE** — when a synced A/V pair collapses into the video asset-clip, the `<adjust-volume>` uses the dropped audio partner's gain (Swift's `linkedAudio ?? clip`), not the video clip's own volume (`redundant_audio_clip_ids` now also returns a video-id→audio-volume map). **Keyframed title opacity DONE** — titles share the `append_opacity_blend` helper, so animated text opacity round-trips too. **FCP-target ENCODING DONE 2026-07-05** — `FcpxmlTarget` enum (Resolve default, mirroring Swift `FCPXMLTarget.default`) + `export_with_target`; for `Fcp` the transform `fit` is `(1,1)` (raw frame-relative scale/position, no aspect-fit compensation) and the crop trim-rect uses plain 0..100 percentages instead of the source-pixel/seq math. `export()` still defaults to Resolve, so existing behaviour is unchanged. 1 test compares Resolve vs FCP scale + crop on hand-computed values (square + 4K sources). Remaining host piece: the export-dialog dropdown to let the user PICK Final Cut Pro (trivial gpui follow-up; the encoding is complete + tested). **Auto-fit-every-visual-clip DONE 2026-07-05** — `<adjust-conform type="fit">` is now emitted for EVERY visual asset-clip (matching Swift), so a source whose resolution/aspect differs from the timeline is fit into the frame rather than shown at native size; it's a no-op for matching sources. (Closes the last geometry divergence the transform audit flagged.) **FCP-target selector DONE 2026-07-05** — `ExportViewModel.fcpxml_target` (+ setter) threads through `interchange_content`/`write_interchange`, and the export dialog shows a "Calibrate for: DaVinci Resolve / Final Cut Pro" toggle. **#254 is now FULLY ported** (geometry for both targets + user selection). Formerly-pending, now resolved. **Historical note**: the title `fontFace` now reflects both bold AND italic (`Bold`/`Italic`/`Bold Italic`/`Regular`) since the **#65 `TextStyle` on-disk-format compat bug was fixed 2026-07-05** — Swift-authored `isBold`/`isItalic` now round-trip into Rust's `TextStyle`. |
| #183 | Export write-failure surfacing (`export` returns `String`, not `Result`) | render_core, export_model | S (prereq for FCPXML robustness) |
| #226 c9222fe #1a5aa2c | **apply_layout** — VideoLayout enum (10 layouts), LayoutSlot, transform/crop math, batch clips per slot | core_model, agent_contract | XL | **Geometry + placement math + re-layout tool DONE 2026-07-03** — `core_model/src/video_layout.rs`: `VideoLayout` (10 layouts, exact Swift slot rects, pip inset 0.28 / margin 0.035, z-order), `media_canvas_aspect`, `crop_fitting_aspect`, `layout_placement` (fill/fit → Transform+Crop w/ anchors). Plus the `apply_layout` **agent tool** (`cmd_apply_layout` + `resolve_layout_anchor` + `apply_layout_place_new`): **FULLY PORTED**. Re-layout mode — each slot takes a `clipIds` array (batch takes into one region; singular `clipId` still accepted), same-track cross-slot time-overlap check + multi-slot coincidence check, all validated before mutation. Place-new mode — each slot takes a `mediaRef` plus top-level `startFrame`/`durationFrames`; creates a stacked video track per slot by z-order (insert-at-0 ascending z → highest z on top), places a new clip per slot with the layout transform/crop baked in, and auto-detects project settings from the first video (via the #177 auto-match seam). 25 tests (7bcfafb). |
| #225 | **Text animation** — TextAnimation/WordTiming/preset enums + agent args (data model portable; renderer is UI) | core_model, agent_contract | L | **DATA MODEL + AGENT ARGS DONE 2026-07-04** — `core_model::text_animation` (WordTiming, TextAnimation, 11-preset enum w/ Swift rawValues + render-mode/agent-value helpers); `Clip.text_animation`/`word_timings` (serde round-trip, no data loss); `timeline_core::rescale_word_timings` on duration change (Swift `setDuration`); `add_texts` accepts `animation`/`highlightColor` (`parse_text_animation`). Only the renderer (TextAnimator/TextFrameRenderer) is UI-deferred. |
| #216 | **Generation recovery** — backend_job_id/result_urls persistence, resume in-flight jobs, generation_status enum | core_model, generation_core | XL | **DATA MODEL (persistence) DONE 2026-07-04** — `GenerationInput.backend_job_id` + `output_index` + `result_urls` (serde `resultURLs` uppercase-acronym key + alias) AND `MediaManifestEntry.generation_status` (serde `generationStatus`, surfaced by `get_media` with a "poll until 'none'" hint) round-trip, so a project saved mid-generation keeps its resumable job + lifecycle state. 3 round-trip/get_media tests. **Still to port**: resume LOGIC only — subscribe to the backend job / restore on launch (app-wired, needs a GenerationBackend adapter). |
| #250 | **MCP stateful sessions** — Mcp-Session-Id routing, per-session Server, Content-Length framing, LRU(32)+1h prune, SSE, tools/list_changed | mcp_server | XL |
| #238 | **MCP project navigation** — get/open/new_project tools + app-layer project registry + `.system` message role | agent_contract, app layer | L-XL | **Half-ported / partly blocked.** `.system` message role DONE (`AgentMessageRole::System`). `create_project`/`open_project`/`delete_project` tool SCHEMAS exist (#172) but had NO executor dispatch → they returned the misleading "Unknown tool"; fixed 2026-07-05 to return an honest limitation message instead. Their FULL behaviour switches the whole app's active project mid-turn (an app-navigation seam like `MatteWriter`, plus `delete_project` is DESTRUCTIVE) — this warrants confirmation before wiring (app-nav + data-loss), so it's deferred. When wired: executor→app project-switch seam that calls `editor_state_hub.load_bundle`/create-new, mirroring Swift `get_projects`/`open_project`/`new_project`. |

## Tier 4 — UI parity (gpui) — visual/interaction match

| PR | UI change | Target | Effort |
|----|-----------|--------|--------|
| #199 #235 b05913b 0cb8848 | **Skills** — SkillStore (~/.palmier/skills SKILL.md), SkillCatalog (GitHub), Settings tab, read_skill tool, prompt index. Largest single gap. | new modules + settings_view + agent | XL | **Store + agent wiring DONE 2026-07-03**: (1) `skill_store.rs` — `parse_frontmatter`, `load_skills`, `load_agent_skills` (with bodies), `prompt_index`. (2) agent_contract — `AgentSkill`, `read_skill` tool (56 tools), `ToolExecutor::set_skills`, `system_instruction_with_skills`/`skill_prompt_index`. (3) boot loads `~/.palmier/skills` into the executor so the in-app agent can discover + `read_skill`. (4) the chat agent-request builder DOES call `system_instruction_with_skills(guard.skills())` and passes it as the turn's system prompt (`chat_view.rs:134-143`) — verified 2026-07-04, so installed skills reach the LLM. 14 tests total. **Still to port**: SkillCatalog (GitHub install/refresh), Settings > Skills pane UI, tour entry points, copy-to-agent. |
| #168 | Project settings as editable dropdowns in inspector (Resolution/FPS/Aspect presets) | inspector_view | H |
| #196 | Update badge redesign — UpdateSidebarCard (home) + UpdateProjectBadge (titlebar) + focus/staleness observers | home_view, titlebar_view | M |
| #204 | Window sizing — Home 1200×880, Settings 1200×900 (min 860×640), Project maximizes to screen | window.rs, settings_view | M | **Sizes DONE 2026-07-03** — `WindowConfig::for_home` 1200×880, `for_settings` 1200×900 min 860×640 (exact Swift #204 values; home size is applied at window open). Project "maximize to visible screen" deferred — Rust is single-window/ActiveScreen so it doesn't map to Swift's per-window model; needs display-bounds resize. |
| #191 | Double-click preview selects clip under cursor (spatial hit-test w/ transform/crop/rotation) | preview_view | H |
| #159 | Chat input focus restore after backspace clears field | chat_view | L |
| #248 | Login-incentive free-credits CTA in chat panel | chat_view | L |

## Non-PR parity (Swift↔Rust gap sweep, 2026-07-03)

Beyond upstream PRs, a multi-agent sweep mapped Swift capabilities missing in
Rust. Landed pure-logic wins (all unit-tested): `format_aspect_ratio`,
`format_duration`, `db_from_linear`/`linear_from_db` (timeline_core::inspector);
`ClipType::content_type_for_extension` (core_model); `SettingsState` model
toggle (app_contract); `format_cost` em-dash fix (generation_core); moment
drag-segment `#start-end` (timeline_core::drag_payload); `EmbeddingStore`
binary `to_bytes`/`from_bytes` (search_visual).

Still-open pure-logic items: pack the 144-line Swift AgentInstructions into
`SYSTEM_INSTRUCTION`; SHA256 transcript cache identity (needs `sha2`).
The XMEML `xml_export` items from the earlier note are now DONE (verified
2026-07-04): keyframed motion+opacity (XML-012), `file://` path norm, `<file>`
dedup (manifest threaded via `export_with_manifest`), and **fade→transitionitem**
— fades now export as single-sided Cross Dissolve (video) / Cross Fade (audio)
`<transitionitem>`s, the form Premiere actually reads, instead of the ignored
`<fadein>/<fadeout>` tags (`write_fade_transition`, 3 tests; mirrors Swift
`XMLExporter.fadeTransition` incl. `cutPointTicks`, alignment, effect IDs).
The sweep also confirmed many suspected gaps are already DONE (keyframe
smoothstep interpolation, folder-drag, `resolved_transform_at`/`crop_at`,
model-config validate/discount).

## Incomplete tool ports (advertised-but-stubbed, found 2026-07-05)

A definitive coverage test (`every_advertised_tool_is_dispatched`, agent_contract)
found **12 of the 59 advertised tools had schemas but no executor dispatch** —
they returned the misleading "Unknown tool". Schemas landed ahead of the logic
across Issues #154/#155/#157/#158/#165/#172/#174. Each returns an honest
limitation message (or is now implemented); the test permanently guards the class.
**6 of the 12 are now implemented (2026-07-05):**

- **#155 compound clips** — end-to-end: `timeline_core::compound`
  (create/dissolve/`flatten_compound_clips`, single-track), agent executor commands
  (via `exec_mut` so undo captures them), and flatten at every render/export
  chokepoint (`compose_frame`, `mix_timeline_audio`, XMEML + FCPXML export) so a
  compound clip renders/exports its nested content. Decode closures key on the
  manifest, so flattened constituents decode with no export-path change. v1 is
  single-track; composing a transform/fades on the compound clip *itself* onto the
  group, and a persistent display name (needs a Clip/Timeline name field —
  `MediaSource` is file-only), are follow-ups.
- **#157 clip presets** — `save/apply/list_clip_presets` via an in-memory store on
  the executor (capture transform/crop/opacity/volume/speed/effects/blend/chroma;
  apply routes speed through `apply_clip_speed`). #157 is Rust-native (no Swift
  equivalent), so this already surpasses Swift. Persisting presets to project.json
  (a data-model decision) is a follow-up.
- **#174 remove_silence** — the pure detector already existed
  (`audio_core::silence_detector`); added `rms_envelope`, a `ClipAudioSource` host
  seam, `cmd_remove_silence` (envelope → detect → source→frame map → ripple
  delete), and the ffmpeg-backed app decoder (`ProjectAudioSource`). Honest
  "unavailable" on the MCP/headless path.

Remaining stubbed tools (3): `create_project`/`open_project`/`delete_project`
(#238 — app-nav, `delete_project` destructive, needs confirmation).
`import_xml` was REMOVED 2026-07-05 — the full upstream ToolDefinitions
comparison showed it never shipped (speculative Issue #154 schema), same as the
audio stubs.

## Tool-surface drift vs upstream v0.6.1 (full name-list diff, 2026-07-05)

Upstream tools we DON'T have (each needs its own port decision):
`update_text`, `export_project` (agent-driven export incl. the #255 timelineId
arg), `send_feedback`, `get_projects` + `new_project` (upstream's real
project-nav pair — our `create_project`/`delete_project`/`duplicate_project`
stub names don't exist upstream; realign when tackling #238; upstream has no
delete tool at all). Also fixed in this pass: our #119 tool shipped as
`sync_audio_clips` but upstream's name is `sync_audio` — renamed + schema
aligned (targetClipId single form, searchWindowSeconds → windowed correlation).
Rust-native extensions kept deliberately (not drift; VERIFIED 2026-07-05 —
none exist in current upstream ToolDefinitions, `git log -S add_shapes` shows
no history either, so these agent-tool NAMES were always Rust-side; the
underlying data-model fields are real ported PR contracts): compound-clip
pair, clip presets, `add_shapes`, `apply_animation`, `set_blend_mode`,
`import_folder`, `duplicate_project`. Upstream exposes three of the
capabilities through consolidated tools instead — chroma keying via
`apply_effect` (its description lists "key"), color grading via the rich
`apply_color` (wheels/curves/hue-curves/LUT; our `set_color_grade` overlaps),
music via `generate_audio` (text-to-music models). Converging on upstream's
consolidated shapes would break our own MCP surface — a user decision, not
drift cleanup. `update_text` PORTED 2026-07-05 (merge semantics, clipIds +
captionGroupId addressing, 'off' clears animation; auto-fit box on typography
changes deferred - needs render-layer text measurement, same as add_texts).
Remaining un-ported upstream tools: `export_project` (needs an export host
seam), `send_feedback` (network backend), `get_projects`/`new_project` (#238
app-nav).

**Resolved 2026-07-05 by the v0.6.1 re-audit:** the speculative
`set_clip_audio_effects`/`set_clip_noise_reduction` stubs were REMOVED — upstream
never shipped those names; the real feature landed as **#251 `denoise_audio`**
(ported: denoise is an `audio.denoise` effect in the existing `effects` stack,
no model change; the DeepFilterNet3 bake is host-deferred).

## Upstream re-audit 2026-07-05 (9a3ae50 → 771b63e, v0.6.1)

6 new commits. Substantive: **#251 Audio Enhancer/Denoise** (PORTED — see above)
and **#255 multiple timelines per project** (3978-line diff, PORTED IN LARGE
PART 2026-07-05; three parallel auditors extracted the exact contracts first).
Ported: the `ProjectFile` project.json root (legacy fallback mirroring Swift,
`Timeline.id/name/folderId`, `Track.displayHeight` clamped 32..200, viewStates
round-trip, sibling-preserving saves incl. the narrow autosave path);
**nesting realigned** from the speculative #155 `compound_timelines` map to
Swift's shipped representation (sequence carriers referencing sibling
timelines — recursive compositor render with group-as-unit carrier transform,
`flatten_nests` for audio/export, executor sibling store, full app wiring);
`create_timeline`/`set_active_timeline`/`duplicate_timeline` tools (59→62) +
`get_timeline` timelines list + prompt paragraph; `add_clips` timelineId
nesting (linked A/V carriers, empty + cycle rejection); `insert_clips`
timelineId nesting (ripple splice, linked audio carrier keeps sequence
source); `rename_media`/`delete_media` accept timelineIds (last-timeline
guard, deleting the active switches to a sibling). Pending: export timelineId
arg (no Rust export_project tool yet), native nested-sequence XML/FCPXML
emission (v1 flattens, content-correct), timeline tab UI. Remainder of the range: version bumps + README fix (skipped). Upstream
branches `feat/audio-suite` and `multicam` exist but are not on main — not
audited.

## Recommended execution order

1. **Tier 1 batch** (correctness, testable): #236+#233 (add_clips), #224, #218, #207+#227, #189, #243.
2. **Tier 2 agent tools**: #177 → #186 → #178 → #160 → #152 → #249/#219/#242/#245.
3. **Tier 3 by sub-spec**: FCPXML export (largest interchange gap) → apply_layout → text animation → MCP stateful sessions → generation recovery → MCP project nav.
4. **Tier 4 UI**: window sizing (#204) + zoom factor first (quick parity), then Skills (#199), inspector settings (#168), update badge (#196), preview double-click (#191), chat focus (#159).

Each Tier 3 subsystem gets its own spec file before implementation.

---

## Ported PRs

| PR | Title | Rust Port | Rust Crate(s) | Notes |
|----|-------|-----------|---------------|-------|
| #8 | Colors + Effects via Metal + Custom Compositor | DONE | agent_contract (effects pipeline) | Effects pipeline ported, Metal layer skipped |
| #40 | Transcription language setting | DONE | core_model (Timeline.transcription_language) | serde round-trip verified |
| #46 | Shape annotations + animation tools | DONE | core_model, agent_contract | ShapeStyle, animation tools |
| #65 | Font weight in TextStyle | **DONE (on-disk compat fixed 2026-07-05)** | core_model (TextStyle) | The `TextStyle` on-disk format is now compatible with current Swift (9a3ae50). Implemented a `#[serde(from = "TextStyleWire", into = "TextStyleWire")]` bridge: on LOAD it accepts EITHER `fontWeight` (Rust) OR Swift's `isBold` (→700/400, `fontWeight` wins when both present) and reads `isItalic` into a new `TextStyle.is_italic: bool` field; on SAVE it writes BOTH `fontWeight` AND `isBold`/`isItalic`, so a `.palmier` written by either app round-trips bold + italic (additive — no key removed, so existing Rust readers are unaffected). Fixes the bidirectional data loss (Swift-authored bold/italic no longer decodes as regular in Rust; Rust weight no longer dropped by Swift). The FCPXML title `fontFace` now uses both bold and italic (`Bold`/`Italic`/`Bold Italic`/`Regular`, mirroring Swift `fontFaceFallback`). `is_italic` ripples to the (few, mostly `..Default::default()`) `TextStyle` literals. 3 new compat tests (Swift `isBold`/`isItalic` load, both-key precedence, dual-write round-trip) plus the pre-existing #65 fontWeight test and the full-project serde round-trip (proves no field dropped). Surfaced 2026-07-05 by the adversarial FCPXML title audit. Remaining nicety: fontName→trait inference for the rare no-`isBold`-no-`fontWeight` file (defaults to 400 as before); the Rust text renderer still doesn't slant italic glyphs (data is preserved for export). |
| #92 | Words-per-caption setting | DONE | search_core (CaptionConfig, phrases_from_words) | 6 tests |
| #94 | Export resolutions (2K, Match Timeline) | DONE | render_core (ExportResolution) | 720p, 1080p, 1440p, 4K, MatchTimeline |
| #105 | .aifc/.flac import support | DONE | core_model (ClipType::from_extension) | 5 extension tests |
| #114 | Fix set_clip_properties rotation | DONE | timeline_core | Rotation fix in clip_properties |
| #115 | Fix writePosition keyframe corruption | DONE | timeline_core | write_position fix |
| #129 | Fix keyframe loss on speed change | DONE | timeline_core (keyframes.rs) | RescaleClipKeyframes preserves keyframes |
| #135 | Missing-media cache pattern | DONE | core_model, agent_contract | missing_entry_ids(), media_offline_ids(), is_media_offline() |
| #136 | XMEML source timecode | DONE | render_core (xml_export.rs), core_model | SourceTimecode struct, format_timecode(), timecode_tags() |
| #144 | Validate speed/volume/opacity/trim | DONE | agent_contract (mutation.rs) | Input validation in set_clip_properties |

---

## Not ported — Swift-specific (no Rust work needed)

| PR | Title | Reason Skipped |
|----|-------|----------------|
| #74 | naturalTimeScale for clip inserts | AVFoundation-specific. clip insert timing uses native CMTime scale. |
| #127 | Fix Metal CIKernel effects rendering as passthrough | Metal shader fix. No Rust Metal kernel code. |
| #130 | Identify Sentry events by Clerk user id | Sentry + Clerk platform integration. |
| #133 | Fix main-thread hang when capturing project thumbnail | Swift AppKit main-thread pattern. |
| #147 | fix: safe-cast format description in readSourceTimecode | AVFoundation CFTypeID cast; was reverted upstream anyway. |
| #149 | Revert of #147 | N/A — revert of a Swift-specific change. |
| #150 | fix: guard timecode format description using CFTypeID | AVFoundation CFTypeID guard. Our Rust impl doesn't use format descriptions. |

---

## Not ported — needs spec work first

| PR | Title | Scope | Action Needed |
|----|-------|-------|---------------|
| #119 | Syncing multiple audio tracks | Large feature. Audio DSP (AudioEnvelope, AudioSyncCorrelator, AudioTrackReader), new agent tool(s), sync menu and toast UI. ~600 LoC Swift. | **IMPLEMENTED 2026-07-05 (Option A)** per `specs/rust-rewrite/12-audio-sync.md`: `sync_audio_clips` tool (60th) correlates each target against the reference (`AudioSyncCorrelator`, #174 `ClipAudioSource` seam) and moves it into sync via `move_clips` (undo via exec_mut; result reports `newClipId` since move re-ids the clip). Anchor formula `delta = ref_anchor − tgt_anchor − offset` with the sign pinned by a padded-clip oracle test; sub-`minConfidence` (default 0.5) targets stay put and are reported. Offset is baked into `start_frame` (no model change). Deferred: `sync_offset_frames` metadata (Option B), sync menu/toast UI, speed≠1 targets. |

---

## Small Swift-only fixes (already in upstream, no Rust impact)

These are one-line or small fixes in Swift code that don't correspond to any
Rust module:

| Commit | Description | Why Not Applicable |
|--------|-------------|--------------------|
| `1dda15f` | Run agent read_image decode off the main actor | Swift concurrency actor isolation. |
| `2e1510f` | Serialize imported-asset finalization to fix batch-generation crash | Swift actor/queue fix. |
| `0fc0e94` | Remove 4K kernel-cost benchmark | Removed a Swift benchmark. |
| `61c8589` | Fix fatal crash: load Metal kernels without Bundle.module | Swift resource loading. |
| `f3f4692` | Fix fatal crash: load Metal kernels without Bundle.module | Same, follow-up. |
| `067680b` | bundle.sh: ship SwiftPM resource bundle so Metal kernels load in release | Swift packaging. |

---

## Rust-side PRs / changes not driven by upstream

These are changes unique to Fronda (no Swift equivalent):

| Change | Crate | Description |
|--------|-------|-------------|
| Generation state machine | generation_core | GEN/ACC/EXP state machines (31 tests) |
| Account state machine | generation_core | Unconfigured/MissingKeys/Ready/Misconfigured |
| Export state machine | generation_core | Idle → Rendering → Cancelling → Completed/Failed |
| UserSettings | generation_core | Notifications, telemetry, disabled_models, agent_api_keys |
| ModelCatalog | generation_core | Filtering disabled models |
| gpui-ce app shell | app_shell_gpui | Window/pane/menu/shortcut shell |
| MCP server | mcp_server | HTTP+JSON-RPC transport |
| Composition plan | render_core | CompositionPlan + DetailedCompositionPlan + validation |
| Audio allocation | render_core | allocate_audio_composition_tracks() |
| Preview helpers | render_core | seek_frame, timescale conversion, content detection |
| Project I/O | project_io | .palmier bundle open/save |
| Search types | search_core | SearchResults, SearchHit, VisualIndex, EmbeddingRow |
| Transcript types | search_core | Transcript, TranscriptSegment, TranscribedWord, TranscriptRange |
| Caption builder | search_core | phrases_from_words() grouping algorithm |
| Snap engine | timeline_core | collect_targets, find_snap, sticky/playhead multi-probe |
| Range selection | timeline_core | TimelineRange, shift_drag_range, drag_range_edge, gap finding |
| Track operations | timeline_core | add/remove/reorder tracks |
| Workflow planner | timeline_core | Multi-track ripple, gap delete, sync-lock |
| Clip properties | timeline_core | Set properties with rotation fix |
| Inspector | timeline_core | Timeline inspect queries (INS-001~011) |
| Keyframe rescale | timeline_core | RescaleClipKeyframes on speed change |
| Missing-media helpers | agent_contract, core_model | media_offline_ids, is_media_offline, is_media_unprocessable |

---

## Swift tests not yet covered by Rust

Source: `Tests/PalmierProTests/` directory listing.

### Captions
| Swift Test File | Rust Coverage | Priority |
|-----------------|---------------|----------|
| CaptionBuilderTests.swift | PARTIAL — `phrases_from_words` tested, full builder pipeline missing | Medium |
| CaptionGenerationTests.swift | NOT STARTED — End-to-end generation from transcript → captions | Medium |
| TranscriptCacheTests.swift | NOT STARTED — Cache identity and invalidation logic | Low |
| TranscriptionLocaleTests.swift | NOT STARTED | Low |

### Export
| Swift Test File | Rust Coverage | Priority |
|-----------------|---------------|----------|
| CompositionBuilderTests.swift | PARTIAL — CompositionPlan exists, full builder parity not verified | Medium |
| ExportResolutionTests.swift | DONE | — |
| ExportServiceRoundTripTests.swift | NOT STARTED | Low |
| LottieExportTests.swift | NOT STARTED | Low |
| PalmierProjectExportTests.swift | NOT STARTED — Self-contained project export | Medium |
| TextExportGlyphTests.swift | NOT STARTED | Low |
| XMLExporterTests.swift | DONE | — |

### Media
| Swift Test File | Rust Coverage | Priority |
|-----------------|---------------|----------|
| ImageVideoGeneratorTests.swift | NOT STARTED | Low |
| LottieDotLottieTests.swift | NOT STARTED | Low |
| LottieImportTests.swift | NOT STARTED | Low |
| LottieVideoGeneratorTests.swift | NOT STARTED | Low |
| MediaPanelTests.swift | NOT STARTED | Low |
| MediaResolverTests.swift | DONE (via missing_entry_ids) | — |
| OverviewRendererTests.swift | NOT STARTED | Low |
| ProjectRegistryTests.swift | NOT STARTED | Low |
| ProjectRoundTripTests.swift | DONE (project_io tests) | — |

### Rendering
| Swift Test File | Rust Coverage | Priority |
|-----------------|---------------|----------|
| RGBAHexTests.swift | NOT STARTED | Low |
| TextLayerOpacityAnimationTests.swift | NOT STARTED | Low |
| TransformCropTests.swift | PARTIAL — top_left, crop_identity, visible_fractions ported; snap_to_boundary, snap_to_canvas_edges missing | Medium |

### Search
| Swift Test File | Rust Coverage | Priority |
|-----------------|---------------|----------|
| EmbeddingStoreTests.swift | NOT STARTED | Low |
| FrameSamplerTests.swift | NOT STARTED | Low |
| ModelDownloaderTests.swift | NOT STARTED | Low |
| MomentDragTests.swift | NOT STARTED | Low |
| SearchIndexCoordinatorTests.swift | NOT STARTED | Low |
| SegmentTrimTests.swift | NOT STARTED | Low |
| TextTokenizerGoldens.swift | NOT STARTED | Low |
| TextTokenizerTests.swift | NOT STARTED | Low |
| VisualEmbedderParityTests.swift | NOT STARTED | Low |
| VisualIndexerTests.swift | NOT STARTED | Low |
| VisualSearchTests.swift | NOT STARTED | Low |

### Timeline
| Swift Test File | Rust Coverage | Priority |
|-----------------|---------------|----------|
| ClipKeyframeExtensionTests.swift | NOT STARTED | Low |
| ClipMathTests.swift | DONE | — |
| ClipMutationsTests.swift | DONE | — |
| KeyframeTests.swift | DONE | — |
| LinkingTests.swift | DONE | — |
| OverwriteEngineTests.swift | DONE | — |
| RippleDeleteRangesTests.swift | DONE | — |
| RippleEngineTests.swift | DONE | — |
| RippleGapDeleteTests.swift | DONE (workflow planner) | — |
| SnapEngineTests.swift | DONE | — |
| TimelineGeometryTests.swift | NOT STARTED | Low |
| TimelineRangeSelectionTests.swift | PARTIAL — basic range ops done, full selection behavior missing | Medium |
| TrackDisplayLabelTests.swift | NOT STARTED | Low |

### Transcription
| Swift Test File | Rust Coverage | Priority |
|-----------------|---------------|----------|
| TranscriptSearchTests.swift | NOT STARTED | Low |

### Other
| Swift Test File | Rust Coverage | Priority |
|-----------------|---------------|----------|
| AgentMentionTests.swift | NOT STARTED | Low |
| GetTranscriptParamTests.swift | NOT STARTED | Low |
| RemoveTracksTests.swift | DONE (track_ops) | — |
| SearchMediaToolTests.swift | DONE (read_tools) | — |
| ShortIdTests.swift | NOT STARTED | Low |
| ToolExecutorTests.swift | DONE (65 exec_* tests) | — |
| UndoToolTests.swift | DONE | — |
| SmokeTests.swift | NOT STARTED | Low |
| TimeFormattingTests.swift | PARTIAL — timecode_tags tested, general formatting not | Low |
| FixtureVideo.swift | N/A — test fixture | — |

---

## Recommended next actions (priority order)

1. **Port `snap_to_boundary` / `snap_to_canvas_edges` on Transform** — easy win,
   Swift TransformCropTests already mapped; low risk, high parity value.
2. **Port `TimelineGeometryTests`** — timeline coordinate math.
3. **Port `ClipKeyframeExtensionTests`** — clip-level keyframe helpers.
4. ~~Write spec for PR #119 (audio sync)~~ — **DONE 2026-07-05** (`12-audio-sync.md`). Implementation is unblocked pending the offset-application decision (move clip vs `sync_offset_frames` field).
5. **Port `CaptionBuilderTests` / `CaptionGenerationTests`** — full caption
   pipeline from transcript → captions.
6. **Port `CompositionBuilderTests`** — verify CompositionPlan matches Swift.
7. **Port `PalmierProjectExportTests`** — self-contained .palmier export.
8. **Search/indexing pipeline** — requires significant new type + algorithm work.

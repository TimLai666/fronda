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
| #207 | ripple_delete_ranges: per-call sync-lock exemption (`ignoreSyncLockedTracks`) | `timeline_core/src/workflow.rs` RippleDeleteConfig | M | Missing param. |
| #227 | Master audio: sync-locked follower tracks are CUT (cleared), not just shifted | `timeline_core/src/workflow.rs` compute_ripple_delete | M | Missing sync-lock clear loop. Correctness. |
| #243 | Default agent model → Sonnet 5 | `app_contract/settings_storage.rs` AGENT_DEFAULT_MODEL, `chat_view.rs` model list | XS | **DONE 2026-07-03** — `sonnet46`→`sonnet5`; chat list `claude-sonnet-4-6`→`claude-sonnet-5`. Backend accepts both during rollout. Tests updated. |

## Tier 2 — New agent tools / schema (medium) — parity features

| PR | Tool / change | Target | Effort |
|----|---------------|--------|--------|
| #177 | `set_project_settings` tool + auto-match timeline on empty add | agent_contract | H | **DONE 2026-07-03** (tool + presets + fps rescale + refit + undo; 6 tests). The rescale/refit core (`apply_fps`/`refit_transforms`/`apply_settings`) already existed in `timeline_core/src/project_settings.rs`; only the tool layer was missing. **Auto-match-on-add (`applySettingsIfNeededForAgent`) still deferred.** |
| #186 | `split_clip` → `split_clips` batch (two modes, dedup, A/V regroup) | agent_contract | M | **DONE 2026-07-03** — renamed tool; explicit `splits` + `trackIndex`/`frames` modes; validate-all-then-apply (no partial state); dedup; re-resolves each cut against current sub-clips; reuses `timeline_core::split_clip` (already does linked A/V regroup). 3 tests. Also added `object_optional` schema helper (the shared `object()` marked every prop required — wrong for exactly-one-of tools; now used by split_clips + set_project_settings for correct MCP schemas). |
| #178 | `language` (BCP-47) param threaded into get_transcript/inspect_media | agent_contract read path | M (schemas already list it) |
| #160 | `remove_words` tool (word→frame, ripple linked partners) | agent_contract | M |
| #152 | `send_feedback` tool (session dedup + 8/session cap + diagnostics) | agent_contract | L |
| #245 | `TranscribedWord.speaker` field + `remove_words` `matches` filler-token arg | search_core, agent_contract | L-M (cloud provider itself is SWIFT-ONLY) |
| #249 | `paid_only` on model catalog + free-tier gating in list_models | generation_core, agent_contract | M |
| #219 | `import_status` field on manifest entry (preparing/downloading/failed) | core_model, agent_contract | M |
| #242 | `create_matte` tool + `ClipType::Matte` + MatteAspect presets | core_model, agent_contract | M |

## Tier 3 — Large new subsystems — need own sub-spec each

| PR(s) | Subsystem | Target | Effort |
|-------|-----------|--------|--------|
| #193 | **FCPXML export v1 baseline** | `render_core/src/fcpxml_export.rs` | XL | **v1 DONE 2026-07-03** — valid FCPXML 1.10: `<resources>` (format + deduped assets w/ media-rep src), `<library>/<event>/<project>/<sequence>/<spine>` with a full-length `<gap>` anchoring every clip as a connected `<asset-clip>` at absolute offset, lane per track (video +, audio −), rational project time. 6 tests. **Wired into the export UI + execution**: `ExportMode::Fcpxml` ("Final Cut Pro (.fcpxml)"), pure `interchange_content()` generator (also covers XMEML), and `write_interchange()`. The Export button now opens a save dialog (`prompt_for_new_path`) and writes the `.xml`/`.fcpxml` file from the live editor timeline+manifest — so both interchange formats actually produce files, and the panel shows a success ("Exported to …") or failure message (`set_interchange_result`). (Video render execution + `.palmier` bundle write are separate, still TODO.) |
| #214 | FCPXML format naming + Rec.709 colorspace | `render_core/src/fcpxml_export.rs` | L | **DONE 2026-07-03** — `format_rate_suffix`/`recognized_video_format_name`/`sequence_format_name`/`frame_duration_str` (NTSC-aware); sequence `<format>` now named (`FFVideoFormat1080p30`, else `FFVideoFormatRateUndefined`) with `colorSpace="1-1-1 (Rec. 709)"`. 3 helper tests + updated exporter tests. Per-asset formats still shared with the sequence (full per-asset formats come with #206). |
| #206 | FCPXML per-asset formats (partial) | `render_core/src/fcpxml_export.rs` | M | **Per-asset formats DONE 2026-07-03** — each visual asset now emits its own `<format>` from its source width/height/fps (`video_format_name`, `frame_duration_str`); audio assets carry no format; visual assets without source dims fall back to `r1`. Assets already dedup by media_ref (same-source A/V collapses to one asset w/ hasVideo+hasAudio). **DONE**: also fixed the frameDuration grid (project fps, not source fps, so asset duration + asset-clip in-point align) and per-clip `format` reference (own format for video, omitted for audio); and the synced-A/V-pair collapse — `redundant_audio_clip_ids` drops the audio partner of a 1-video/1-audio link group sharing source/timing/trim/speed AND enabled state. The `enabled` guard IS ported (derived from the TRACK: video `!hidden`, audio `!muted`) — a muted audio partner is NOT collapsed, or its silenced audio would become audible via the video asset-clip. Multiple tests. Only Swift's volume-from-partner emission is unported (the Rust exporter emits no per-clip volume). |
| #197 #247 #254 | FCPXML refinements on the v1 baseline | `render_core/src/fcpxml_export.rs` | L each | retime `<ref-clip>` compound wrapping (#197), source timecode (#247), per-target (Resolve/FCP) transform/crop/blend calibration + text stroke (#254). |
| #183 | Export write-failure surfacing (`export` returns `String`, not `Result`) | render_core, export_model | S (prereq for FCPXML robustness) |
| #226 c9222fe #1a5aa2c | **apply_layout** — VideoLayout enum (10 layouts), LayoutSlot, transform/crop math, batch clips per slot | core_model, agent_contract | XL | **Geometry + placement math + re-layout tool DONE 2026-07-03** — `core_model/src/video_layout.rs`: `VideoLayout` (10 layouts, exact Swift slot rects, pip inset 0.28 / margin 0.035, z-order), `media_canvas_aspect`, `crop_fitting_aspect`, `layout_placement` (fill/fit → Transform+Crop w/ anchors). Plus the `apply_layout` **agent tool** (`cmd_apply_layout` + `resolve_layout_anchor` + `apply_layout_place_new`): **FULLY PORTED**. Re-layout mode — each slot takes a `clipIds` array (batch takes into one region; singular `clipId` still accepted), same-track cross-slot time-overlap check + multi-slot coincidence check, all validated before mutation. Place-new mode — each slot takes a `mediaRef` plus top-level `startFrame`/`durationFrames`; creates a stacked video track per slot by z-order (insert-at-0 ascending z → highest z on top), places a new clip per slot with the layout transform/crop baked in, and auto-detects project settings from the first video (via the #177 auto-match seam). 25 tests (7bcfafb). |
| #225 | **Text animation** — TextAnimation/WordTiming/preset enums + agent args (data model portable; renderer is UI) | core_model, agent_contract | L | **DATA MODEL + AGENT ARGS DONE 2026-07-04** — `core_model::text_animation` (WordTiming, TextAnimation, 11-preset enum w/ Swift rawValues + render-mode/agent-value helpers); `Clip.text_animation`/`word_timings` (serde round-trip, no data loss); `timeline_core::rescale_word_timings` on duration change (Swift `setDuration`); `add_texts` accepts `animation`/`highlightColor` (`parse_text_animation`). Only the renderer (TextAnimator/TextFrameRenderer) is UI-deferred. |
| #216 | **Generation recovery** — backend_job_id/result_urls persistence, resume in-flight jobs, generation_status enum | core_model, generation_core | XL | **DATA MODEL (persistence) DONE 2026-07-04** — `GenerationInput.backend_job_id` + `output_index` + `result_urls` (serde `resultURLs` uppercase-acronym key + alias) AND `MediaManifestEntry.generation_status` (serde `generationStatus`, surfaced by `get_media` with a "poll until 'none'" hint) round-trip, so a project saved mid-generation keeps its resumable job + lifecycle state. 3 round-trip/get_media tests. **Still to port**: resume LOGIC only — subscribe to the backend job / restore on launch (app-wired, needs a GenerationBackend adapter). |
| #250 | **MCP stateful sessions** — Mcp-Session-Id routing, per-session Server, Content-Length framing, LRU(32)+1h prune, SSE, tools/list_changed | mcp_server | XL |
| #238 | **MCP project navigation** — get/open/new_project tools + app-layer project registry + `.system` message role | agent_contract, app layer | L-XL |

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
`SYSTEM_INSTRUCTION`; keyframed XMEML export + `file://` path norm + `<file>`
dedup + fade→transitionitem (all need the media manifest threaded into
`xml_export` as done for FCPXML); SHA256 transcript cache identity (needs `sha2`).
The sweep also confirmed many suspected gaps are already DONE (keyframe
smoothstep interpolation, folder-drag, `resolved_transform_at`/`crop_at`,
model-config validate/discount).

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
| #65 | Font weight in TextStyle | DONE | core_model (TextStyle.font_weight) | serde round-trip verified |
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
| #119 | Syncing multiple audio tracks | Large feature. Audio DSP (AudioEnvelope, AudioSyncCorrelator, AudioTrackReader), new agent tool(s), sync menu and toast UI. ~600 LoC Swift. | Needs a design spec before porting. Involves: cross-correlation math, PCM decoding abstraction, new `sync_audio_clips` tool, timeline undo for sync operations, platform adapter for audio file I/O. |

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
4. **Write spec for PR #119 (audio sync)** — before any implementation work.
5. **Port `CaptionBuilderTests` / `CaptionGenerationTests`** — full caption
   pipeline from transcript → captions.
6. **Port `CompositionBuilderTests`** — verify CompositionPlan matches Swift.
7. **Port `PalmierProjectExportTests`** — self-contained .palmier export.
8. **Search/indexing pipeline** — requires significant new type + algorithm work.

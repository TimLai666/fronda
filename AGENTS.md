<!-- SPECTRA:START v1.0.2 -->

# Spectra Instructions

This project uses Spectra for Spec-Driven Development(SDD). Specs live in `openspec/specs/`, change proposals in `openspec/changes/`.

## Use `$spectra-*` skills when:

- A discussion needs structure before coding → `$spectra-discuss`
- User wants to plan, propose, or design a change → `$spectra-propose`
- Tasks are ready to implement → `$spectra-apply`
- There's an in-progress change to continue → `$spectra-ingest`
- User asks about specs or how something works → `$spectra-ask`
- Implementation is done → `$spectra-archive`
- Commit only files related to a specific change → `$spectra-commit`

## Workflow

discuss? → propose → apply ⇄ ingest → archive

- `discuss` is optional — skip if requirements are clear
- Requirements change mid-work? `ingest` → resume `apply`

## Parked Changes

Changes can be parked（暫存）— temporarily moved out of `openspec/changes/`. Parked changes won't appear in `spectra list` but can be found with `spectra list --parked`. To restore: `spectra unpark <name>`. The `$spectra-apply` and `$spectra-ingest` skills handle parked changes automatically.

<!-- SPECTRA:END -->

# Spectra repo-specific overrides

**Do NOT park changes in this repo.** Never run `spectra park` or move changes out of `openspec/changes/`. Unimplemented change proposals stay in `openspec/changes/` and are committed to git — we track not-yet-implemented specs in version control. If a parked change is ever found, restore it with `spectra unpark <name>` and commit it. This rule overrides the auto-generated Spectra instructions above.

# Fronda Rust-first repo rules

Primary implementation: `Fronda`, a cross-platform Rust app. Legacy behavioral reference: Palmier Pro on Swift 6.2, SwiftUI + AppKit, AVFoundation.

The Swift baseline targets macOS 26, arm64 only, non-sandboxed Developer ID. **Fronda is the primary codebase and is cross-platform** - it builds and runs on macOS, Windows, and Linux. UI is built with `gpui-ce` and must visually match the Swift version exactly. UI work does not require a macOS machine; develop and verify on any platform using gpui.

## Build

Primary Rust workflow:

```bash
cargo test --workspace
cargo check -p fronda-app-shell-gpui --features desktop-app --bin fronda
```

Legacy Swift compatibility baseline:

```bash
swift build
swift run
```

## Code style

- Keep comments minimal. Only write one when the _why_ is non-obvious. Don't restate what the code does, don't narrate the current change, don't leave `// removed X` breadcrumbs. One short line max - no multi-line comment blocks or paragraph docstrings.

## Design System

All UI styling MUST use `AppTheme` constants from `Sources/PalmierPro/UI/AppTheme.swift`. Never use hardcoded numeric values for:

- **Spacing/padding** -> `AppTheme.Spacing.*` (xxs through xxl)
- **Font sizes** -> `AppTheme.FontSize.*` (xxs through display)
- **Font weights** -> `AppTheme.FontWeight.*` (regular, medium, semibold, bold)
- **Corner radii** -> `AppTheme.Radius.*` (xs through xl)
- **Border widths** -> `AppTheme.BorderWidth.*` (hairline, thin, medium, thick)
- **Opacity** -> `AppTheme.Opacity.*` (subtle, faint, muted, medium, strong, prominent)
- **Icon frame sizes** -> `AppTheme.IconSize.*` (xs through xl)
- **Shadows** -> `AppTheme.Shadow.*` (sm, md, lg) via `.shadow(AppTheme.Shadow.md)`
- **Colors** -> `AppTheme.Text.*`, `AppTheme.Border.*`, `AppTheme.Background.*`
- **Animation durations** -> `AppTheme.Anim.*`

If a needed value doesn't exist in AppTheme, add it there first - don't hardcode it.

## Drag and drop

SwiftUI `.onDrop` on a parent view shadows every drop target inside its layout area on macOS 26 - even AppKit `NSDraggingDestination` children registered directly with the window. Inner `.onDrop` modifiers silently never fire while a parent `.onDrop` is active.

Rule: **any drop target that spans an area containing other drop targets must use native AppKit** (see `MediaPanelDropArea` in `Sources/PalmierPro/MediaPanel/`). Inner / leaf drops can stay SwiftUI `.onDrop`. Do not stack SwiftUI `.onDrop` modifiers in parent/child layouts.

## Voice

For Rust-side product and UI copy, Fronda speaks like a quietly capable desktop editor for filmmakers: direct, technical, calm, and confident. Prefer Apple HIG-style terseness over warmth. Never chatty or cute. Never marketing. When the product needs to ask for action, lead with the action verb; when it reports state, name the thing.

## Rust rewrite rules

This repo's primary implementation is the cross-platform Rust app `Fronda`. The current Swift codebase remains the behavioral reference until a Rust implementation explicitly replaces a subsystem.

- Treat `specs/rust-rewrite/` as the compatibility baseline for the rewrite. If behavior changes intentionally, update the relevant spec in the same change and mark the decision explicitly.
- For rewrite work, prefer preserving observable behavior over line-by-line source translation. Port the contract, not the syntax.
- Default Rust UI stack: `gpui-ce`. Use it for windows, panes, focus, input, drag/drop, and app shell behavior.
- `Fronda` is the Rust rewrite name. The current Swift module/resource/runtime identifiers such as `PalmierPro`, `palmier-pro`, and `palmier://` are still compatibility identifiers. Do not rename them piecemeal; any identifier migration must be explicit and spec-backed.
- Keep non-UI logic out of `gpui-ce` whenever possible. Timeline math, persistence, media-library logic, agent/MCP contracts, search/indexing state, and export planning should live in plain Rust crates/modules with no UI dependency.
- Core Rust logic must not depend on platform APIs directly. Wrap platform-specific behavior such as file dialogs, notifications, secure credential storage, updater hooks, trash/reveal-in-file-manager, and window chrome behind explicit adapters.
- Preserve compatibility with existing on-disk/project contracts unless a spec says otherwise:
  - `.palmier` project packages
  - `project.json`
  - `media.json`
  - `generation-log.json`
  - `chat/*.json`
  - agent tool names and schemas
  - MCP resource/tool surface
- All timeline math remains frame-based. Use integer project frames as the source of truth; never let source-native fps silently replace project fps in editing logic.
- Non-obvious clip-resolution contracts (mirror Swift exactly; verified in `timeline_core`/`render_core` tests — do not regress):
  - `position_track` stores normalized **top-left**, not centre (spec `INS-003`). `resolved_transform_at` resolves scale first, then centre = top_left + size/2. Any renderer/exporter reading position keyframes must apply the same conversion (XMEML `center` param does).
  - `volume_track` keyframe values are **decibels**. Effective linear gain = `clip.volume * linear_from_db(sampled_dB)` (static volume is an outer gain, keyframes do not replace it). `opacity_track` is plain linear 0..1 and does replace the static.
  - Fade multiplier = `min(in_ramp, out_ramp)` where each ramp is `t` or `smoothstep(t)` per `fade_in/out_interpolation`; no half-frame offset. Applies to video opacity (`fade_multiplier_at`) and audio (`audio_mixer::fade_gain`) identically.
  - The agent `set_keyframes` tool takes Swift-format array rows `[frame, ...values, interp?]` (interp ∈ linear/hold/smooth, default smooth), supporting all six properties: opacity/volume/rotation (1 value), position/scale (2), crop (4, order top,right,bottom,left). `tool_exec::parse_keyframe_rows` + `keyframe_property_arity` are the shared parser used by both the executor and `mutation::validate_set_keyframes` so the two never diverge.
- Avoid expanding the Swift app with large new features unless explicitly requested. While the rewrite is in progress, prefer:
  - bug fixes
  - parity/spec capture
  - migration scaffolding
  - compatibility fixes
- When porting a subsystem, bring its tests/spec coverage with it. Do not mark a port complete if the behavior only exists informally in code.
- Test priority for Rust work:
  1. pure unit/property tests for math and state transitions
  2. serde/snapshot tests for file formats and agent/MCP contracts
  3. fixture-based integration tests for import/export/search/transcription/generation flows
  4. `gpui-ce` interaction tests only for behavior that is inherently UI-driven
- Keep Rust modules deterministic and explicit. Prefer small pure functions, explicit data flow, and stable serialized structures over hidden globals or view-owned business logic.
- Generation, account, and export state machines live in `crates/generation_core/` (crate name `generation_core`, package `fronda-generation-core`). Pure state machine logic with no platform dependencies.
- `ClipType::from_extension(ext)` in `core_model` classifies file extensions into clip types (covers aiff, aifc, flac).
- XML export supports source timecode (`SourceTimecode` struct + `format_timecode`/`timecode_tags` functions) in `render_core/src/xml_export.rs`.
- `MediaManifestEntry` has optional `source_timecode_frame`, `source_timecode_quanta`, `source_timecode_drop_frame` fields (upstream PR #136).
- `GenerationInput` implements `Default`.
- `MediaManifest::missing_entry_ids(now, callback)` / `resolve_url_for(id, now, ..)` / `is_missing_for(id, now, ..)` take an injected `now` and treat an entry as not-missing only when its cached remote copy is FRESH (`MediaManifestEntry::cache_is_fresh(now)` = URL set AND expiry unexpired; None-expiry is lenient/fresh). VERIFIED 2026-07-04: Swift PR #135 is a stat()-caching perf fix whose `missingAssetIds` keys on LOCAL-FILE existence only and does NOT consult `cachedRemoteURL` — so the cached-URL exclusion is a Rust *enhancement* (the earlier "(upstream PR #135)" attribution was wrong). It now mirrors Swift `MediaAsset.freshRemoteURL`'s expiry check so an expired cache no longer hides an offline asset. `ToolExecutor::media_offline_ids/is_media_offline/is_media_unprocessable` thread `now` too; app-layer callers pass `chrono::Utc::now()`.
- media.json serde keys must preserve Swift's uppercase acronyms (`sourceFPS`, `cachedRemoteURL`, `imageURLs`, `imageURLAssetIds`) — `rename_all="camelCase"` lowercases them and silently drops the fields. `media_manifest.rs` has explicit `rename` + `alias` on the affected fields; keep new acronym-ending fields renamed too. `project_io::write_json` writes atomically (temp+rename); `write_chat_sessions` prunes per-file and never deletes unreadable chat files.
- `ToolExecutor` has `media_offline_ids()`, `is_media_offline()`, and `is_media_unprocessable()` helpers that delegate to `missing_entry_ids()`.
- `render_core::compositor` is the pure CPU frame compositor: `compose_frame` flattens a timeline frame into RGBA — layer order, per-clip transform/crop/opacity, rotation, flip (`flip_image`), all 16 blend modes via `blend_rgb` (weighted by backdrop alpha per W3C: `(1-dst_a)*src + dst_a*blend`), fades (`timeline_core::fade_multiplier_at`), keyframe-resolved via `timeline_core::resolved_*_at`, bilinear sampling. Also composites shape annotations (`rasterize_shape`: rect/oval/circle fill+stroke; arrow/line pending — endpoint coord space unconfirmed), chroma key (`apply_chroma_key` + spill), colour adjustments (`apply_color_adjustments`: exposure/contrast/saturation/brightness), blur (`apply_blur`), and vignette (`apply_vignette`). Text overlays render via `render_core::text` (bundled fonts + pure-Rust `ab_glyph`): per-family font selection (Anton/Bebas/Marker/Shrikhand/…, else Poppins), Regular/Bold weight, multiline, L/C/R alignment, letter spacing, line height, drop shadow, caption background (padding + rounded corners), and outline/stroke — with `font_size` scaled to Swift's 1080-tall reference canvas. Text rotation, shadow blur, and variable-font axes are follow-ups. `render_sequence` drives per-frame compose for the exporter. Media decode is a `fetch_source` closure (platform adapter), fully unit-tested with synthetic sources.
- LAYER-ORDER CONVENTION (critical, non-obvious): `timeline.tracks[0]` is the TOP visual layer, `tracks[n-1]` the bottom — proven by the XMEML export doing `.rev()` (so tracks[n-1] → FCP V1 bottom). The compositor walks tracks BACK-TO-FRONT (`tracks.iter().rev()`) so tracks[0] blits last (on top); FCPXML gives tracks[0] the HIGHEST lane. Do not "fix" these to forward order — it inverts multi-track compositing. `Transform.center_x/width` etc. are normalized `0..1` of the canvas.
- CLIP PRESETS (#157, `ToolExecutor.clip_presets`): `save/apply/list_clip_presets` use an in-memory `HashMap<name, ClipPreset>` on the executor (capture transform/crop/opacity/volume/speed/effects/blend/chroma; apply routes speed through `apply_clip_speed` so duration/keyframes stay correct). Session-scoped — persisting to project.json is a follow-up (a data-model decision). #157 is Rust-native (no Swift equivalent).
- REMOVE_SILENCE (#174): pure detector in `audio_core::silence_detector` (`rms_envelope` → `detect_silence` → `source_ranges_to_project_frames`; `source_offset_seconds = trim_start_frame/fps` per `extract_clip_audio`'s convention). Decoding is the `ClipAudioSource` host seam (like `MatteWriter`) — `app_shell_gpui::audio_source::ProjectAudioSource` decodes via ffmpeg; unset on MCP/headless → "unavailable". `cmd_remove_silence` ripple-deletes all detected ranges in one `apply_ripple_delete_on_track` call.
- NESTED TIMELINES (#255 representation; the earlier #155 `compound_timelines` map is GONE): a nest is a clip with `source_clip_type == ClipType::Sequence` whose `media_ref` is a SIBLING timeline's id in `ProjectFile.timelines` — nothing is embedded in the parent. `timeline_core::compound` has `nest_clips` (group → NEW child `Timeline`, returned for the caller's sibling store, + carrier), `decompose_nest` (NestFlattener windowing: window = `trim..trim+dur`, shift = `start-trim`), and `flatten_nests` (recursive, cycle-safe, depth ≤ `NEST_MAX_DEPTH` 8) used by AUDIO mixing + XML/FCPXML export. VIDEO does NOT flatten — `compose_frame_with_timelines` composes the child recursively at the blit-destination size, so the carrier's transform/crop/opacity apply to the group AS A UNIT (Swift `NestRenderTests` semantics). Its `fetch(clip, local_frame)` receives the frame on the clip's OWN timeline — do not seek with the root frame. The executor holds `sibling_timelines` (`set_sibling_timelines`/`sibling_timeline_map`); the hub seeds them from `bundle.multi` and saves via `save_project_state_with_siblings` (upsert by id). Sequence carriers don't retime (Swift `supportsRetiming == false`). `Clip.compound_timeline_id`/`Timeline.compound_timelines` remain as inert serde fields — nothing reads or writes them; do not use them.
- PROJECT FILE (#255): `project.json`'s root is `ProjectFile{timelines, activeTimelineId, openTimelineIds, viewStates}`; legacy bare-Timeline files decode via `ProjectFile::decode`'s fallback (mirrors Swift exactly incl. the original-error rethrow). `Timeline` has `id`/`name`/`folder_id`; `Track.display_height` serializes (clamped 32..200 on decode, default 50). `ProjectBundle.multi: MultiTimelineState` carries siblings in file order + `active_index` so saves preserve array order; the narrow `save_project_state` read-modify-writes so an autosave never deletes sibling timelines.
- `app_shell_gpui::video_export` renders a real timeline to mp4 via statically-linked ffmpeg: `Mp4Encoder` (RGBA→YUV420P, H.264 with MPEG-4 fallback), `decode_frame_rgba`, and `export_project` (resolves manifest sources, decodes each clip's mapped source frame, composites, encodes). Verified end-to-end against a committed H.264 fixture.
- `agent_contract::agent_loop` runs the Anthropic tool-use conversation loop (`run_agent_turn`, `parse_response`) behind the sync `LlmTransport` trait; `run_agent_turn` takes an `execute_tool` closure (not a borrowed executor) so a caller locks a shared executor only per tool call, never across HTTP. Pure and mock-tested. `app_shell_gpui::anthropic_transport::AnthropicTransport` is the concrete blocking reqwest (rustls) impl. Wired into the chat panel (`ChatView::spawn_agent_turn`) — send runs a background agent turn, `agent_bridge` maps tool records to chat `ToolCall`s for display. Live call needs `ANTHROPIC_API_KEY` (not auto-tested).
- `app_shell_gpui::video_export::export_project` is wired to the Export button (Video mode → composite + encode to a chosen .mp4 on the gpui background executor). `app_shell_gpui::preview_render::render_frame_png` composites one frame to a cache PNG; `PreviewView` shows it via `gpui::img` (paused-only, revision+frame keyed, off the UI thread) so the preview canvas displays the real composited frame.
- Audio pipeline (all verified): `audio_core::audio_mixer` (`mix` sums placed sources with gain/fades/clamp; `compute_peaks` for waveforms), `audio_core::wav` (16-bit PCM WAV encode/write), `render_core::audio_plan::mix_timeline_audio` (Timeline → placements → mix, pure), and `app_shell_gpui::audio_export` (`decode_audio_pcm` via ffmpeg resampler + `export_audio_wav` stem + `clip_waveform_peaks`). ffmpeg-the-third resampler needs mask-backed channel layouts pinned via `default_for_channels` (frames normalized with `set_ch_layout`), else swr errors "Input changed". Decode verified against self-generated PCM WAV fixtures.
- Video export with sound: `video_export::Mp4Encoder::new_with_audio` + `write_audio` mux an AAC stream (packed f32 → planar `frame_size` chunks, no resampler as mix rate == AAC rate, both streams from PTS 0). `audio_export::export_project_with_audio` composites video (one `SourceDecoder` per source, opened once) + mixes audio + muxes; the Export button (Video mode) calls it. Verified structurally (re-decode → both streams). `frame::Audio::new` needs a `ChannelLayoutMask` (`layout.mask()`); the encoder uses `set_ch_layout(ChannelLayout)`.
- Inspector Format section + preview badges read the live timeline (not hardcoded) via `EditorStateHub` + `timeline_core` formatters (#168 display).
- `timeline_core::project_presets` holds the inspector/preview dropdown data (upstream #168): `ASPECT_PRESETS`, `FPS_PRESETS`, `QUALITY_PRESETS`, `ZOOM_PRESETS` with active-selection logic, mirroring Swift `PreviewContainerView` exactly.
- XML export (`render_core/src/xml_export.rs`) emits keyframed motion params (scale/rotation/center/crop) and a keyframed Opacity filter when the clip has the matching animation track (XML-012). Fades export as single-sided `<transitionitem>`s (Cross Dissolve for video, Cross Fade for audio) at the track level — the form Premiere reads — NOT `<fadein>/<fadeout>` tags (`write_fade_transition`).
- Cross-platform: the `desktop-app` `fronda` bin compiles clean on Windows (gpui-ce + ffmpeg + reqwest), verified via `cargo check` (2026-07-03/04).
- `mcp_server::session` is the pure core of stateful MCP sessions (#250): `SessionStore` (LRU32 + 1h TTL, injected clock, monotonic `seq` for deterministic tie-break) + `parse_session_id` (skips colon-less header lines). Per-session executor wiring / SSE / tools/list_changed are deferred (basic single-shared-executor HTTP server already works).
- `mcp_server::server` enforces Issue #122 bearer auth: `start()`/`spawn()` call `config.validate()` (a non-loopback bind without a token is rejected), and `handle_connection` rejects any request lacking a matching `Authorization: Bearer <token>` with 401 (constant-time compare) before executing. Default config is loopback + no token (auth not enforced locally).

## Upstream PR management

- Upstream PRs were re-audited on 2026-07-05 (upstream HEAD `771b63e`, v0.6.1),
  covering the 6 new commits `9a3ae50..771b63e`: **#251 Audio Enhancer/Denoise**
  (ported — `denoise_audio` tool; no model change, denoise is an `audio.denoise`
  effect) and **#255 multiple timelines per project** (ProjectFile wrapper in
  project.json — compat-critical, under audit), rest version bumps/docs. Upstream
  also has non-main branches `feat/audio-suite` and `multicam` — not audited
  (main-only rule). Prior audits: 2026-07-03 at `9a3ae50` (86 commits,
  PRs ~#148–#254), 2026-06-25 at `b9b4ad9`. Full tiered results and the porting
  execution order are in `specs/rust-rewrite/97-upstream-pr-audit.md`.
- Do NOT re-fetch upstream or re-audit PRs unless there are new commits on
  the upstream `main` branch. Check with `git fetch upstream && git --no-pager log upstream/main --oneline | head -5` first.
- Only port upstream PRs that are explicitly requested or contain Rust-relevant
  bug fixes. Swift/AVFoundation/Metal-only PRs are automatically skipped.

## Upstream PR porting status

| PR   | Description                             | Status      | Rust Crate                                   |
| ---- | --------------------------------------- | ----------- | -------------------------------------------- |
| #8   | Colors + Effects via Metal              | DONE        | agent_contract (effects pipeline)            |
| #46  | Shape annotations + animation tools     | DONE        | core_model, agent_contract                   |
| #40  | Transcription language setting          | DONE        | core_model (Timeline.transcription_language) |
| #65  | Font weight in TextStyle                | DONE        | core_model — `TextStyle` on-disk format is now compatible with current Swift (9a3ae50). A `#[serde(from/into)]` `TextStyleWire` bridge reads EITHER `fontWeight` (Rust) OR `isBold` (Swift, →700/400), reads `isItalic` into a new `TextStyle.is_italic` field, and on save writes BOTH `fontWeight` + `isBold`/`isItalic` — so a `.palmier` written by either app round-trips bold/italic. `fontWeight` wins when both present (richer). The FCPXML title `fontFace` now reflects bold AND italic. 3 compat tests (Swift keys, both-keys precedence, dual-write round-trip) + the existing full-project round-trip. |
| #92  | Words-per-caption setting               | DONE        | search_core (CaptionConfig)                  |
| #105 | .aifc/.flac import support              | DONE        | core_model (ClipType::from_extension)        |
| #114 | Fix set_clip_properties rotation        | DONE        | timeline_core                                |
| #115 | Fix writePosition keyframe corruption   | DONE        | timeline_core                                |
| #129 | Fix keyframe loss on speed change       | DONE        | timeline_core (keyframes.rs)                 |
| #136 | XMEML source timecode                   | DONE        | render_core (xml_export.rs), core_model      |
| #144 | Validate speed/volume/opacity/trim      | DONE        | agent_contract (mutation.rs)                 |
| #94  | Export resolutions (2K, Match Timeline) | DONE        | render_core (ExportResolution)               |
| #135 | Missing-media cache pattern             | DONE        | core_model, agent_contract                   |
| #224 | Open project with corrupt media.json    | DONE        | project_io (degrade to empty manifest)       |
| #236 | Symmetric trim model for add/insert     | DONE        | agent_contract (resolve_placement)           |
| #233 | add_clips keeps project fps fixed        | DONE        | agent_contract (source-fps warning)          |
| #243 | Default agent model → Sonnet 5           | DONE        | app_contract, app_shell_gpui (chat list)     |
| #189 | Caption phrase timing from word stamps   | DONE (already) | search_core (phrases_from_words)          |
| #177 | set_project_settings tool + presets      | DONE        | agent_contract (set_project_settings + first-clip auto-detect on add_clips & apply_layout place-new) |
| #186 | split_clip → split_clips batch           | DONE        | agent_contract (two modes, dedup, A/V)       |
| #193 | FCPXML export v1 baseline                | DONE (v1)   | render_core (fcpxml_export.rs) + ExportMode::Fcpxml + write_interchange (save dialog writes .xml/.fcpxml); refinements #197/#254 pending (#214/#206/#247 done) |
| #247 | FCPXML source timecode + relink-by-filename | DONE     | render_core fcpxml_export.rs: start_timecode_frames — asset `start` = embedded timecode, asset-clip in-point += origin (round(tc_frame/quanta*fps)); asset/asset-clip `name` = on-disk filename so Resolve relinks |
| #204 | Window sizing (Home/Settings)           | DONE        | app_shell_gpui window.rs (project maximize-to-screen deferred) |
| #214 | FCPXML format naming + Rec.709           | DONE        | render_core fcpxml_export.rs (NTSC-aware naming) |
| #206 | FCPXML per-asset formats + A/V collapse  | DONE        | render_core fcpxml_export.rs: per-asset formats keep source resolution but frameDuration on the PROJECT grid (asset duration + asset-clip in-point align, no FCP conform-snap); each asset-clip references its own format (audio omits it); synced A/V pairs (same source/timing/trim/speed in one link group) collapse into the video asset-clip, dropping the redundant audio partner (`redundant_audio_clip_ids`). Asset-clips now also carry (Resolve target, #254): `<adjust-crop>` + `<adjust-conform type="fit">` + `<adjust-transform>` (scale/rotation/position/flip, fit-compensated, static + keyframed via `write_kf_param`+`resolved_transform_at`) + `<adjust-blend>` (opacity, static + keyframed) + `<adjust-volume>` (dB), emitted only when non-default (default clip stays self-closing); hidden/muted tracks → `enabled="0"`. Text overlays emit `<title>` (titleBasic effect + text-style-def + transform; `write_title`). Retimed clips (speed≠1) emit `<timeMap>` + retimed-axis `start`/keyframe times (#197). Pending: FCP-target value encoding (host/UI dropdown; Resolve is Swift's default). |
| #226 | apply_layout geometry + placement + tool   | DONE        | core_model video_layout.rs + agent_contract cmd_apply_layout: re-layout mode (batch clipIds, same-track overlap + coincidence checks) AND place-new mode (mediaRef → stacked video track per slot by z-order, clip creation, settings auto-detect) |
| #227 | Master audio: sync-locked follower cut (not just shifted) | DONE | timeline_core workflow.rs compute_ripple_delete adds every sync-locked follower to cleared_track_indices → executor clears the range + shifts; the old shift-refuse loop was dead code (verified by 200k-timeline fuzz) |
| #207 | ripple_delete_ranges per-call sync-lock exemption | DONE | timeline_core RippleDeleteConfig.ignore_sync_lock_track_indices + compute skips ignored followers; executor shifts only cleared tracks so ignored sync-locked tracks stay in place; tool arg `ignoreSyncLockTrackIndices` (index list); 2 tests |
| #160 #245 | remove_words tool (word→frame + ripple linked partners) + matches | DONE (pure) | timeline_core::word_cut (WordCutPlanner/cut_ranges, span_frames, CutAggressiveness, plan_word_removal — single primary track, refuse unlinked multi-track); agent_contract cmd_remove_words + schema + parse_word_spans/matches (index-clamp, words XOR matches); shared apply_ripple_delete_on_track. 26 tests. Transcriber host-deferred — set_timeline_words seam (empty → "No transcribable speech", same boundary as get_transcript/#178) |
| #242 | create_matte tool (solid-colour image) | DONE (end-to-end) | core_model::matte MatteAspect (7 presets + even/fit sizing); agent_contract cmd_create_matte + MatteWriter host seam (registers a ClipType::Image asset, no ClipType::Matte); app_shell::matte_writer::ProjectMatteWriter renders the PNG (`image` crate) into the project media/ dir, wired on open/save-as. 12 tests |
| #199 | Skills: store + read_skill tool + prompt | PARTIAL     | skill_store.rs + agent_contract read_skill/set_skills/prompt inject + boot load; catalog/UI pending |
| #225 | Text animation (data model + agent args) | PARTIAL     | core_model text_animation.rs (WordTiming/TextAnimation/11-preset enum) + Clip.text_animation/word_timings serde + timeline_core rescale_word_timings + agent add_texts animation/highlightColor args; renderer (TextAnimator/TextFrameRenderer) UI-deferred |
| #74  | naturalTimeScale for clip inserts       | DEFERRED    | AVFoundation-specific                        |
| #119 | Audio syncing multiple tracks           | DONE (v1)   | agent_contract sync_audio_clips (spec 12-audio-sync.md; correlator + #174 seam; offset baked into start_frame, newClipId reported; UI + sync_offset_frames metadata deferred) |
| #251 | Audio Enhancer / Denoise                | DONE (contract) | agent_contract denoise_audio (mirrors Swift setDenoise merge semantics; denoise = `audio.denoise` effect in the existing stack, NO model change; replaced the never-shipped set_clip_noise_reduction/set_clip_audio_effects stubs). DeepFilterNet3 bake + preview/export substitution = host, deferred |
| #255 | Multiple timelines per project          | PARTIAL     | DONE: ProjectFile on-disk contract (decode fallback, id/name/folderId, displayHeight, viewStates round-trip, sibling-preserving saves); nesting realigned to Swift's sequence-carrier representation (recursive compositor render, audio/export flatten, executor sibling store, app wiring); create/set_active/duplicate_timeline tools (59→62) + get_timeline timelines list + prompt paragraph; add_clips timelineId nesting (linked A/V carriers, empty+cycle rejection). insert_clips nesting (linked A/V carrier via link_audio_for_placed_clips sourceClipType carry-through); rename/delete_media accept timelineIds (last-timeline guard, delete-active switches). PENDING: export timelineId arg (no Rust export_project tool yet), native nested-sequence XML/FCPXML emission (v1 flattens), timeline tab UI |
| #133 | Project thumbnail main-thread hang      | DEFERRED    | Swift-specific pattern                       |

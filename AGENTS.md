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
- `render_core::compositor` is the pure CPU frame compositor: `compose_frame` flattens a timeline frame into RGBA — layer order, per-clip transform/crop/opacity, rotation, flip (`flip_image`), all 16 blend modes via `blend_rgb` (weighted by backdrop alpha per W3C: `(1-dst_a)*src + dst_a*blend`), fades (`timeline_core::fade_multiplier_at`), keyframe-resolved via `timeline_core::resolved_*_at`, bilinear sampling. Also composites shape annotations (`rasterize_shape`: rect/oval/circle fill+stroke; arrow/line via `rasterize_line_or_arrow` — #45; endpoints normalized 0..1 of the box [start top-left, end bottom-right], no endpoints → horizontal centre line/arrow, Arrow adds barbs scaled to the stroke width; Rust-native, no Swift rasterizer to mirror — documented assumption), chroma key (`apply_chroma_key`: soft hue-based key mirroring Swift's `Metal/ChromaKey.metal` — `k = hueCloseness(soft) * satGate * chromaGate`, alpha `*= 1-k` for feathered edges, luma-mix spill; `ChromaKey.softness` field + `dd`/chroma gate keep dark/near-grey pixels per upstream #291; NOT the old hard RGB-distance cutoff), colour adjustments (`apply_color_adjustments`: exposure/contrast/saturation/brightness), blur (`apply_blur`), and vignette (`apply_vignette`). Text overlays render via `render_core::text` (bundled fonts + pure-Rust `ab_glyph`): per-family font selection (Anton/Bebas/Marker/Shrikhand/…, plus the variable families Inter/Geist/GeistMono/DMSans/Caveat/PlayfairDisplay/SpaceGrotesk, else Poppins), Regular/Bold weight — with the wght variation axis applied on variable faces (`VariableFont::set_variation`, #65; no-op on static faces), multiline, L/C/R alignment, letter spacing, line height, drop shadow, caption background (padding + rounded corners), and outline/stroke — with `font_size` scaled to Swift's 1080-tall reference canvas. Text rotation and shadow blur are follow-ups. `render_sequence` drives per-frame compose for the exporter. Media decode is a `fetch_source` closure (platform adapter), fully unit-tested with synthetic sources.
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
- CHROMA KEY UI + EYEDROPPER (#291, change `chroma-inspector-eyedropper`): Rust had no chroma editing UI (chroma was AI-tool-only). `app_shell_gpui::chroma_controls` is the pure layer (rgb↔hue, `ChromaControls` read-clip/build `apply_effect key.chroma` args, `frame_uv_from_click` aspect-fit mapping; 6 tests). Inspector "Chroma Key" section (visual clips): On/Off, key swatch + Green/Blue presets + eyedropper, Tolerance/Softness/Spill scrub rows (threaded through the scrub machinery → `apply_effect`). Preview eyedropper: `chroma_sampling` arms a clip id, a bounds-capturing `canvas()` + `on_mouse_down` maps the click to a frame pixel (Fit/zoom=1), samples the composited PNG (`sample_png_pixel`), and applies `key.chroma` with the sampled hue. gpui interaction is human-verified (repo can't run gpui); compile + pure tests cover the rest.
- AUDIO METER (#293): `audio_core::audio_meter` is the pure state machine (Swift `AudioMeterChannelState`/`Hub` parity — dB, level/peak decay, peak-hold, clip latch; injected time; 8 tests). No live audio output engine exists, so it's a PLAYHEAD meter: `app_shell_gpui::audio_export::timeline_audio_envelope` (mono 0..1 peaks over the timeline, background-decoded + revision-cached like the preview PNG) is sampled at the playhead each render, ingested into `StereoMeter`, and drawn as L/R bars (peak tick + clip tint) in the preview transport (`render_audio_meter`). Live-output meter awaits an audio playback engine.
- `timeline_core::project_presets` holds the inspector/preview dropdown data (upstream #168): `ASPECT_PRESETS`, `FPS_PRESETS`, `QUALITY_PRESETS`, `ZOOM_PRESETS` with active-selection logic, mirroring Swift `PreviewContainerView` exactly.
- XML export (`render_core/src/xml_export.rs`) emits keyframed motion params (scale/rotation/center/crop) and a keyframed Opacity filter when the clip has the matching animation track (XML-012). Fades export as single-sided `<transitionitem>`s (Cross Dissolve for video, Cross Fade for audio) at the track level — the form Premiere reads — NOT `<fadein>/<fadeout>` tags (`write_fade_transition`).
- XML IMPORT (#154, a surpass-Swift item — neither side had import): `render_core::xml_import` reverses the exporters into an `ImportedTimeline {timeline, files, notes}` with a dependency-free depth-aware tag scanner (`xml_blocks`/`attr`/`first_text`; no XML crate). `parse_xmeml` (clip.media_ref = the `<file id>` = `{ref}-v`/`-a`) and `parse_fcpxml` (clip.media_ref = the asset NAME; project-scoped `<sequence>`, lane→track inversion mirroring `lane_of_track`, rational-time `parse_rational_seconds`, 1× trim = clip start − asset timecode origin) both work; `import_xml` dispatches XMEML/FCPXML, Premiere/Resolve stay `NotImplemented`. v1 skips nests/retimed/keyframes/titles with notes. Verified by export→parse round-trips. App wiring: `app_shell_gpui::timeline_import::import_timeline_from_xml` relinks each referenced file (match library by filename, else register the path via `import_media` so a missing file shows offline), remaps `clip.media_ref` keyed by BOTH file id AND filename (the two parsers differ), then `ToolExecutor::adopt_timeline` swaps it in as the new ACTIVE timeline (prior one becomes a sibling — import never overwrites open work; clears undo, bumps revision). Reachable via File → Import Timeline (⌘⇧I, distinct from ⌘I Import Media). No agent/MCP tool added — import is a UI action.
- Cross-platform: the `desktop-app` `fronda` bin compiles clean on Windows (gpui-ce + ffmpeg + reqwest), verified via `cargo check` (2026-07-03/04).
- `mcp_server::session` is the pure core of stateful MCP sessions (#250): `SessionStore` (LRU32 + 1h TTL, injected clock, monotonic `seq` for deterministic tie-break) + `parse_session_id` (skips colon-less header lines). Per-session executor wiring / SSE / tools/list_changed are deferred (basic single-shared-executor HTTP server already works).
- TEXT INPUT STACK (2026-07-05): `text_field::TextField` (single-line) + `text_area::TextArea` (multiline: shape_text/WrappedLine soft wrap, visual-line up/down with goal column, paste keeps newlines, content-driven height min..max lines) are the ONLY text-input paths - both are EntityInputHandler-based (real IME/CJK composition), key contexts contain the `input` marker, and `bind_text_field_keys` + `bind_text_area_keys` run at boot. Modifier-free global shortcuts (space/q/w/j/k/l/left/right/i/o/=/-/backspace/backtick + shift variants) live in `global_shortcuts.rs` as gpui actions bound with a `"!input"` context predicate - NEVER route them through raw key_down listeners (a listener can't tell typing from a shortcut); app_root's key listener skips typing-conflicting chords entirely. Known limits: no scroll past max_lines (paints over), no field-level undo, char (not grapheme) boundaries, interactive behavior verified by compile+pure tests only (no gpui interaction tests).
- TOOL SURFACE v2 (upstream #263 @141c69b, change `tool-surface-v2`, 2026-07-10): the agent/MCP surface is the v2 contract — **56 tools** = 48 upstream + 8 Rust extensions (duplicate_project, add_shapes, apply_animation, create/dissolve_compound_clip, save/apply/list_clip_presets); the multicam trio (manage_multicam/change_cam/get_multicam) landed via the multicam-engine change (53 → 56). Host split via `tools::tool_host()`: shared 51; `mcp_tools()` 55 (+ get_projects/open_project/new_project/close_project); `in_app_tools()` 52 (+ read_skill) — MCP server serves `mcp_tools()`, chat panel `in_app_tools()`. Every clip-mutation tool returns the C-4 **mutation envelope** (`agent_contract::envelope::build_envelope`): clips (v2 shape + track, cap 30 + clipsNote), captionGroups (fold ≥3), shifted rules (≥3 pure shifts per (track, delta)), removedClipIds, createdTracks, notes; tool extras merge top-level; organize_media returns a plain payload + re-read note when it switches the active timeline. **Short-id contract** (C-3) lives in `ToolExecutor::execute`: known id keys expand from ≥8-char prefixes (ambiguous → hard error), outputs shorten every known id over the pre∪post universe (`id_short.rs`). `get_timeline` v2 (C-5: frames [start,end), default-strip, keyframe collapse @0.0005, per-track gaps, A/V fold with deviation-only `audio` object, captionGroups summaries, window+captionDetail, 200-row cap) and `get_transcript` v2 (C-6: global word indices, per-clip [index,text,start] rows, segments granularity, 10000-word paging) are built on `agent_contract::timeline_v2`. `detect_beats` is NEW (pure `audio_core::beat_detector`, ClipAudioSource seam, per-mediaRef cache); `sync_clips` (renamed from sync_audio) adds mode auto|audio|timecode (timecode = manifest source-timecode fields, confidence 1.0, correlator sign convention). Absorbed & retired: create/rename/delete_folder, move_to_folder, rename_media, delete_media (→ organize_media), remove_tracks (→ manage_tracks), create_matte + import_folder (→ import_media source), duplicate_timeline (→ create_timeline.from), list_folders (→ get_media), set_blend_mode (→ set_clip_properties.blendMode), set_chroma_key (→ apply_effect key.chroma, mirrored into Clip.chroma_key), set_color_grade (→ apply_color), generate_music (→ generate_audio). ripple_delete_ranges takes [start,end] pairs + clipId mode + units + `ignoreSyncLockedTracks` (renamed). SYSTEM_INSTRUCTION = upstream Appendix-B serverInstructions verbatim (product name reads Fronda) + delimited `# Fronda extensions` section; MCP initialize ships `mcp_instructions()` (+ projectNavigation). Count assertions live in tools.rs, spec_tool_snapshots.rs, spec_mcp_contract.rs, mcp server.rs. Known follow-ups: `currentFrame` needs a host playhead wire (reports 0), search_media/inspect_media/inspect_timeline keep pre-v2 honest descriptions pending real semantic search/frame sampling, generation tools error honestly until the remote backend lands.
- `mcp_server::server` enforces Issue #122 bearer auth: `start()`/`spawn()` call `config.validate()` (a non-loopback bind without a token is rejected), and `handle_connection` rejects any request lacking a matching `Authorization: Bearer <token>` with 401 (constant-time compare) before executing. Default config is loopback + no token (auth not enforced locally).

## Upstream PR management

- **Merged upstream/main v0.6.1 → v0.6.5 on 2026-07-12** (upstream HEAD
  `f0f5b473`, merge commit brings the Swift baseline current; the Rust
  `crates/` workspace was untouched by the merge). A **Rust-porting re-audit
  is PENDING** for the commits `771b63e..f0f5b473`. New PRs to triage (not yet
  in the porting table below): **#294** elevenlabs voice isolator + dubbing
  (generation — likely Rust port), **#296** allow 65-point `.cube` LUTs
  (Rust-relevant if LUT support exists), **#288** validate video-to-audio span
  in the agent path (bug fix — check `agent_contract`), **#297** track tools
  called / **#290** analytics / **#292** hosted Sonnet 5 (telemetry/config —
  mostly skip or already covered by #243). Already ported before this merge:
  #293 audio meter, #291 chroma eyedropper, #283 multicam v2, #138 HDR export,
  #176 duplicate_clips, #284 aspect labels, #263 tool-surface v2, #274 beat
  detection. Full new-commit list: `git --no-pager log 771b63e..f0f5b473 --oneline`.
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
| #65  | Font weight in TextStyle                | DONE        | core_model — `TextStyle` on-disk format is now compatible with current Swift (9a3ae50). A `#[serde(from/into)]` `TextStyleWire` bridge reads EITHER `fontWeight` (Rust) OR `isBold` (Swift, →700/400), reads `isItalic` into a new `TextStyle.is_italic` field, and on save writes BOTH `fontWeight` + `isBold`/`isItalic` — so a `.palmier` written by either app round-trips bold/italic. `fontWeight` wins when both present (richer). The FCPXML title `fontFace` now reflects bold AND italic. 3 compat tests (Swift keys, both-keys precedence, dual-write round-trip) + the existing full-project round-trip. **wght renderer DONE 2026-07-10 (upstream-m-batch, task 2.3)**: `render_core::text` now applies `font_weight` as the `wght` variation axis (ab_glyph 0.2.32 `VariableFont::set_variation`; no-op on static faces), and the bundled variable families (Inter/Geist/GeistMono/DMSans/Caveat/PlayfairDisplay/SpaceGrotesk) are wired into `font_for` so the axis is reachable — matching Swift `BundledFonts` (registers every bundled face). 2 tests (variable-family mapping incl. geistmono>geist; sub-bold 100-vs-590 wght changes stroke coverage). |
| #92  | Words-per-caption setting               | DONE        | search_core (CaptionConfig)                  |
| #105 | .aifc/.flac import support              | DONE        | core_model (ClipType::from_extension)        |
| #114 | Fix set_clip_properties rotation        | DONE        | timeline_core                                |
| #115 | Fix writePosition keyframe corruption   | DONE        | timeline_core                                |
| #129 | Fix keyframe loss on speed change       | DONE        | timeline_core (keyframes.rs)                 |
| #136 | XMEML source timecode                   | DONE        | render_core (xml_export.rs), core_model      |
| #144 | Validate speed/volume/opacity/trim      | DONE (live) | agent_contract — mutation.rs validators WIRED into the live path: `ToolExecutor::validate_args` gates `execute()` before dispatch (23 tools incl. the #264 frame ceiling), e2e-tested through `executor.execute`. Unwired (shape diverges from the live executor): validate_split_clip (legacy singular), validate_ripple_delete_ranges (Swift clip-scoped contract), validate_import_folder — RESOLVED: executor aligned to `path`, validator wired. Inspector-boost conflict RESOLVED (option b): the volume ceiling is `mutation::volume_ceiling_linear()` = `timeline_core::linear_from_db(VOLUME_CEILING_DB)` (+15 dB ≈ 5.6234, Swift VolumeScale) — the tool layer accepts the whole UI-reachable range; beyond it still rejects. Schema text + e2e tests updated; inspector-shaped dB commits pinned by `inspector_shaped_volume_boost_roundtrip`. |
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
| v0.6.1 tool surface | Full ToolDefinitions name-diff + gap ports | DONE 2026-07-05 | sync_audio (renamed from sync_audio_clips, windowed correlation), denoise_audio, update_text, export_project (ExportHost seam), get_projects (ProjectLister), open_project/new_project (ProjectNavigator - executor self-swap avoids the hub-lock deadlock); speculative import_xml/create_project/delete_project/set_clip_* stubs removed. `send_feedback` resolved 2026-07-11 (change `feedback-github-link`): no backend is run — the app menu Send Feedback opens `agent_contract::FEEDBACK_ISSUES_URL` (Fronda GitHub issues) and the tool returns that URL as guidance when no `FeedbackSender` is installed (seam kept for hosts). 63 tools (superseded by #263 v2 below) |
| #263 | Tool surface v2 (48-tool merge) | DONE 2026-07-10 | agent_contract (envelope.rs, timeline_v2.rs, id_short.rs, tools.rs host split), audio_core (beat_detector), mcp_server, app_shell_gpui — 53 tools at the time (45 upstream + 8 extensions; multicam trio landed later via #283 → 56), mutation envelopes, short ids, get_timeline/get_transcript v2, organize_media/manage_tracks/close_project/detect_beats, 15 absorptions, sync_clips rename, SYSTEM_INSTRUCTION v2 (see the TOOL SURFACE v2 bullet above) |
| #74  | naturalTimeScale for clip inserts       | DEFERRED    | AVFoundation-specific                        |
| #119 | Audio syncing multiple tracks           | DONE (v1)   | agent_contract sync_audio (spec 12-audio-sync.md; correlator + #174 seam; offset baked into start_frame, newClipId reported; UI + sync_offset_frames metadata deferred) |
| #251 | Audio Enhancer / Denoise                | DONE (contract) | agent_contract denoise_audio (mirrors Swift setDenoise merge semantics; denoise = `audio.denoise` effect in the existing stack, NO model change; replaced the never-shipped set_clip_noise_reduction/set_clip_audio_effects stubs). DeepFilterNet3 bake + preview/export substitution = host, deferred |
| #255 | Multiple timelines per project          | PARTIAL     | DONE: ProjectFile on-disk contract (decode fallback, id/name/folderId, displayHeight, viewStates round-trip, sibling-preserving saves); nesting realigned to Swift's sequence-carrier representation (recursive compositor render, audio/export flatten, executor sibling store, app wiring); create/set_active/duplicate_timeline tools (59→62) + get_timeline timelines list + prompt paragraph; add_clips timelineId nesting (linked A/V carriers, empty+cycle rejection). insert_clips nesting (linked A/V carrier via link_audio_for_placed_clips sourceClipType carry-through); rename/delete_media accept timelineIds (last-timeline guard, delete-active switches). export_project tool WITH timelineId (ExportHost seam); native nested-sequence XMEML (<sequence id> inline-once/reference) + FCPXML (<media> compounds + ref-clips) emission per spec 13. timeline tab bar v1 (switch/create/delete via the shared tools) + inline rename (double-click the active tab, Enter commits via rename_media, Esc cancels; a real text_field::TextField with IME composition). #255 COMPLETE |
| #133 | Project thumbnail main-thread hang      | DEFERRED    | Swift-specific pattern                       |
| #283 | Multicam v2 (engine + 3 tools)          | DONE 2026-07-10 | change `multicam-engine`: core_model::multicam (typed MulticamSource — ProjectFile.multicamGroups Value→Vec, serde keys mirror Swift, lossless round-trip); timeline_core::multicam (MulticamEngine port: apply/rewrite/clamp-to-coverage/through-edit merge/overlay place+clear, group creation with hole-filling program spans, program rows, move/atomicity/manual-ripple violations, trim bounds, group offsets — MulticamEngineTests transplanted, 25 tests); agent_contract manage_multicam/change_cam/get_multicam per the tool-surface-v2 reserved slots (53 → 56; shared 51/MCP 55/in-app 52), executor multicam_groups store + id-universe entries + get_timeline multicamGroups + guards (move_clips whole-group-or-refuse + camera lane lock, manage_tracks remove/sync-unlock refusal, set_clip_properties timing refusal, sync_clips refusal, ripple atomicity) — MulticamToolTests transplanted (12 e2e). Hub/navigator seed + save groups (saved_multicam_groups prune). Deferred: audio-correlation sync maps (pinned/timecode/master only; correlator seam follow-up), multicam UI, undo of group METADATA (timeline undo works; stale metadata is save-pruned) |
| #211 | Project checkpoint autosave             | PORTED (change `upstream-m-batch`) | `app_shell_gpui::editor_state_hub`: pure `autosave_should_fire(has_root, cur_rev, last_rev)` decision (injected-counter testable) + `autosave_if_dirty()` (coalesced tick: saves only with a root AND revision advance; rapid edits collapse to one save) + `save_now()` (named wrapper for the Home-transition/timer) + `mark_saved_revision()` (baseline set on save/save_as/load_bundle). Mirrors Swift `VideoProject.scheduleProjectCheckpointAutosave` (no interval; coalesce to next tick). close_project's existing save is untouched. Follow-up (1 line, another slice's `app_root.rs`): call `hub.autosave_if_dirty()` on the revision-poll tick / `hub.save_now()` on show-Home. 6 tests. |
| #138 | 10-bit HDR video export (HEVC Main10)   | PORTED (change `upstream-m-batch`) | `app_shell_gpui::video_export`: `VideoCodec::H265Hdr` (HEVC, `YUV420P10LE`, `set_colorspace(BT2020NCL)` + direct-write `color_primaries=BT2020` / `color_trc=ARIB_STD_B67` HLG). HDR resolves to libx265 ONLY — a missing 10-bit HEVC encoder is an explicit error, NEVER a silent SDR/8-bit fallback. Threads end-to-end via the existing `VideoCodec` param through `audio_export::export_project_with_audio` (untouched). `export_model`: `hdr` field + `set_hdr` + `effective_video_codec` (H.265+hdr→H265Hdr); `export_view`: HDR toggle (H.265 only) drives codec selection. Structural test `hdr_export_is_10bit_bt2020_hlg_or_errors` asserts 10-bit + BT.2020 + HLG survive re-decode when libx265 is present, else the explicit-error path + no leftover file. NOTE: this dev/CI ffmpeg has no libx265, so the color-tag re-decode assertions are logic/compile-verified but not runtime-exercised here; the no-silent-fallback error path IS runtime-verified. 3 tests. |
| #176 | duplicate_clips agent tool              | PORTED (change `upstream-m-batch`) | Upstream PR #176 contract (description verbatim). `agent_contract::tool_exec::cmd_duplicate_clips` (single-line dispatch arm → `exec_enveloped` C-4 envelope): full-fidelity clone (fresh ids; keyframes/effects/fades/speed/opacity/volume/transform/crop preserved), linked partners auto-duplicated (`partner_moves_for_move_of` relative offset), the duplicated set re-links via a fresh link group (≥2 members share / lone member unlinks), destination overwrite via `clear_region`, multicam stamp dropped. `mutation::validate_duplicate_clips` wired into `validate_args` (non-empty entries, toFrame≥0 + frame ceiling, toTrack≥0). Short-id: `clipId` already a SCALAR_ID_KEY and `expand_input_ids` recurses into `entries`, so nested prefix-expansion + output shortening already cover it (test-confirmed; no id_short change). Tool count 56→57 across all 4 assertion files + host split (shared 52 / MCP 56 / in-app 53). 4 Swift tests transplanted + short-id + validator = 11 tests. |
| #284 | Aspect-ratio display labels             | PORTED (change `upstream-m-batch`) | `generation_core::model_catalog::aspect_ratio_display_label` (+ `aspect_ratio_display_token`) verbatim from Swift `ImageModelConfig.aspectRatioDisplayLabel`: colon-form ids pass through, underscore enum ids tokenize ("landscape_16_9"→"Landscape 16:9", "square_hd"→"Square HD"). Golden vectors transplanted from upstream PR #284's deleted `ImageModelConfigTests`. `tool_exec::cmd_list_models` emits `aspectRatioLabels` parallel to `aspectRatios` on Video/Image entries (additive, compatible). Generation panel picker wiring is a trivial follow-up (`generation_view.rs`, another slice). 3 tests. |

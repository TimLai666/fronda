# Upstream PR Audit — Rust Rewrite Applicability

Audit date: 2026-06-25
Upstream HEAD: `b9b4ad9` (palmier-io/palmier-pro main)

This file catalogs every upstream PR (from the Swift `palmier-pro` repo), its
current porting status in Fronda, and any action items.

## Legend

- **DONE** — Ported to Rust and verified with tests.
- **NOT STARTED** — Applicable to Rust, not yet ported.
- **N/A (Swift-specific)** — Relies on AVFoundation, Metal, AppKit, or other
  Apple-only APIs. No Rust equivalent needed.
- **N/A (Reverted)** — Was merged then reverted upstream; not applicable.
- **DEFERRED** — Applicable but blocked on larger infra or spec work first.

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

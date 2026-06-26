# Search, Transcription, Generation, and App Shell

Scope sources:

- `Sources/PalmierPro/Search/**`
- `Sources/PalmierPro/Transcription/**`
- `Sources/PalmierPro/Generation/**`
- `Sources/PalmierPro/Account/**`
- `Sources/PalmierPro/Settings/**`
- `Sources/PalmierPro/Help/**`
- `Sources/PalmierPro/App/**`
- `Sources/PalmierPro/Telemetry/**`
- `Tests/PalmierProTests/Search/**`
- `Tests/PalmierProTests/Captions/**`
- `Tests/PalmierProTests/Transcription/TranscriptSearchTests.swift`

## A. Search model lifecycle and indexing

- [x] `SRCH-001`: Media search remains enabled by default unless the user disables it.
- [x] `SRCH-002`: Opening a project attempts to prepare the visual model and then sweep current assets for indexing.
- [x] `SRCH-003`: Search enable/disable state persists across launches.
- [x] `SRCH-004`: Disabling search cancels in-flight indexing and unloads the model without deleting stored indexes.
- [x] `SRCH-005`: Re-enabling search prepares the model if installed and re-sweeps current assets.
- [x] `SRCH-006`: Removing the installed model resets coordinators, unloads the embedder, and deletes installed model files.
- [x] `SRCH-007`: Only video/image assets participate in visual indexing.
- [x] `SRCH-008`: Only audio assets or video assets with audio participate in transcript indexing.
- [x] `SRCH-009`: Assets currently generating are not scheduled for indexing.
- [x] `SRCH-010`: Index queues dedupe asset ids within a batch.
- [x] `SRCH-011`: Failed assets are not retried again within the same batch, but may be retried in a later sweep.
- [x] `SRCH-012`: Missing queued assets are treated as completed so a batch cannot stall forever.
- [x] `SRCH-013`: Search indexing pauses while any export is active.
- [x] `SRCH-014`: Export pause is refcounted, not boolean.
- [x] `SRCH-015`: Search index identity depends on path + modification time + file size.
- [x] `SRCH-016`: Still-image indexes contain exactly one embedding row at time zero.
- [x] `SRCH-017`: Short videos still receive at least one midpoint sample.
- [x] `SRCH-018`: Frame-sampler output times are strictly increasing with no duplicates.
- [x] `SRCH-019`: Scene starts are promoted from visual change detection, but long static spans still receive coverage-floor samples.
- [x] `SRCH-020`: Corrupt/undecodable videos yield a valid empty index rather than causing perpetual retry.
- [x] `SRCH-021`: Visual search is available only when the model is ready and the trimmed query is non-empty.
- [x] `SRCH-022`: Visual search keeps the best frame per shot before cross-asset ranking.
- [x] `SRCH-023`: Visual search sorts hits by descending score.
- [x] `SRCH-024`: Visual search applies the current absolute minimum score and relative-cutoff behavior.
- [x] `SRCH-025`: If the top score is non-positive, visual search returns no hits.
- [x] `SRCH-026`: Search UI keeps `Moments`, `Spoken`, and `Files` as separate result groups.
- [x] `SRCH-027`: Clearing the query clears visual and spoken results immediately.
- [x] `SRCH-028`: Search-result drags preserve current payload semantics:
  - still-image moment hit → plain asset drag
  - video/spoken hit → segmented `palmier-asset://<id>#<start>-<end>` drag

## B. Transcript cache and transcript search

- [x] `TRN-001`: Transcript cache identity depends on path + modification time + file size.
- [x] `TRN-002`: Only full-file transcripts are cached on disk.
- [x] `TRN-003`: Range-limited transcript requests reuse the full-file cache when available and otherwise transcribe only the requested range.
- [x] `TRN-004`: Range-limited transcript requests do not overwrite the canonical full-file cache with partial data.
- [x] `TRN-005`: Transcript range filtering keeps segments/words whose time spans overlap the requested range.
- [x] `TRN-006`: Boundary-straddling transcript segments remain included in filtered results.
- [x] `TRN-007`: Words without complete start/end timestamps are dropped from filtered results.
- [x] `TRN-008`: Filtered transcript text is rebuilt from surviving segments.
- [x] `TRN-009`: Transcript keyword search operates over cached-on-disk transcripts only.
- [x] `TRN-010`: Transcript keyword matching remains case-insensitive and diacritic-insensitive.
- [x] `TRN-011`: A transcript-search segment is a hit only if all query terms match that segment.

## C. Transcription and locale behavior

- [x] `TRN-012`: Locale matching prefers exact language+region matches first.
- [x] `TRN-013`: If no exact region exists, locale matching falls back to any supported locale with the same language.
- [x] `TRN-014`: Region override suffixes and Unicode extension tags such as `@rg=...` and `-u-rg-...` do not block language matching.
- [x] `TRN-015`: If no supported language matches, locale selection returns `nil`.
- [ ] `TRN-016`: Video transcription first extracts audio to a temp PCM `.caf` file using the current sample-rate/channel/bit-depth contract.
- [ ] `TRN-017`: If a video has no audio track, video transcription fails cleanly.
- [x] `TRN-018`: Range-limited transcription offsets timestamps back into original source time after extracting/transcribing the narrowed source span.
- [ ] `TRN-019`: If the on-device speech model must be installed and installation fails, transcription fails cleanly with a model-install error.

## D. Caption generation

- [x] `CAP-001`: Only clips with transcribable audio are valid caption sources.
- [x] `CAP-002`: Silent video is never selected as a caption source.
- [x] `CAP-003`: When linked audio/video clips represent the same source, caption generation targets the audio side rather than both sides.
- [x] `CAP-004`: Auto-detect captioning chooses the dominant spoken track by word count and captions only that track.
- [x] `CAP-005`: Caption generation reuses cached transcripts by default.
- [x] `CAP-006`: Caption generation bypasses transcript cache when profanity-censoring or explicit locale options would produce a different transcript.
- [x] `CAP-007`: Phrase splitting preserves the current sentence/clause/word-grouping heuristics.
- [x] `CAP-008`: Phrase timing remains distributed proportionally and respects the current minimum-display-duration behavior.
- [x] `CAP-009`: Caption phrase ownership requires meaningful overlap with a destination clip before assignment.
- [x] `CAP-010`: Generated captions are inserted on a fresh top video track.
- [x] `CAP-011`: If caption placement yields no clips, the inserted caption track is reverted.
- [x] `CAP-012`: Caption placement must not accidentally prune unrelated tracks.
- [x] `CAP-013`: Caption text case modes remain `auto`, `upper`, and `lower`.

## E. Generation and AI-edit workflow

- [x] `GEN-001`: AI generation is allowed only when account state says AI is allowed.
- [x] `GEN-002`: Available models come from the live model catalog and current settings filters.
- [x] `GEN-003`: Submit is blocked when estimated cost exceeds remaining credits.
- [x] `GEN-004`: Generation creates placeholder assets immediately before the backend job settles.
- [x] `GEN-005`: Placeholder assets are project-internal when a project is open and temp-based otherwise.
- [x] `GEN-006`: Image count requests are clamped to the current `1...4` behavior.
- [x] `GEN-007`: Reference upload order is preserved even though uploads happen concurrently.
- [x] `GEN-008`: Pre-uploaded URLs skip re-upload.
- [x] `GEN-009`: Upload cache is reused only for pristine asset bytes and not for trimmed/preprocessed variants.
- [x] `GEN-010`: Trimmed first-source video references are exported to temp media before upload.
- [x] `GEN-011`: Generation snapshots preserve prompt/model/duration/aspect ratio plus modality-specific options, reference URLs, reference asset ids, and `createdAt`.
- [x] `GEN-012`: Multi-image generation preserves requested placeholder count after clamping.
- [x] `GEN-013`: Backend submit returns a job id and the client subscribes to job updates.
- [x] `GEN-014`: If subscription cannot start, placeholders fail cleanly.
- [x] `GEN-015`: On success, result URLs are downloaded and placeholders are finalized into normal media assets.
- [x] `GEN-016`: If fewer result URLs arrive than placeholders, unmatched placeholders fail with current error semantics.
- [x] `GEN-017`: Download failure stores `pendingDownloadURL` and supports retry.
- [x] `GEN-018`: Upload/submit failure marks placeholders failed with the surfaced localized error.
- [x] `GEN-019`: For clip-replacement flows, only the first successful result may replace the original target clip.
- [x] `GEN-020`: Rerun reconstructs generation parameters from stored `GenerationInput`.
- [x] `GEN-021`: Rerun fails cleanly if the original model no longer exists or required stored inputs are missing.
- [x] `GEN-022`: Upscale/action availability preserves current rules by asset type, duration, and generating state.
- [x] `GEN-023`: Prompt mention tags and reference-slot rules preserve current model-driven generation-panel behavior.
- [x] `GEN-024`: Generated audio lands on audio tracks using the current auto-placement rules.

## F. Account, billing, and settings

- [x] `ACC-001`: Missing required backend config keys put account state into the current misconfigured state instead of crashing.
- [x] `ACC-002`: Remaining credits remain computed as `(monthly budget + purchased credits) - spent credits`, clamped at zero.
- [x] `ACC-003`: Top-off amount validation keeps the current minimum and maximum bounds.
- [x] `ACC-004`: Billing/checkout URLs remain host-whitelisted and reject untrusted destinations.
- [x] `SET-001`: The account pane is hidden when backend configuration is missing or invalid.
- [x] `SET-002`: Notifications preference persists across launches.
- [x] `SET-003`: Privacy/telemetry preference persists across launches.
- [x] `SET-004`: Telemetry privacy changes apply on next launch, not immediately for the current run.
- [x] `SET-005`: Disabled-model preferences persist and filter generation choices.
- [x] `SET-006`: Agent pane stores API keys in secure storage and masks all but the last 4 characters in UI.
- [x] `SET-007`: Storage pane actions preserve current semantics for clearing caches, embeddings, and installed search models.

## G. Help, feedback, and app shell behavior

- [x] `APP-001`: App startup still performs logging bootstrap, telemetry startup, bundled font registration, notifications configuration, MCP startup when enabled, updater initialization, and deferred account/model configuration after the first Home window is shown.
- [ ] `APP-002`: Reopening the app with no visible windows shows Home again.
- [x] `APP-003`: Feedback submission requires non-empty message and current maximum length validation.
- [x] `APP-004`: Feedback screenshot capture occurs before the feedback window becomes key, so the feedback window does not capture itself.
- [x] `APP-005`: Feedback screenshots are downscaled to the current maximum-dimension behavior.
- [ ] `APP-006`: Generation-complete notification clicks activate the app and reveal the generated asset in the best matching project.
- [x] `APP-007`: The “What’s New” surface appears only on a real version change and not on first install.
- [x] `APP-008`: Update badge visibility follows current update-available detection and local dismissal state.
- [x] `APP-009`: MCP instructions/help continue exposing server URL, copyable snippets, and install guidance for supported clients.

## H. Telemetry

- [x] `TEL-001`: Telemetry is enabled by default unless explicitly disabled.
- [x] `TEL-002`: Telemetry enabled state is latched for the current launch.
- [x] `TEL-003`: If DSN is empty or telemetry is disabled for the launch, telemetry startup is skipped cleanly.
- [x] `TEL-004`: Telemetry startup preserves current configuration semantics for environment, traces sample rate, app-hang timeout, and failed-request capture.
- [x] `TEL-005`: Notice-level log forwarding creates breadcrumbs rather than full error events.
- [x] `TEL-006`: Warning/error/fault log forwarding preserves current telemetry severity mapping.
- [x] `TEL-007`: Project-open telemetry context includes current project summary counts.
- [x] `TEL-008`: `Telemetry.trace` preserves current success/failure wrapping semantics.
- [x] `TEL-009`: Uncaught exceptions and fatal signals are also written to the local crash log path.

## Upstream change tracking

- `Upstream #7`: The search pipeline must support CLIP-style visual search: frame sampling → SIGLIP embedding → text tokenization → FAISS-style index → query. The search model lifecycle (SRCH-001–028) defines the behavior contract. For the Rust rewrite, the search/indexing implementation may use different on-device models or APIs, but must preserve the cache identity rules (path + mtime + file size), result grouping (Moments/Spoken/Files), and observable failure states.

- `Upstream #26`: The chat/agent system should implement conversation prefix caching for Anthropic API requests to reduce cost and latency. The Rust chat system should cache the system prompt + conversation prefix.

- `Upstream #27`: The import pipeline must support WebP still images. The image decoder in the generation/search pipeline must handle WebP format.

- `Upstream #34`: The generation pipeline must report clear errors for unprocessable media files during upload/reference workflows. `isMediaUnprocessable` must be checked before attempting upload.

- `Upstream #40`: The transcription system must support project-level spoken language setting with per-call language override. Locale matching must prefer exact language+region matches, fall back to same-language matches, and strip Unicode extension tags (TRN-012–015).

- `Upstream #51`: Agent tools must support transcription-based editing: `get_transcript` with `language` param, trim-by-transcript, and delete-by-transcript operations.

- `Upstream #61`: The generation pipeline should support HDR output for video generation when the model and user preferences allow HDR content. This affects the export/generation path, not the search pipeline.

- `Upstream #65`: Text rendering in the search/transcription/generation pipeline must respect variable font weight (`wght`) axis when rendering captions and text overlays.

- `Upstream #67`: The app shell must support project duplication through the Home screen context menu or agent tool.

- `Upstream #96`: The preview/composition pipeline must distinguish between offline media and unplayable media. Generation placeholders that resolve to unplayable output must be reported clearly rather than silently failing.

- `Upstream #81` / `Upstream #82`: The app startup sequence must handle slow or inaccessible project storage without hanging. The Rust app shell (APP-001) should use async startup with timeouts for project registry enumeration.

## Migration decisions to record explicitly

- `Decision:` Search/indexing and transcription are currently tightly coupled to the existing on-device model/toolchain. The Rust rewrite may swap implementations, but should preserve cache identity rules, result grouping, and observable failure states.
- `Decision:` Account, updater, notifications, and some help flows are currently macOS-biased. The Rust rewrite should preserve user-facing states and workflows while deciding which behaviors stay macOS-only and which become cross-platform abstractions.

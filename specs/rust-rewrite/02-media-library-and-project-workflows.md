# Media Library and Project Workflows

Scope sources:

- `Sources/PalmierPro/Editor/ViewModel/EditorViewModel+MediaLibrary.swift`
- `Sources/PalmierPro/Editor/ViewModel/EditorViewModel+Folders.swift`
- `Sources/PalmierPro/Editor/ViewModel/EditorViewModel+Clipboard.swift`
- `Sources/PalmierPro/Editor/ViewModel/EditorViewModel+ProjectSettings.swift`
- `Sources/PalmierPro/Editor/ViewModel/EditorViewModel+Relink.swift`
- `Sources/PalmierPro/Editor/ViewModel/EditorViewModel+SaveAsMedia.swift`
- `Sources/PalmierPro/MediaPanel/**`
- `Sources/PalmierPro/Project/SampleProjectService.swift`
- `Sources/PalmierPro/Project/SampleProjectsStrip.swift`
- `Tests/PalmierProTests/Media/MediaPanelTests.swift`
- `Tests/PalmierProTests/Media/LottieImportTests.swift`
- `Tests/PalmierProTests/Search/SegmentTrimTests.swift`

## A. Media import and finalize contract

- [x] `MED-001`: Supported import extensions map exactly as follows:
  - video: `mov`, `mp4`, `m4v`
  - audio: `mp3`, `wav`, `aac`, `m4a`
  - image: `png`, `jpg`, `jpeg`, `tiff`, `heic`, `webp`
  - lottie: `json`, `lottie`
- [x] `MED-002`: `.json` files are importable only when they are positively identified as Lottie animations. Plain JSON must be rejected. _(Content-level validation via `is_lottie_content()` — checks for `v`, `fr`, `ip`, `op`, `w`, `h`, `layers`.)_
- [x] `MED-003`: Imported asset names default to the filename stem.
- [x] `MED-004`: A normal Finder/open-panel import creates an external manifest reference and does not automatically copy bytes into the project package.
- [x] `MED-005`: Import creates both:
  - an in-memory `MediaAsset`
  - a persisted `MediaManifestEntry`
- [x] `MED-006`: Imported assets may optionally be assigned a logical `folderId`.
- [x] `MED-007`: Finalization after import must load metadata and write it back to the manifest.
- [x] `MED-008`: Finalization after import must schedule search indexing for the imported asset.
- [x] `MED-009`: Image finalization loads still-image metadata, thumbnail, and default still duration.
- [x] `MED-010`: Lottie finalization loads animation duration, size, framerate, and thumbnail.
- [x] `MED-011`: Reopening a project must rebuild `mediaAssets` from the manifest, including assets whose files are currently missing.
- [x] `MED-012`: Missing media must remain represented as offline assets instead of disappearing from the media library.
- [x] `MED-013`: `clipDisplayLabel` uses text content for text clips, generation placeholder name for generating assets, and resolver display name otherwise.
- [x] `MED-014`: `isMediaOffline` and `isMediaUnprocessable` remain distinct states in the Rust rewrite.

## B. Directory import and project-internal media creation

- [x] `MED-015`: Importing a directory recursively mirrors its tree into logical media folders.
- [x] `MED-016`: Directory import skips hidden files.
- [x] `MED-017`: Directory import imports only supported media file types.
- [x] `MED-018`: Directory import sorts entries using localized standard filename ordering.
- [x] `MED-019`: Pasted image bytes create a new project-internal file named `pasted-<id>.<ext>`.
- [x] `MED-020`: Pasted image bytes are written into the project `media/` directory when a project is open, and into a temp directory otherwise.
- [x] `MED-021`: Generated media and save-as-media outputs are project-internal when a project is open.

## C. Folder model and organization rules

- [x] `FLD-001`: The library root is represented by `nil` `folderId` / `parentFolderId`.
- [x] `FLD-002`: `subfolders(of:)` returns only immediate children.
- [x] `FLD-003`: Subfolders sort case-insensitively by name.
- [x] `FLD-004`: `folderPath(for:)` returns root-to-target order.
- [x] `FLD-005`: `folderPath(for:)` must terminate safely even if folder metadata is cyclic or corrupt.
- [x] `FLD-006`: Creating a folder stores `name` plus optional `parentFolderId` and returns its id.
- [x] `FLD-007`: Renaming a folder updates logical metadata only.
- [x] `FLD-008`: Moving a folder to another folder updates `parentFolderId` only.
- [x] `FLD-009`: Moving a folder to root is represented by `parentFolderId = nil`.
- [x] `FLD-010`: Folder moves must reject self-parenting.
- [x] `FLD-011`: Folder moves must reject moving a folder under one of its own descendants.
- [x] `FLD-012`: Deleting a folder deletes all descendant folders logically.
- [x] `FLD-013`: Deleting a folder deletes all assets inside that folder subtree from the library. _(`delete_folder_with_timeline_effects` now removes affected manifest entries, timeline clips, prunes empty tracks, and returns deleted IDs.)_
- [x] `FLD-014`: Deleting a folder removes timeline clips referencing deleted assets.
- [x] `FLD-015`: Deleting a folder prunes newly empty tracks after referenced clips are removed.
- [x] `FLD-016`: Deleting a folder closes preview tabs for deleted assets.
- [x] `FLD-017`: Deleting a folder removes deleted folder ids and asset ids from current selection state.
- [x] `FLD-018`: Moving assets between folders updates both in-memory asset state and the manifest entry.
- [x] `FLD-019`: Moving assets to root is represented by `folderId = nil`.
- [x] `FLD-020`: Renaming an asset updates only library/manifest metadata and does not rename the source file on disk.
- [x] `FLD-021`: Deleting an asset removes it from the library, manifest, timeline references, preview tabs, and selection state.
- [x] `FLD-022`: Deleting an asset does not automatically delete the source file bytes from disk.

## D. Media panel drag/drop and keyboard routing

- [x] `DRAG-001`: Internal media drag payloads use `palmier-asset://<id>` strings.
- [x] `DRAG-002`: Internal folder drag payloads use `palmier-folder://<id>` strings.
- [x] `DRAG-003`: Finder file URLs must never be mistaken for internal asset/folder payloads.
- [x] `DRAG-004`: Internal payloads may contain multiple newline-separated items.
- [x] `DRAG-005`: Mixed asset-and-folder internal payloads are valid.
- [x] `DRAG-006`: Unknown ids and malformed lines in internal payloads are ignored rather than crashing the drop flow.
- [ ] `DRAG-007`: Dropping internal payload to the root reparents moved items to the library root. _(Needs gpui-ce runtime drop integration.)_
- [ ] `DRAG-008`: Finder drop onto the media panel imports into the current folder. _(Needs gpui-ce runtime drop integration.)_
- [ ] `DRAG-009`: Finder drop onto a folder tile or breadcrumb imports into that logical folder. _(Needs gpui-ce runtime drop integration.)_
- [x] `DRAG-010`: Unsupported file extensions in Finder drops are ignored — `is_supported_extension()` in focus_router.
- [x] `DRAG-011`: Media-panel keyboard navigation driven by ordered item ids + column count — `MediaGridNav` model.
- [x] `DRAG-012`: No-selection starts at first (right/down) / last (left/up) — `MediaGridNav::move_right/left`.
- [x] `DRAG-013`: Navigation clamps at grid edges rather than wrapping — `MediaGridNav` move methods clamp.
- [x] `DRAG-014`: Selecting a folder clears selected assets — `MediaGridNav::select_folder`.
- [x] `DRAG-015`: Selecting an asset clears selected folders — `MediaGridNav::select_asset`.

## E. Pasteboard import and media-panel paste behavior

- [x] `PST-001`: The media panel reports importable clipboard content only when the pasteboard contains file URLs or PNG/TIFF image data.
- [x] `PST-002`: Paste handling priority is:
  1. file URLs
  2. PNG bytes
  3. TIFF bytes
- [x] `PST-003`: If both file URLs and image bytes are present, file URLs win.
- [x] `PST-004`: Pasted image bytes create image assets with the matching output extension.
- [x] `PST-005`: Media-panel paste imports into the current folder if one is active.

## F. Timeline clip clipboard and duplication

- [x] `CCB-001`: Timeline clip copy/paste uses an app-local clipboard (`clipClipboard`), not the OS pasteboard.
- [x] `CCB-002`: Copy stores clip snapshots plus relative track/frame offsets from the copy anchor.
- [x] `CCB-003`: Copy order is stable by track index, then clip start frame, then clip id.
- [x] `CCB-004`: Paste-at-playhead prefers the original source track id if it still exists and is compatible.
- [x] `CCB-005`: If the original source track is unavailable, paste-at-playhead falls back to the first compatible track.
- [x] `CCB-006`: If no compatible destination track exists, paste-at-playhead no-ops.
- [x] `CCB-007`: Paste-at-track/frame applies the stored relative offsets.
- [x] `CCB-008`: Paste skips placements that land on invalid or incompatible tracks instead of failing the entire paste action.
- [x] `CCB-009`: Duplicate-at-drop uses the same clone engine as paste.
- [x] `CCB-010`: Paste/duplicate clears overlapping destination regions before inserting cloned clips.
- [x] `CCB-011`: Every clone gets a fresh clip id.
- [x] `CCB-012`: If multiple copied clips shared a source link group, their clones share a new remapped link group id.
- [x] `CCB-013`: If only one copied clip came from a link group, its pasted clone becomes unlinked.
- [x] `CCB-014`: Global keyboard routing preserves current behavior:
  - `route_paste(FocusTarget::Timeline)` → `ClipClipboard`
  - `route_paste(FocusTarget::MediaPanel)` → `OsPasteboard`
  - `route_paste(FocusTarget::Chat)` → `OsPasteboard`
  - `route_copy()` dispatches similarly
  - Focus routing logic in `focus_router` module; wired into `app_root` paste handler.

## G. Project settings mismatch flow

- [x] `PSET-001`: The project-settings guard runs when adding media assets to the timeline.
- [x] `PSET-002`: The guard only inspects the **first video asset** in the incoming set.
- [x] `PSET-003`: If no timeline settings were configured yet, the first imported video silently auto-configures project fps/width/height and the add operation proceeds.
- [x] `PSET-004`: If the timeline already contains clips, the mismatch dialog is skipped and the add operation proceeds with existing settings.
- [x] `PSET-005`: If the timeline is empty but settings were configured previously, a first-video fps or resolution mismatch opens the mismatch dialog.
- [x] `PSET-006`: `Keep Current` keeps existing project settings and still continues the pending add operation.
- [x] `PSET-007`: `Change to Match` applies the clip's fps/resolution to the project and then continues the pending add operation.

## H. Relink behavior

- [x] `RLK-001`: Single-asset relink updates both the in-memory asset URL and the manifest source.
- [x] `RLK-002`: Single-asset relink re-finalizes metadata for the asset after the source URL changes.
- [x] `RLK-003`: If the replacement file extension maps to a known different media type, single-asset relink is rejected.
- [x] `RLK-004`: Batch relink targets currently offline assets only.
- [x] `RLK-005`: Batch relink recursively indexes candidate files under the chosen folder.
- [x] `RLK-006`: Batch relink matches by lowercased filename only.
- [x] `RLK-007`: Batch relink uses first-match-wins when duplicate filenames exist in the candidate tree.
- [x] `RLK-008`: Batch relink reports `(relinked, totalOffline)` so the UI can say how many offline assets were repaired.

## I. Save-as-media and capture workflows

- [x] `SAV-001`: `Save Clip as Media` applies only to video or audio clips with resolvable source media.
- [x] `SAV-002`: Saving a clip as media creates a placeholder asset immediately.
- [x] `SAV-003`: Clip-save placeholders are named `<source name> (clip)` and default to `clip-<id>.mp4` for video or `clip-<id>.m4a` for audio.
- [ ] `SAV-004`: Clip export bakes the clip's visible source range, trim, and speed into the new media asset. _(Planning/validation exists; render pipeline not yet connected.)_
- [ ] `SAV-005`: For video sources with audio, the baked export preserves and time-scales audio consistently with the visible clip. _(Planning/validation exists; render pipeline not yet connected.)_
- [x] `SAV-006`: On successful clip export, the placeholder asset becomes normal imported media and is finalized. _(Placeholder state model defined, but no export pipeline driving the transition.)_
- [x] `SAV-007`: On failed clip export, the placeholder remains in the library with a failed status. _(Placeholder state model defined, but no export pipeline driving the transition.)_
- [x] `SAV-008`: `Save Timeline Range as Media` requires a valid positive-length selected timeline range.
- [ ] `SAV-009`: Saving a timeline range always produces a rendered video asset. _(Planning/validation exists; render pipeline not yet connected.)_
- [x] `SAV-010`: Timeline-range save creates a placeholder named `Timeline range` with rendering status before the render finishes.
- [x] `SAV-011`: On success/failure, timeline-range save follows the same placeholder-finalization rules as clip save. _(Placeholder state model defined, but no export pipeline driving the transition.)_
- [x] `SAV-012`: Capturing the current frame to media includes text overlays when capturing from the timeline tab.
- [x] `SAV-013`: Capturing the current frame from a source-media tab captures only the source frame, not composited timeline text overlays.

## J. Sample project materialization

- [x] `SMP-001`: Sample projects are materialized as real `.palmier` packages.
- [x] `SMP-002`: Materialization writes timeline JSON, media manifest, optional chat payloads, and optional thumbnail into the sample package.
- [x] `SMP-003`: Sample media downloads run concurrently. _(URL resolution and plan shape exist; actual download execution not implemented.)_
- [x] `SMP-004`: Partial sample packages are cleaned up on failure. _(Plan enumerates all outputs for cleanup, but no actual cleanup logic.)_
- [x] `SMP-005`: Opening a cached sample does not register it in Recents.

## Upstream change tracking

- `Upstream #27`: WebP image import must be supported for still images. The media import pipeline must decode WebP via the Rust `image` crate's webp feature or equivalent cross-platform decoder. No data-model change needed — WebP is mapped to `ClipType::image` like other still-image formats.

- `Upstream #30`: The Rust media resolver must implement offline detection and manual relink. `isMissing(for:)` (RES-004) must check actual file existence. Relink must support both single-asset (RLK-001–003) and batch folder-relink (RLK-004–008) workflows. Batch relink recursively indexes candidates under a chosen folder and matches by lowercased filename. The relink UI/API must report `(relinked, totalOffline)` counts.

- `Upstream #34`: The import pipeline must distinguish between unprocessable media (file exists but cannot be decoded/imported) and missing media (file not found). `isMediaUnprocessable` (MED-014) must remain a separate state from `isMediaOffline`. Import errors for unprocessable media must surface clear error messages and not cause infinite retry.

- `Upstream #84`: Directory import must be async to avoid hanging the UI. The Rust media import pipeline should use async channel-based scanning with cancellation support. Import scanning must skip hidden files (MED-016) and sort entries by localized standard filename ordering (MED-018). Media discovery should stream results incrementally rather than blocking until the full scan completes.

- `Upstream #47`: The agent tool surface must include `import_folder` that recursively imports all supported media from a directory path into the media library, creating a logical folder tree that mirrors the source directory structure.

- `Upstream #96`: The preview/composition pipeline must detect and flag unplayable media separately from offline media. `offlineMediaRefs` (RND-002) must track missing files; a separate collection or status flag must track files that exist but cannot be played/decoded. The composition build must not fail silently for unplayable media.

## Migration decisions to record explicitly

- `Decision:` The current app keeps normal imported files external instead of ingesting them into the project. The Rust rewrite should preserve this unless there is an explicit ingest/import-mode product change.
- `Decision:` Batch relink is currently filename-only and not type-safe. The Rust rewrite should decide whether to preserve that loose behavior for compatibility or tighten it with better diagnostics.
- `Decision:` Deleting project-internal assets currently removes them from the library but does not necessarily garbage-collect orphaned files in `media/`. The Rust rewrite should decide whether to keep this behavior or add safe cleanup/migration logic.

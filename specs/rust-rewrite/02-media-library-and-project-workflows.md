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

- [ ] `MED-001`: Supported import extensions map exactly as follows:
  - video: `mov`, `mp4`, `m4v`
  - audio: `mp3`, `wav`, `aac`, `m4a`
  - image: `png`, `jpg`, `jpeg`, `tiff`, `heic`, `webp`
  - lottie: `json`, `lottie`
- [ ] `MED-002`: `.json` files are importable only when they are positively identified as Lottie animations. Plain JSON must be rejected.
- [ ] `MED-003`: Imported asset names default to the filename stem.
- [ ] `MED-004`: A normal Finder/open-panel import creates an external manifest reference and does not automatically copy bytes into the project package.
- [ ] `MED-005`: Import creates both:
  - an in-memory `MediaAsset`
  - a persisted `MediaManifestEntry`
- [ ] `MED-006`: Imported assets may optionally be assigned a logical `folderId`.
- [ ] `MED-007`: Finalization after import must load metadata and write it back to the manifest.
- [ ] `MED-008`: Finalization after import must schedule search indexing for the imported asset.
- [ ] `MED-009`: Image finalization loads still-image metadata, thumbnail, and default still duration.
- [ ] `MED-010`: Lottie finalization loads animation duration, size, framerate, and thumbnail.
- [ ] `MED-011`: Reopening a project must rebuild `mediaAssets` from the manifest, including assets whose files are currently missing.
- [ ] `MED-012`: Missing media must remain represented as offline assets instead of disappearing from the media library.
- [ ] `MED-013`: `clipDisplayLabel` uses text content for text clips, generation placeholder name for generating assets, and resolver display name otherwise.
- [ ] `MED-014`: `isMediaOffline` and `isMediaUnprocessable` remain distinct states in the Rust rewrite.

## B. Directory import and project-internal media creation

- [ ] `MED-015`: Importing a directory recursively mirrors its tree into logical media folders.
- [ ] `MED-016`: Directory import skips hidden files.
- [ ] `MED-017`: Directory import imports only supported media file types.
- [ ] `MED-018`: Directory import sorts entries using localized standard filename ordering.
- [ ] `MED-019`: Pasted image bytes create a new project-internal file named `pasted-<id>.<ext>`.
- [ ] `MED-020`: Pasted image bytes are written into the project `media/` directory when a project is open, and into a temp directory otherwise.
- [ ] `MED-021`: Generated media and save-as-media outputs are project-internal when a project is open.

## C. Folder model and organization rules

- [ ] `FLD-001`: The library root is represented by `nil` `folderId` / `parentFolderId`.
- [ ] `FLD-002`: `subfolders(of:)` returns only immediate children.
- [ ] `FLD-003`: Subfolders sort case-insensitively by name.
- [ ] `FLD-004`: `folderPath(for:)` returns root-to-target order.
- [ ] `FLD-005`: `folderPath(for:)` must terminate safely even if folder metadata is cyclic or corrupt.
- [ ] `FLD-006`: Creating a folder stores `name` plus optional `parentFolderId` and returns its id.
- [ ] `FLD-007`: Renaming a folder updates logical metadata only.
- [ ] `FLD-008`: Moving a folder to another folder updates `parentFolderId` only.
- [ ] `FLD-009`: Moving a folder to root is represented by `parentFolderId = nil`.
- [ ] `FLD-010`: Folder moves must reject self-parenting.
- [ ] `FLD-011`: Folder moves must reject moving a folder under one of its own descendants.
- [ ] `FLD-012`: Deleting a folder deletes all descendant folders logically.
- [ ] `FLD-013`: Deleting a folder deletes all assets inside that folder subtree from the library.
- [ ] `FLD-014`: Deleting a folder removes timeline clips referencing deleted assets.
- [ ] `FLD-015`: Deleting a folder prunes newly empty tracks after referenced clips are removed.
- [ ] `FLD-016`: Deleting a folder closes preview tabs for deleted assets.
- [ ] `FLD-017`: Deleting a folder removes deleted folder ids and asset ids from current selection state.
- [ ] `FLD-018`: Moving assets between folders updates both in-memory asset state and the manifest entry.
- [ ] `FLD-019`: Moving assets to root is represented by `folderId = nil`.
- [ ] `FLD-020`: Renaming an asset updates only library/manifest metadata and does not rename the source file on disk.
- [ ] `FLD-021`: Deleting an asset removes it from the library, manifest, timeline references, preview tabs, and selection state.
- [ ] `FLD-022`: Deleting an asset does not automatically delete the source file bytes from disk.

## D. Media panel drag/drop and keyboard routing

- [ ] `DRAG-001`: Internal media drag payloads use `palmier-asset://<id>` strings.
- [ ] `DRAG-002`: Internal folder drag payloads use `palmier-folder://<id>` strings.
- [ ] `DRAG-003`: Finder file URLs must never be mistaken for internal asset/folder payloads.
- [ ] `DRAG-004`: Internal payloads may contain multiple newline-separated items.
- [ ] `DRAG-005`: Mixed asset-and-folder internal payloads are valid.
- [ ] `DRAG-006`: Unknown ids and malformed lines in internal payloads are ignored rather than crashing the drop flow.
- [ ] `DRAG-007`: Dropping internal payload to the root reparents moved items to the library root.
- [ ] `DRAG-008`: Finder drop onto the media panel imports into the current folder.
- [ ] `DRAG-009`: Finder drop onto a folder tile or breadcrumb imports into that logical folder.
- [ ] `DRAG-010`: Unsupported file extensions in Finder drops are ignored.
- [ ] `DRAG-011`: Media-panel keyboard navigation is driven by the currently ordered item ids plus current column count.
- [ ] `DRAG-012`: With no active selection, right/down starts at the first item and left/up starts at the last item.
- [ ] `DRAG-013`: Navigation clamps at grid edges rather than wrapping.
- [ ] `DRAG-014`: Selecting a folder clears selected assets.
- [ ] `DRAG-015`: Selecting an asset clears selected folders.

## E. Pasteboard import and media-panel paste behavior

- [ ] `PST-001`: The media panel reports importable clipboard content only when the pasteboard contains file URLs or PNG/TIFF image data.
- [ ] `PST-002`: Paste handling priority is:
  1. file URLs
  2. PNG bytes
  3. TIFF bytes
- [ ] `PST-003`: If both file URLs and image bytes are present, file URLs win.
- [ ] `PST-004`: Pasted image bytes create image assets with the matching output extension.
- [ ] `PST-005`: Media-panel paste imports into the current folder if one is active.

## F. Timeline clip clipboard and duplication

- [ ] `CCB-001`: Timeline clip copy/paste uses an app-local clipboard (`clipClipboard`), not the OS pasteboard.
- [ ] `CCB-002`: Copy stores clip snapshots plus relative track/frame offsets from the copy anchor.
- [ ] `CCB-003`: Copy order is stable by track index, then clip start frame, then clip id.
- [ ] `CCB-004`: Paste-at-playhead prefers the original source track id if it still exists and is compatible.
- [ ] `CCB-005`: If the original source track is unavailable, paste-at-playhead falls back to the first compatible track.
- [ ] `CCB-006`: If no compatible destination track exists, paste-at-playhead no-ops.
- [ ] `CCB-007`: Paste-at-track/frame applies the stored relative offsets.
- [ ] `CCB-008`: Paste skips placements that land on invalid or incompatible tracks instead of failing the entire paste action.
- [ ] `CCB-009`: Duplicate-at-drop uses the same clone engine as paste.
- [ ] `CCB-010`: Paste/duplicate clears overlapping destination regions before inserting cloned clips.
- [ ] `CCB-011`: Every clone gets a fresh clip id.
- [ ] `CCB-012`: If multiple copied clips shared a source link group, their clones share a new remapped link group id.
- [ ] `CCB-013`: If only one copied clip came from a link group, its pasted clone becomes unlinked.
- [ ] `CCB-014`: Global keyboard routing preserves current behavior:
  - timeline-focused paste uses the clip clipboard
  - media-panel-focused paste imports from the OS pasteboard instead

## G. Project settings mismatch flow

- [ ] `PSET-001`: The project-settings guard runs when adding media assets to the timeline.
- [ ] `PSET-002`: The guard only inspects the **first video asset** in the incoming set.
- [ ] `PSET-003`: If no timeline settings were configured yet, the first imported video silently auto-configures project fps/width/height and the add operation proceeds.
- [ ] `PSET-004`: If the timeline already contains clips, the mismatch dialog is skipped and the add operation proceeds with existing settings.
- [ ] `PSET-005`: If the timeline is empty but settings were configured previously, a first-video fps or resolution mismatch opens the mismatch dialog.
- [ ] `PSET-006`: `Keep Current` keeps existing project settings and still continues the pending add operation.
- [ ] `PSET-007`: `Change to Match` applies the clip’s fps/resolution to the project and then continues the pending add operation.

## H. Relink behavior

- [ ] `RLK-001`: Single-asset relink updates both the in-memory asset URL and the manifest source.
- [ ] `RLK-002`: Single-asset relink re-finalizes metadata for the asset after the source URL changes.
- [ ] `RLK-003`: If the replacement file extension maps to a known different media type, single-asset relink is rejected.
- [ ] `RLK-004`: Batch relink targets currently offline assets only.
- [ ] `RLK-005`: Batch relink recursively indexes candidate files under the chosen folder.
- [ ] `RLK-006`: Batch relink matches by lowercased filename only.
- [ ] `RLK-007`: Batch relink uses first-match-wins when duplicate filenames exist in the candidate tree.
- [ ] `RLK-008`: Batch relink reports `(relinked, totalOffline)` so the UI can say how many offline assets were repaired.

## I. Save-as-media and capture workflows

- [ ] `SAV-001`: `Save Clip as Media` applies only to video or audio clips with resolvable source media.
- [ ] `SAV-002`: Saving a clip as media creates a placeholder asset immediately.
- [ ] `SAV-003`: Clip-save placeholders are named `<source name> (clip)` and default to `clip-<id>.mp4` for video or `clip-<id>.m4a` for audio.
- [ ] `SAV-004`: Clip export bakes the clip’s visible source range, trim, and speed into the new media asset.
- [ ] `SAV-005`: For video sources with audio, the baked export preserves and time-scales audio consistently with the visible clip.
- [ ] `SAV-006`: On successful clip export, the placeholder asset becomes normal imported media and is finalized.
- [ ] `SAV-007`: On failed clip export, the placeholder remains in the library with a failed status.
- [ ] `SAV-008`: `Save Timeline Range as Media` requires a valid positive-length selected timeline range.
- [ ] `SAV-009`: Saving a timeline range always produces a rendered video asset.
- [ ] `SAV-010`: Timeline-range save creates a placeholder named `Timeline range` with rendering status before the render finishes.
- [ ] `SAV-011`: On success/failure, timeline-range save follows the same placeholder-finalization rules as clip save.
- [ ] `SAV-012`: Capturing the current frame to media includes text overlays when capturing from the timeline tab.
- [ ] `SAV-013`: Capturing the current frame from a source-media tab captures only the source frame, not composited timeline text overlays.

## J. Sample project materialization

- [ ] `SMP-001`: Sample projects are materialized as real `.palmier` packages.
- [ ] `SMP-002`: Materialization writes timeline JSON, media manifest, optional chat payloads, and optional thumbnail into the sample package.
- [ ] `SMP-003`: Sample media downloads run concurrently.
- [ ] `SMP-004`: Partial sample packages are cleaned up on failure.
- [ ] `SMP-005`: Opening a cached sample does not register it in Recents.

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

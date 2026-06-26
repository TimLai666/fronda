# Foundation and Project Model

Scope sources:

- `Sources/PalmierPro/Utilities/Constants.swift`
- `Sources/PalmierPro/Project/VideoProject.swift`
- `Sources/PalmierPro/Project/ProjectRegistry.swift`
- `Sources/PalmierPro/Models/ClipType.swift`
- `Sources/PalmierPro/Models/MediaManifest.swift`
- `Sources/PalmierPro/Models/MediaResolver.swift`
- `Sources/PalmierPro/Editor/ViewModel/EditorViewModel+Cost.swift`
- `Tests/PalmierProTests/Media/ProjectRegistryTests.swift`
- `Tests/PalmierProTests/Media/ProjectRoundTripTests.swift`
- `Tests/PalmierProTests/Media/MediaResolverTests.swift`
- `Tests/PalmierProTests/Timeline/ClipMutationsTests.swift`

## A. Core type and compatibility rules

- [x] `CORE-001`: The canonical clip/media types are exactly `video`, `audio`, `image`, `text`, and `lottie`.
- [x] `CORE-002`: `video`, `image`, `text`, and `lottie` are all treated as **visual** clip types.
- [x] `CORE-003`: Track compatibility is strict: audio is compatible only with audio, while all visual types are mutually compatible.
- [x] `CORE-004`: Project time is frame-based. Timeline math and persistence use integer project frames, not seconds.
- [x] `CORE-005`: Any source-seconds-to-frame mapping in the rewrite must be computed against the **project fps**, not the source file's native fps.

## B. Project package contract

- [x] `PRJ-001`: Project files use the `.palmier` extension.
- [ ] `PRJ-002`: The document type identifier is `io.palmier.project`.
- [ ] `PRJ-003`: The default untitled project name is `Untitled Project`.
- [ ] `PRJ-004`: The default project storage root is `~/Documents/Palmier Pro`.
- [x] `PRJ-005`: A project package may contain the following well-known children:
  - `project.json`
  - `media.json`
  - `generation-log.json`
  - `thumbnail.jpg`
  - `media/`
  - `chat/`
- [x] `PRJ-006`: `project.json` is required when opening a project.
- [x] `PRJ-007`: Opening must fail if `project.json` is missing.
- [x] `PRJ-008`: Opening must fail if `project.json` exists but timeline decode fails.
- [x] `PRJ-009`: `media.json` is optional, but if present and invalid, project open must fail.
- [x] `PRJ-010`: `generation-log.json` is optional, and invalid generation-log decode must not prevent project open.
- [x] `PRJ-011`: On open, the editor state must restore timeline, media manifest, and generation log when present.
- [x] `PRJ-012`: If no generation log was persisted, the app must seed it from AI-generated assets already present in the project.
- [x] `PRJ-013`: Saving must persist the current timeline snapshot, manifest snapshot, generation log snapshot, thumbnail, non-empty chat sessions, and any existing internal `media/` directory.
- [ ] `PRJ-014`: Closing the active project returns the app back to the Home view.
- [ ] `PRJ-015`: Renaming or moving a project file must update the matching entry in the recent-project registry.

## C. Recent-project registry

- [x] `REC-001`: Recent projects are persisted in `~/Documents/Palmier Pro/project-registry.json`.
- [x] `REC-002`: Registry deduplication is based on the standardized file URL.
- [x] `REC-003`: Registering an already-known project updates `lastOpenedDate` but keeps the same entry id.
- [x] `REC-004`: Registering a new project creates a new UUID-backed entry with `createdDate` and `lastOpenedDate` set.
- [x] `REC-005`: Removing a recent project removes only the registry entry and does not delete the package from disk.
- [x] `REC-006`: Deleting a recent project attempts to move the package to Trash, then removes the registry entry only if that trash operation succeeds.
- [x] `REC-007`: If the package is already missing, deleting it from Recents still removes the registry entry.
- [x] `REC-008`: Updating a project URL replaces the stored URL for the matching entry and updates `lastOpenedDate`.
- [x] `REC-009`: `sortedEntries` are ordered by descending `lastOpenedDate`.
- [x] `REC-010`: `ProjectEntry.name` is derived from the package filename stem.
- [x] `REC-011`: `ProjectEntry.isAccessible` reflects whether the stored file path currently exists.
- [x] `REC-012`: Inaccessible recent projects remain visible in the Home UI, can be removed from Recents, and can still surface a delete action.

## D. Persistence schema and backward compatibility

### Media manifest

- [x] `FMT-001`: New manifests default to `version = 2`.
- [x] `FMT-002`: If `version` is absent while decoding, it must decode as `1`.
- [x] `FMT-003`: If `entries` is absent while decoding, it must decode as an empty array.
- [x] `FMT-004`: If `folders` is absent while decoding, it must decode as an empty array.

### Media source model

- [x] `FMT-005`: A media source is always one of:
  - `external(absolutePath)`
  - `project(relativePath)`
- [x] `FMT-006`: `GenerationInput` persists prompt/model/duration/aspect ratio plus optional modality-specific fields, reference URLs, reference asset ids, and `createdAt`.

### Timeline decode defaults

- [x] `FMT-007`: Missing track flags decode to:
  - `muted = false`
  - `hidden = false`
  - `syncLocked = true`
- [x] `FMT-008`: Missing clip fields must decode to the same defaults as the current Swift model, including default speed/volume/opacity, zero trims/fades, default transform/crop, and nil optional linkage/text fields.
- [x] `FMT-009`: Timeline round-trips must preserve clip timing, transform/crop, keyframes, text content/style, link groups, and track flags.

### Generation log

- [x] `FMT-010`: New generation logs default to `version = 1`.
- [x] `FMT-011`: A missing `GenerationLogEntry.id` decodes to a fresh UUID string.
- [x] `FMT-012`: Legacy `cost` dollar values migrate to `costCredits = ceil(cost * 100)`.
- [x] `FMT-013`: If neither `costCredits` nor legacy `cost` exists, `costCredits` remains `nil`.
- [x] `FMT-014`: `generationLogEntries` are sorted newest-first by `createdAt`, with deterministic fallback ordering when timestamps are absent.

## E. Media resolver contract

- [x] `RES-001`: `MediaResolver.entry(for:)` returns the live manifest entry for an asset id if one exists.
- [x] `RES-002`: `MediaResolver.expectedURL(for:)` reconstructs the file URL even if the file is currently missing.
- [x] `RES-003`: `MediaResolver.resolveURL(for:)` returns a URL only when the expected file currently exists on disk.
- [x] `RES-004`: `MediaResolver.isMissing(for:)` is true when the expected file does not exist, or when the manifest entry itself is missing.
- [x] `RES-005`: `MediaResolver.displayName(for:)` falls back to `Offline` when no manifest entry exists.
- [x] `RES-006`: Resolver reads must reflect live manifest changes immediately. Methods operate on the manifest reference directly — no stale cache.

## F. Project settings and fps/resolution retiming

- [x] `PCFG-001`: Timeline settings are `fps`, `width`, `height`, and `settingsConfigured`.
- [x] `PCFG-002`: When fps changes, the rewrite must rescale:
  - `currentFrame`
  - `sourcePlayheadFrame`
  - clip `startFrame`
  - clip `durationFrames`
  - `trimStartFrame`
  - `trimEndFrame`
  - keyframe frame positions
  - fade lengths
- [x] `PCFG-003`: FPS retiming must preserve same-track non-overlap after rounding.
- [x] `PCFG-004`: FPS retiming must collapse rounded keyframe collisions deterministically, matching the current last-value-wins behavior.
- [x] `PCFG-005`: When canvas size changes, clips still sitting on the old auto-fit transform must be re-fit to the new canvas.
- [x] `PCFG-006`: When canvas size changes, manually adjusted clips must keep their user-authored transform.
- [x] `PCFG-007`: Applying new project settings marks `settingsConfigured = true`.

## Migration decisions to record explicitly

- `Decision:` The current project storage root is macOS-specific (`~/Documents/Palmier Pro`). The Rust rewrite should decide whether this stays identical on macOS only, or becomes a per-platform app-data path with migration logic.
- `Decision:` `MediaAsset.toManifestEntry(projectURL:)` currently treats any path with a `projectURL.path` prefix as project-internal. The Rust rewrite should replace that with a stricter descendant check while preserving existing projects.
- `Decision:` The current schema must remain backward-compatible with existing `.palmier` files even if the Rust rewrite introduces a cleaner internal model.

## Upstream change tracking

These upstream PRs define behavior the Rust rewrite must eventually match. Spec entries below represent requirements that are not yet implemented in Rust.

- `Upstream #99`: The `Clip` data model must support optional `chromaKey: ChromaKey?`, `colorGrade: ColorGrade?`, and `blendMode: BlendMode` fields. A new `ClipType::Adjustment` variant is needed. The serialized `project.json` / `media.json` schemas must round-trip these fields when present, but remain backward-compatible with projects that lack them. See `03-timeline-editor-and-preview.md` for compositor-level requirements.

- `Upstream #62`: The `Timeline` model must support optional `lut: LUTRef?` and `primaries: PrimaryGrade?` fields for project-level color grading. The serialized project format (`.palmier` / `project.json`) must round-trip these fields. `LUTRef` includes `kind`, `lookID`, `cubeName`, `cubeDimension`, `cubeBase64` (inline base64-encoded .cube file), and `intensity`. `PrimaryGrade` includes `temperature`, `tint`, `exposure`, `contrast`, `saturation`, `vibrance`, `highlights`, `shadows` (all -100..100 range). `GradeCurve` stores `master`, `red`, `green`, `blue` curves as `[(x, y)]` points.

- `Upstream #46`: The `Clip` model should eventually support an optional `shapeStyle: ShapeStyle?` field and `ClipType::Shape` variant for shape annotations. `ShapeStyle` includes `kind`, `stroke`, `fill`, `cornerRadius`, `arrowhead`. This is deferred until the shape-annotation feature is explicitly planned for the rewrite.

- `Upstream #105`: The file-type allowlist for media import must include `.aifc` (AIFF-C) and `.flac` extensions for audio, matching `ClipType::audio` handling. Simple extension mapping, no data-model change.

- `Upstream #67`: The project model must support a `duplicate()` operation that creates a copy of the entire `.palmier` package at a new path, including timeline JSON, media manifest, generation log, chat sessions, and `media/` directory. The copy must be a deep clone with fresh UUIDs for the project package identity; clip/media/chat UUIDs within stay untouched for referential integrity. The duplicate entry must be registered in Recents.

- `Upstream #81` / `Upstream #82`: Project open/restore flow must handle edge cases where the project storage directory is slow to enumerate or where previously-opened projects reference paths that are no longer accessible. The project registry startup must not hang; inaccessible entries must be surfaced (REC-011) rather than blocking the scan. The Rust project-loading sequence should use async streaming for directory enumeration with a reasonable timeout.

- `Upstream #119`: The audio sync feature introduces an `AudioSyncCorrelator` that computes RMS-envelope-based cross-correlation between two audio clips to determine sync offset. The Rust data model should support an optional `syncOffsetFrames: Option<i64>` on linked audio clips to record the computed offset for downstream render alignment. The correlation math (RMS extraction, lag computation, peak detection) is pure data and should live as a testable Rust module in `timeline_core` or a new `audio_core` crate.

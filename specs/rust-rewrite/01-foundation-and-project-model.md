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

- [ ] `CORE-001`: The canonical clip/media types are exactly `video`, `audio`, `image`, `text`, and `lottie`.
- [ ] `CORE-002`: `video`, `image`, `text`, and `lottie` are all treated as **visual** clip types.
- [ ] `CORE-003`: Track compatibility is strict: audio is compatible only with audio, while all visual types are mutually compatible.
- [ ] `CORE-004`: Project time is frame-based. Timeline math and persistence use integer project frames, not seconds.
- [ ] `CORE-005`: Any source-seconds-to-frame mapping in the rewrite must be computed against the **project fps**, not the source file’s native fps.

## B. Project package contract

- [ ] `PRJ-001`: Project files use the `.palmier` extension.
- [ ] `PRJ-002`: The document type identifier is `io.palmier.project`.
- [ ] `PRJ-003`: The default untitled project name is `Untitled Project`.
- [ ] `PRJ-004`: The default project storage root is `~/Documents/Palmier Pro`.
- [ ] `PRJ-005`: A project package may contain the following well-known children:
  - `project.json`
  - `media.json`
  - `generation-log.json`
  - `thumbnail.jpg`
  - `media/`
  - `chat/`
- [ ] `PRJ-006`: `project.json` is required when opening a project.
- [ ] `PRJ-007`: Opening must fail if `project.json` is missing.
- [ ] `PRJ-008`: Opening must fail if `project.json` exists but timeline decode fails.
- [ ] `PRJ-009`: `media.json` is optional, but if present and invalid, project open must fail.
- [ ] `PRJ-010`: `generation-log.json` is optional, and invalid generation-log decode must not prevent project open.
- [ ] `PRJ-011`: On open, the editor state must restore timeline, media manifest, and generation log when present.
- [ ] `PRJ-012`: If no generation log was persisted, the app must seed it from AI-generated assets already present in the project.
- [ ] `PRJ-013`: Saving must persist the current timeline snapshot, manifest snapshot, generation log snapshot, thumbnail, non-empty chat sessions, and any existing internal `media/` directory.
- [ ] `PRJ-014`: Closing the active project returns the app back to the Home view.
- [ ] `PRJ-015`: Renaming or moving a project file must update the matching entry in the recent-project registry.

## C. Recent-project registry

- [ ] `REC-001`: Recent projects are persisted in `~/Documents/Palmier Pro/project-registry.json`.
- [ ] `REC-002`: Registry deduplication is based on the standardized file URL.
- [ ] `REC-003`: Registering an already-known project updates `lastOpenedDate` but keeps the same entry id.
- [ ] `REC-004`: Registering a new project creates a new UUID-backed entry with `createdDate` and `lastOpenedDate` set.
- [ ] `REC-005`: Removing a recent project removes only the registry entry and does not delete the package from disk.
- [ ] `REC-006`: Deleting a recent project attempts to move the package to Trash, then removes the registry entry only if that trash operation succeeds.
- [ ] `REC-007`: If the package is already missing, deleting it from Recents still removes the registry entry.
- [ ] `REC-008`: Updating a project URL replaces the stored URL for the matching entry and updates `lastOpenedDate`.
- [ ] `REC-009`: `sortedEntries` are ordered by descending `lastOpenedDate`.
- [ ] `REC-010`: `ProjectEntry.name` is derived from the package filename stem.
- [ ] `REC-011`: `ProjectEntry.isAccessible` reflects whether the stored file path currently exists.
- [ ] `REC-012`: Inaccessible recent projects remain visible in the Home UI, can be removed from Recents, and can still surface a delete action.

## D. Persistence schema and backward compatibility

### Media manifest

- [ ] `FMT-001`: New manifests default to `version = 2`.
- [ ] `FMT-002`: If `version` is absent while decoding, it must decode as `1`.
- [ ] `FMT-003`: If `entries` is absent while decoding, it must decode as an empty array.
- [ ] `FMT-004`: If `folders` is absent while decoding, it must decode as an empty array.

### Media source model

- [ ] `FMT-005`: A media source is always one of:
  - `external(absolutePath)`
  - `project(relativePath)`
- [ ] `FMT-006`: `GenerationInput` persists prompt/model/duration/aspect ratio plus optional modality-specific fields, reference URLs, reference asset ids, and `createdAt`.

### Timeline decode defaults

- [ ] `FMT-007`: Missing track flags decode to:
  - `muted = false`
  - `hidden = false`
  - `syncLocked = true`
- [ ] `FMT-008`: Missing clip fields must decode to the same defaults as the current Swift model, including default speed/volume/opacity, zero trims/fades, default transform/crop, and nil optional linkage/text fields.
- [ ] `FMT-009`: Timeline round-trips must preserve clip timing, transform/crop, keyframes, text content/style, link groups, and track flags.

### Generation log

- [ ] `FMT-010`: New generation logs default to `version = 1`.
- [ ] `FMT-011`: A missing `GenerationLogEntry.id` decodes to a fresh UUID string.
- [ ] `FMT-012`: Legacy `cost` dollar values migrate to `costCredits = ceil(cost * 100)`.
- [ ] `FMT-013`: If neither `costCredits` nor legacy `cost` exists, `costCredits` remains `nil`.
- [ ] `FMT-014`: `generationLogEntries` are sorted newest-first by `createdAt`, with deterministic fallback ordering when timestamps are absent.

## E. Media resolver contract

- [ ] `RES-001`: `MediaResolver.entry(for:)` returns the live manifest entry for an asset id if one exists.
- [ ] `RES-002`: `MediaResolver.expectedURL(for:)` reconstructs the file URL even if the file is currently missing.
- [ ] `RES-003`: `MediaResolver.resolveURL(for:)` returns a URL only when the expected file currently exists on disk.
- [ ] `RES-004`: `MediaResolver.isMissing(for:)` is true when the expected file does not exist, or when the manifest entry itself is missing.
- [ ] `RES-005`: `MediaResolver.displayName(for:)` falls back to `Offline` when no manifest entry exists.
- [ ] `RES-006`: Resolver reads must reflect live manifest changes immediately. The Rust rewrite must not introduce stale cache behavior when the manifest changes without changing entry count.

## F. Project settings and fps/resolution retiming

- [ ] `PCFG-001`: Timeline settings are `fps`, `width`, `height`, and `settingsConfigured`.
- [ ] `PCFG-002`: When fps changes, the rewrite must rescale:
  - `currentFrame`
  - `sourcePlayheadFrame`
  - clip `startFrame`
  - clip `durationFrames`
  - `trimStartFrame`
  - `trimEndFrame`
  - keyframe frame positions
  - fade lengths
- [ ] `PCFG-003`: FPS retiming must preserve same-track non-overlap after rounding.
- [ ] `PCFG-004`: FPS retiming must collapse rounded keyframe collisions deterministically, matching the current last-value-wins behavior.
- [ ] `PCFG-005`: When canvas size changes, clips still sitting on the old auto-fit transform must be re-fit to the new canvas.
- [ ] `PCFG-006`: When canvas size changes, manually adjusted clips must keep their user-authored transform.
- [ ] `PCFG-007`: Applying new project settings marks `settingsConfigured = true`.

## Migration decisions to record explicitly

- `Decision:` The current project storage root is macOS-specific (`~/Documents/Palmier Pro`). The Rust rewrite should decide whether this stays identical on macOS only, or becomes a per-platform app-data path with migration logic.
- `Decision:` `MediaAsset.toManifestEntry(projectURL:)` currently treats any path with a `projectURL.path` prefix as project-internal. The Rust rewrite should replace that with a stricter descendant check while preserving existing projects.
- `Decision:` The current schema must remain backward-compatible with existing `.palmier` files even if the Rust rewrite introduces a cleaner internal model.

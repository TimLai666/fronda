## ADDED Requirements

### Requirement: add_clips auto mode always creates fresh shared tracks

When every entry of an add_clips call omits trackIndex, the executor SHALL place visual entries on a newly created video track and audio entries (including linked partners) on a newly created audio track appended at the bottom of the audio zone, never reusing existing tracks (upstream #342 semantics).

#### Scenario: Music after linked dialogue does not overwrite it

- **WHEN** a linked dialogue clip pair exists and a subsequent add_clips call adds music with no trackIndex
- **THEN** the dialogue clips remain unmodified and the music lands on a new audio track below the dialogue's audio track

### Requirement: manage_tracks addresses tracks by stable trackId

The manage_tracks tool SHALL accept a stable trackId selector (mutually exclusive with index) for move/remove/set operations, SHALL fail reorders whose destination leaves the track-kind zone with a hard error instead of clamping, SHALL return reorderedTracks/removedTracks receipts, and get_timeline SHALL expose each track's trackId with short-id support (upstream #307).

#### Scenario: trackId survives reordering

- **WHEN** tracks are reordered and a subsequent manage_tracks call addresses a track by trackId
- **THEN** the operation applies to the same track regardless of its new index

#### Scenario: Out-of-zone reorder is a hard error

- **WHEN** a reorder destination would move an audio track into the video zone
- **THEN** the call fails with an explicit error and no track is moved

### Requirement: detect_beats rejects silent video up front

detect_beats SHALL reject media whose manifest entry has has_audio == false with an explicit no-audio error before attempting any decode (upstream #274 follow-up).

#### Scenario: Video without audio

- **WHEN** detect_beats targets a video asset with has_audio false
- **THEN** the tool returns a no-audio error, not a generic decode failure

### Requirement: detect_beats windowed calls report window-local bpm

When a detect_beats call restricts the response window, the reported bpm SHALL be recomputed from the beats inside the window (60 / median inter-beat interval), not the whole-track bpm; empty analyses and empty windows SHALL return distinct explanatory notes, and bpm/downbeats fields SHALL be omitted when absent (upstream #274 follow-up).

#### Scenario: Window over a different-tempo section

- **WHEN** the analysis contains two tempo regions and the request windows the second
- **THEN** the reported bpm reflects the windowed region's inter-beat intervals

### Requirement: Beat cache invalidates when the source file changes

The per-mediaRef beat cache SHALL tag entries with the source file's size and mtime and recompute when the file changes; when the file cannot be stat'ed the cache behaves as before (upstream #274 follow-up).

#### Scenario: Replaced media file

- **WHEN** the file behind a cached mediaRef changes size or mtime and detect_beats runs again
- **THEN** the analysis is recomputed instead of serving the stale cache

### Requirement: import_media contract text matches in-place registration semantics

The import_media tool description and path-property text SHALL state that file-path imports are registered in place and return ready synchronously (files must remain available at their original location), replacing the stale copied-in-background/poll-get_media wording (upstream #333).

#### Scenario: Description read by an agent

- **WHEN** the import_media tool definition is listed
- **THEN** its text contains the in-place registration contract and no "downloading" polling instruction

### Requirement: CAF audio files are importable

The extension and MIME classification tables SHALL accept .caf audio files (ClipType::from_extension, content type audio/x-caf, media-library supported extensions, import_media format list and rejection message) so CAF assets from Swift-authored projects resolve in Fronda (upstream #338).

#### Scenario: Importing a .caf file

- **WHEN** a .caf path is imported through any import path
- **THEN** it classifies as an audio clip instead of being rejected

### Requirement: GenerationInput preserves targetLanguage

MediaManifestEntry.generationInput SHALL round-trip the Swift targetLanguage field (absent field stays absent; present value survives Fronda open→save) (upstream #294 on-disk slice).

#### Scenario: Swift-authored dubbing entry

- **WHEN** a media.json written by Swift contains generationInput.targetLanguage and Fronda loads and saves the project
- **THEN** the saved media.json still contains the identical targetLanguage value

### Requirement: TextStyle preserves v0.6.10 styling fields

TextStyle SHALL round-trip the post-#330/#336 Swift on-disk fields — isUnderlined, isStruckThrough, isOverlined, tracking, lineSpacing, fontCase, border width, and the rich Background object (padding axes, corner radius, offsets, outline color/width) — through the TextStyleWire bridge without loss, while remaining readable from pre-#330 project files (upstream #330/#336 on-disk slices).

#### Scenario: Post-0.6.10 project round-trip

- **WHEN** a project.json text clip written by Swift v0.6.10 with all new style fields set is loaded and saved by Fronda
- **THEN** every new field survives with its original value, key-for-key

#### Scenario: Pre-0.6.9 project still loads

- **WHEN** a project.json written before these fields existed is loaded
- **THEN** decoding succeeds with the new fields at their defaults

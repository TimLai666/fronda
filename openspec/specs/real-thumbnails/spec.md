# real-thumbnails Specification

## Purpose

TBD - created by archiving change 'media-import-recents-thumbnails'. Update Purpose after archive.

## Requirements

### Requirement: Image media renders real thumbnails

Media panel tiles for Image entries SHALL render the source image file (External absolute path, or Project relative path resolved against the current project root). Video entries SHALL render a first-frame thumbnail extracted through the system-ffmpeg adapter when an ffmpeg executable is available (FRONDA_FFMPEG env var, else PATH), cached under the Fronda config directory and keyed by source path plus mtime so source updates re-extract. When the file is missing, the path cannot be resolved, ffmpeg is unavailable, or extraction fails, the tile SHALL fall back to the type-colored placeholder. Audio tiles keep the placeholder. Linking a native decoding library into the app remains an explicit architecture decision this capability does not take.

#### Scenario: Image tile shows the file

- **WHEN** an Image entry references an existing PNG via an absolute path
- **THEN** its tile renders that image

#### Scenario: Missing file falls back

- **WHEN** an Image entry's source file does not exist on disk
- **THEN** its tile renders the type-colored placeholder

#### Scenario: Video tile shows an extracted frame

- **WHEN** ffmpeg is available and a Video entry references an existing video file
- **THEN** its tile renders a frame extracted from that video, and a repeat visit serves the cached thumbnail without re-running ffmpeg

#### Scenario: No ffmpeg falls back silently

- **WHEN** no ffmpeg executable can be started
- **THEN** video tiles render the type-colored placeholder and no error surfaces

#### Scenario: Source update invalidates the cache

- **WHEN** a video file's mtime changes after a thumbnail was cached
- **THEN** the next request extracts a fresh thumbnail


<!-- @trace
source: video-thumbnails-ffmpeg-adapter
updated: 2026-07-03
code:
  - crates/app_shell_gpui/src/video_thumbnails.rs
  - crates/app_shell_gpui/src/lib.rs
  - crates/app_shell_gpui/src/media_panel_view.rs
-->

---
### Requirement: Project cards show the bundle thumbnail

Recent-project cards SHALL render the bundle's thumbnail.png when the file exists, and the placeholder block otherwise.

#### Scenario: Thumbnail present

- **WHEN** a registry entry's bundle contains thumbnail.png
- **THEN** the card renders that image

<!-- @trace
source: media-import-recents-thumbnails
updated: 2026-07-03
code:
  - crates/app_shell_gpui/src/project_registry_store.rs
  - crates/app_shell_gpui/src/app_root.rs
  - crates/app_shell_gpui/src/mcp_service.rs
  - crates/app_shell_gpui/Cargo.toml
  - crates/app_shell_gpui/src/media_import.rs
  - crates/app_shell_gpui/src/media_panel_model.rs
  - crates/app_shell_gpui/src/lib.rs
  - crates/app_shell_gpui/src/editor_state_hub.rs
  - crates/app_shell_gpui/src/home_model.rs
  - crates/app_shell_gpui/src/media_panel_view.rs
-->

---
### Requirement: Thumbnail cache evicts stale and excess entries

The thumbnail cache SHALL bound its growth. After a new thumbnail is written for a source, prior cached files for that same source (same hash prefix, different mtime) SHALL be removed. On app startup a background pass SHALL prune the cache directory to a fixed size cap (256 MB), deleting oldest files first until under the cap; when already under the cap it deletes nothing. All cleanup failures SHALL be silent and MUST NOT affect the app.

#### Scenario: Source update removes the old thumbnail

- **WHEN** a source's mtime changes and a fresh thumbnail is written
- **THEN** the previous thumbnail for that source is removed and unrelated sources' thumbnails remain

#### Scenario: Size cap prunes oldest first

- **WHEN** the cache exceeds the size cap at startup
- **THEN** the oldest files are deleted until the total is under the cap

#### Scenario: Under-cap cache is untouched

- **WHEN** the cache is under the size cap at startup
- **THEN** no files are deleted

##### Example: Prune order

| File | mtime | size |
| ---- | ----- | ---- |
| a.png | oldest | deleted first |
| b.png | middle | deleted next if still over cap |
| c.png | newest | kept |

<!-- @trace
source: thumbnail-cache-cleanup
updated: 2026-07-03
code:
  - crates/app_shell_gpui/src/app_root.rs
  - crates/app_shell_gpui/src/video_thumbnails.rs
-->
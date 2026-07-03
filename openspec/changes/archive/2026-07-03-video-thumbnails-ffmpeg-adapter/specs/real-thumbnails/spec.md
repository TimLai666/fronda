## MODIFIED Requirements

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

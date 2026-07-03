## MODIFIED Requirements

### Requirement: Image media renders real thumbnails

Media panel tiles for Image entries SHALL render the source image file (External absolute path, or Project relative path resolved against the current project root). Video entries SHALL render a first-frame thumbnail decoded in-process through the linked ffmpeg library (statically compiled into the binary on Windows), cached under the Fronda config directory and keyed by source path plus mtime so source updates re-decode; decoding uses the ffmpeg library, not an ffmpeg executable, so no ffmpeg command-line tool is required at runtime. When the file is missing, the path cannot be resolved, or decoding fails, the tile SHALL fall back to the type-colored placeholder. Audio tiles keep the placeholder.

#### Scenario: Image tile shows the file

- **WHEN** an Image entry references an existing PNG via an absolute path
- **THEN** its tile renders that image

#### Scenario: Missing file falls back

- **WHEN** an Image entry's source file does not exist on disk
- **THEN** its tile renders the type-colored placeholder

#### Scenario: Video tile shows a decoded frame without a system ffmpeg

- **WHEN** a Video entry references an existing supported video file and no ffmpeg executable is on PATH
- **THEN** its tile renders a frame decoded by the statically linked ffmpeg, and a repeat visit serves the cached thumbnail without re-decoding

#### Scenario: Decode failure falls back silently

- **WHEN** a video file is corrupt or its codec is unsupported
- **THEN** the tile renders the type-colored placeholder and no error surfaces

#### Scenario: Source update invalidates the cache

- **WHEN** a video file's mtime changes after a thumbnail was cached
- **THEN** the next request re-decodes a fresh thumbnail

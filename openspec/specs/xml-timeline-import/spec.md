# xml-timeline-import Specification

## Purpose

TBD - created by archiving change 'xml-import-fcpxml'. Update Purpose after archive.

## Requirements

### Requirement: FCPXML import parses a project sequence into a Timeline

The system SHALL parse a FCPXML document produced by `fcpxml_export` back into an
`ImportedTimeline` (a `Timeline` plus a list of referenced files and notes),
reversing the exporter's resource table, lane assignment, and rational-time
encoding. `import_xml` SHALL dispatch the `Fcpxml` format to this parser instead
of returning `NotImplemented`.

#### Scenario: fps recovered from the sequence format

- **WHEN** parsing a FCPXML whose `<project>` sequence references a `<format>` with `frameDuration="1/30s"`
- **THEN** the imported `Timeline.fps` SHALL be 30

##### Example: NTSC frame duration

- **GIVEN** a format `frameDuration="1001/30000s"`
- **WHEN** the parser reads it
- **THEN** fps SHALL be 30 (round of 1/(1001/30000))

#### Scenario: rational time strings convert to seconds

- **WHEN** parsing offset/duration/start attributes
- **THEN** `"0s"` becomes 0.0, `"5s"` becomes 5.0, `"120/30s"` becomes 4.0 seconds, and a zero denominator or non-numeric value SHALL yield no value

#### Scenario: nested-media sequence does not shadow the project

- **WHEN** a FCPXML contains nested `<media><sequence>` blocks in `<resources>` before the `<project>` sequence
- **THEN** the parser SHALL read the sequence scoped inside `<project>`, not the first sequence in document order


<!-- @trace
source: xml-import-fcpxml
updated: 2026-07-11
code:
  - crates/agent_contract/src/timeline_v2.rs
  - crates/render_core/src/xml_import.rs
  - crates/app_shell_gpui/src/ai_edit_tab_view.rs
  - crates/app_shell_gpui/src/generation_view.rs
-->

---
### Requirement: Lane assignment round-trips to track order

The parser SHALL reconstruct tracks from asset-clip lanes exactly inverse to the
exporter: positive (video) lanes ordered high-to-low become `tracks[0..]` with
`tracks[0]` the top layer, and negative (audio) lanes ordered `-1, -2, …` follow.

#### Scenario: two video tracks plus one audio track

- **WHEN** a timeline with a top video track, a bottom video track, and an audio track is exported to FCPXML and re-imported
- **THEN** the imported timeline SHALL have three tracks in order Video, Video, Audio, with `tracks[0]` referencing the original top-layer media

#### Scenario: clip placement, duration, and trim survive

- **WHEN** re-importing an exported asset-clip
- **THEN** `start_frame` SHALL equal the clip's timeline offset, `duration_frames` the clip duration, and `trim_start_frame` the clip's source in-point minus the asset's embedded timecode origin

##### Example: source timecode origin subtracted

- **GIVEN** an asset with embedded timecode 30 frames (quanta 30, fps 30) and a clip trimmed 15 frames in
- **WHEN** the exporter writes the asset-clip `start="45/30s"` and re-import runs
- **THEN** the recovered `trim_start_frame` SHALL be 15


<!-- @trace
source: xml-import-fcpxml
updated: 2026-07-11
code:
  - crates/agent_contract/src/timeline_v2.rs
  - crates/render_core/src/xml_import.rs
  - crates/app_shell_gpui/src/ai_edit_tab_view.rs
  - crates/app_shell_gpui/src/generation_view.rs
-->

---
### Requirement: Referenced files are collected for host relink

The parser SHALL collect each asset's `name` and `<media-rep src>` into the
`ImportedTimeline.files` list (deduplicated by asset id) so a host can relink
media by filename. The pure parser SHALL NOT touch the filesystem.

#### Scenario: file references gathered

- **WHEN** importing a FCPXML whose assets carry `<media-rep src="file:///media/top.mp4"/>`
- **THEN** `files` SHALL include an entry whose path contains `top.mp4`


<!-- @trace
source: xml-import-fcpxml
updated: 2026-07-11
code:
  - crates/agent_contract/src/timeline_v2.rs
  - crates/render_core/src/xml_import.rs
  - crates/app_shell_gpui/src/ai_edit_tab_view.rs
  - crates/app_shell_gpui/src/generation_view.rs
-->

---
### Requirement: Unsupported constructs import losslessly-degraded with notes

Retimed clips (`speed != 1`, carrying a `<timeMap>`) SHALL import at 1× speed and
record a note. Nested `<ref-clip>` carriers and `<title>` text overlays SHALL be
skipped with a note. Premiere and Resolve XML SHALL continue to return
`NotImplemented`.

#### Scenario: retimed clip noted

- **WHEN** importing an asset-clip that carries a `<timeMap>`
- **THEN** the clip SHALL import with `speed == 1.0` and `notes` SHALL contain a "retimed" entry

#### Scenario: Premiere/Resolve still stubbed

- **WHEN** `import_xml` is called with `PremiereXml` or `DavinciXml`
- **THEN** it SHALL return `XmlImportError::NotImplemented`

<!-- @trace
source: xml-import-fcpxml
updated: 2026-07-11
code:
  - crates/agent_contract/src/timeline_v2.rs
  - crates/render_core/src/xml_import.rs
  - crates/app_shell_gpui/src/ai_edit_tab_view.rs
  - crates/app_shell_gpui/src/generation_view.rs
-->
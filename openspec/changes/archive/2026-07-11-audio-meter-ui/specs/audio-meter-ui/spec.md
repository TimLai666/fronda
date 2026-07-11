## ADDED Requirements

### Requirement: Timeline audio envelope

`audio_export::timeline_audio_envelope` SHALL return a mono peak envelope
(values in 0..1) of the whole timeline's mixed audio, with the requested number
of buckets. It SHALL return empty for a silent timeline or zero buckets, and
SHALL decode via the existing export audio path.

#### Scenario: non-silent envelope from an audio clip

- **WHEN** a timeline with one audible audio clip is requested with N buckets
- **THEN** the result SHALL have N values, all within 0..1, and at least one clearly above silence

### Requirement: Playhead-driven transport meter

The preview transport SHALL show a stereo level meter fed by the timeline audio
level at the playhead: the envelope is computed off the UI thread and cached by
project revision, and each render samples it at the playhead frame and ingests
it into the pure meter with a monotonic time. The meter SHALL render L/R level
bars with a peak tick and a clip tint.

#### Scenario: meter reflects the playhead level

- **WHEN** the playhead sits over a loud region of the timeline audio
- **THEN** the meter bars SHALL fill toward their maximum; over silence they SHALL fall toward the floor

#### Scenario: envelope computed once per revision

- **WHEN** the project has not changed
- **THEN** the envelope SHALL NOT be recomputed (the cached one is reused)

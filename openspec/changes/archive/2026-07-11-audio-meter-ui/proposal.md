## Why

The audio meter pure core landed (`audio-meter-core`) but had no UI feed because Fronda has no live audio playback engine to tap. This adds a visible meter fed from the timeline audio level at the playhead — a background-decoded envelope sampled per frame — so the transport shows a moving level meter. (It's a playhead meter, not a live-output meter; value is modest until audio playback exists, but the user chose to build it.)

## What Changes

- `audio_export::timeline_audio_envelope`: mono peak envelope (0..1) of the whole timeline's mixed audio, one bucket per frame. Reuses the export decode/mix path. Tested against a generated WAV.
- Preview view: compute the envelope off the UI thread (cached by project revision, mirroring the preview-PNG background pattern); each render, sample the envelope at the playhead, ingest it into the pure `StereoMeter` with a monotonic time, and render L/R level bars (peak tick + clip tint) in the transport bar.

## Non-Goals

- No live audio output engine (the meter is playhead-driven, not tapped from playback).
- Mono feed shown on both channels (no separate L/R until a stereo envelope / real playback exists).
- Correct behavior is human-verified in the running app (this repo can't run gpui) — compile + the envelope test cover what's machine-checkable.

## Capabilities

### New Capabilities

- `audio-meter-ui`: a transport-bar stereo level meter fed by the timeline audio envelope at the playhead.

## Impact

- Affected code:
  - Modified: `crates/app_shell_gpui/src/{audio_export.rs,preview_view.rs}`
- No on-disk / tool-surface change. Envelope decode is background + revision-cached, so it doesn't block the UI.

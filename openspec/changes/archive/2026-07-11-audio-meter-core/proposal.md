## Why

Upstream #293 adds an audio level meter. Its Swift core (`AudioMeterChannelState` / `AudioMeterHub`) is a **pure state machine** — dB conversion, level/peak decay, peak-hold, clip latching — independent of any audio engine. Port that now so the meter's logic is at parity and fully tested. The visible meter widget is deferred: Fronda's Rust app has **no real-time audio playback engine**, so there's no live output to tap; a visible meter would need background decoding of the timeline audio envelope, which is disproportionate until audio playback exists (and it largely duplicates the waveform when you can't hear the audio).

## What Changes

- New `audio_core::audio_meter`: `MeterChannel` / `StereoMeter` mirroring Swift exactly — `ingest(peak, time)`, `display(time)` with `LEVEL_DECAY_DB_PER_SEC=24`, `PEAK_DECAY_DB_PER_SEC=18`, `PEAK_HOLD_SECONDS=1.5`, floor −60 dB; `decibels`, `normalized_level`, `peak_magnitude` helpers.
- Time is an injected parameter (seconds), so the state machine is deterministic and unit-tested with synthetic times — the eventual UI passes real monotonic time.

## Non-Goals

- No meter UI widget yet (deferred pending an audio playback engine; a background timeline-audio-envelope feed is the follow-up when playback lands).
- No audio playback engine (separate, larger work).
- No change to any existing behavior, on-disk format, or tool surface.

## Capabilities

### New Capabilities

- `audio-meter-core`: a pure, deterministic stereo peak-meter state machine (dB, decay, peak-hold, clip) ready to drive a meter UI once a live or playhead audio feed exists.

## Impact

- Affected code:
  - New: `crates/audio_core/src/audio_meter.rs` (+ `pub mod` in `lib.rs`)
- No dependency, on-disk, or contract change. Pure additive module with 8 unit tests.

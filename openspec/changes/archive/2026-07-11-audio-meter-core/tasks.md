## 1. Pure meter core

- [x] 1.1 `MeterChannel`/`StereoMeter` + `decibels`/`normalized_level`/`peak_magnitude` mirror Swift's meter (decay, peak-hold, clip). Verified by 8 `audio_meter` tests.

- [x] 1.2 Meter UI widget + feed is explicitly out of scope (proposal Non-Goals): blocked on an audio playback engine — a follow-up will feed it from a background timeline-audio-envelope (cached per revision) sampled at the playhead, or tap the audio engine once playback lands.

## 2. Gates

- [x] 2.1 `cargo test -p fronda-audio-core` exit 0 (audio_meter suite green).
- [x] 2.2 `cargo test --workspace` exit 0.

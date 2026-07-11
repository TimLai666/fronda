## ADDED Requirements

### Requirement: Pure stereo audio meter state machine

`audio_core::audio_meter` SHALL provide a deterministic stereo peak meter
mirroring Swift's `AudioMeterChannelState` / `AudioMeterHub`: a channel ingests
a peak amplitude at a monotonic time (seconds) and reports a decayed level and a
held peak in dB (floor −60 dB, level decay 24 dB/s, peak decay 18 dB/s, peak
hold 1.5 s), latching a clip flag at full scale. Time is an injected parameter
so behavior is deterministic and unit-testable.

#### Scenario: dB reference points

- **WHEN** converting amplitudes with `decibels`
- **THEN** 1.0 → 0 dB, 0.1 → −20 dB, and 0 → the −60 dB floor

#### Scenario: level decays after ingest

- **WHEN** a channel ingests full scale at t=0 and is displayed at t=1
- **THEN** the level SHALL have dropped by 24 dB (the per-second decay)

#### Scenario: peak holds then decays

- **WHEN** a channel ingests full scale at t=0
- **THEN** the peak SHALL still read 0 dB at t=1 (within the 1.5 s hold) and have decayed by a time past the hold window

#### Scenario: clip flag latches at full scale

- **WHEN** a channel ingests a peak ≥ 1.0
- **THEN** its display SHALL report clipped until `reset_clipping`

#### Scenario: stereo channels are independent

- **WHEN** the left channel is fed loud and the right quiet
- **THEN** their displayed levels SHALL differ, and `reset` SHALL return both to the floor

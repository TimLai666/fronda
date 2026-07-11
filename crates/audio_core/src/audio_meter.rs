//! Pure audio level-meter state machine (upstream #293). Mirrors Swift
//! `AudioMeterChannelState` / `AudioMeterHub`: ingest a peak amplitude at a
//! monotonic time (seconds), decay level and peak in dB over time with a
//! peak-hold, and flag clipping. No audio-output dependency — the caller feeds
//! peaks (e.g. from the mixed timeline audio at the playhead) and supplies the
//! time, so the whole thing is deterministic and unit-testable.

pub const METER_FLOOR_DB: f32 = -60.0;
pub const METER_CEILING_DB: f32 = 0.0;
const LEVEL_DECAY_DB_PER_SEC: f32 = 24.0;
const PEAK_DECAY_DB_PER_SEC: f32 = 18.0;
const PEAK_HOLD_SECONDS: f64 = 1.5;

/// Amplitude (0..1+) → dB, floored at [`METER_FLOOR_DB`].
pub fn decibels(amplitude: f32) -> f32 {
    if amplitude > 0.0 {
        (20.0 * amplitude.log10()).max(METER_FLOOR_DB)
    } else {
        METER_FLOOR_DB
    }
}

/// A meter's normalized 0..1 fill for a dB value across floor..ceiling.
pub fn normalized_level(db: f32) -> f32 {
    ((db - METER_FLOOR_DB) / (METER_CEILING_DB - METER_FLOOR_DB)).clamp(0.0, 1.0)
}

/// Peak (max magnitude) of a sample slice — the analyzer feed.
pub fn peak_magnitude(samples: &[f32]) -> f32 {
    samples.iter().fold(0.0f32, |m, &s| m.max(s.abs()))
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeterChannelDisplay {
    pub level_db: f32,
    pub peak_db: f32,
    pub clipped: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StereoMeterDisplay {
    pub left: MeterChannelDisplay,
    pub right: MeterChannelDisplay,
}

/// One channel's decaying level + held peak.
#[derive(Debug, Clone)]
pub struct MeterChannel {
    level_db: f32,
    level_time: f64,
    peak_db: f32,
    peak_hold_until: f64,
    clipped: bool,
}

impl Default for MeterChannel {
    fn default() -> Self {
        Self {
            level_db: METER_FLOOR_DB,
            level_time: 0.0,
            peak_db: METER_FLOOR_DB,
            peak_hold_until: 0.0,
            clipped: false,
        }
    }
}

impl MeterChannel {
    /// Feed a peak amplitude at `time` (seconds). The level jumps to the louder
    /// of the incoming and the currently-decayed level; the peak holds when
    /// higher, else keeps decaying from the previous peak.
    pub fn ingest(&mut self, peak: f32, time: f64) {
        let current = self.display(time);
        let incoming = decibels(peak);
        self.level_db = incoming.max(current.level_db);
        self.level_time = time;
        if incoming >= current.peak_db {
            self.peak_db = incoming;
            self.peak_hold_until = time + PEAK_HOLD_SECONDS;
        } else if time > self.peak_hold_until {
            self.peak_db = current.peak_db;
            self.peak_hold_until = time;
        }
        self.clipped = self.clipped || peak >= 1.0;
    }

    /// The decayed level/peak at `time`.
    pub fn display(&self, time: f64) -> MeterChannelDisplay {
        let level_elapsed = (time - self.level_time).max(0.0) as f32;
        let peak_elapsed = (time - self.peak_hold_until).max(0.0) as f32;
        MeterChannelDisplay {
            level_db: (self.level_db - level_elapsed * LEVEL_DECAY_DB_PER_SEC).max(METER_FLOOR_DB),
            peak_db: (self.peak_db - peak_elapsed * PEAK_DECAY_DB_PER_SEC).max(METER_FLOOR_DB),
            clipped: self.clipped,
        }
    }

    pub fn reset_clipping(&mut self) {
        self.clipped = false;
    }
}

/// Stereo meter hub (Swift `AudioMeterHub`).
#[derive(Debug, Clone, Default)]
pub struct StereoMeter {
    left: MeterChannel,
    right: MeterChannel,
}

impl StereoMeter {
    pub fn ingest(&mut self, left_peak: f32, right_peak: f32, time: f64) {
        self.left.ingest(left_peak, time);
        self.right.ingest(right_peak, time);
    }

    pub fn display(&self, time: f64) -> StereoMeterDisplay {
        StereoMeterDisplay {
            left: self.left.display(time),
            right: self.right.display(time),
        }
    }

    pub fn reset_clipping(&mut self) {
        self.left.reset_clipping();
        self.right.reset_clipping();
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.01
    }

    #[test]
    fn decibels_reference_points() {
        assert!(approx(decibels(1.0), 0.0), "full scale = 0 dB");
        assert_eq!(decibels(0.0), METER_FLOOR_DB, "silence floors");
        assert!(approx(decibels(0.1), -20.0), "0.1 = -20 dB");
        assert_eq!(decibels(1e-9), METER_FLOOR_DB, "below floor clamps");
    }

    #[test]
    fn normalized_level_spans_floor_to_ceiling() {
        assert!(approx(normalized_level(METER_FLOOR_DB), 0.0));
        assert!(approx(normalized_level(METER_CEILING_DB), 1.0));
        assert!(approx(normalized_level(-30.0), 0.5));
    }

    #[test]
    fn peak_magnitude_is_max_abs() {
        assert!(approx(peak_magnitude(&[0.1, -0.8, 0.3]), 0.8));
        assert_eq!(peak_magnitude(&[]), 0.0);
    }

    #[test]
    fn ingest_sets_level_then_decays_toward_floor() {
        let mut ch = MeterChannel::default();
        ch.ingest(1.0, 0.0); // 0 dB
        assert!(approx(ch.display(0.0).level_db, 0.0));
        // After 1s the level decays by LEVEL_DECAY_DB_PER_SEC (24).
        assert!(approx(ch.display(1.0).level_db, -24.0));
        // Eventually floors.
        assert_eq!(ch.display(100.0).level_db, METER_FLOOR_DB);
    }

    #[test]
    fn peak_holds_then_decays() {
        let mut ch = MeterChannel::default();
        ch.ingest(1.0, 0.0);
        // Held for PEAK_HOLD_SECONDS (1.5s) at 0 dB.
        assert!(approx(ch.display(1.0).peak_db, 0.0), "still held at 1s");
        // After the hold window, the peak decays (18 dB/s).
        let d = ch.display(2.5); // 1.0s past hold-until (1.5)
        assert!(approx(d.peak_db, -18.0), "peak decays past hold, got {}", d.peak_db);
    }

    #[test]
    fn louder_ingest_raises_held_level() {
        let mut ch = MeterChannel::default();
        ch.ingest(0.1, 0.0); // -20 dB
        ch.ingest(1.0, 0.1); // 0 dB louder → level jumps up
        assert!(approx(ch.display(0.1).level_db, 0.0));
    }

    #[test]
    fn clip_flag_latches_at_full_scale() {
        let mut ch = MeterChannel::default();
        ch.ingest(0.5, 0.0);
        assert!(!ch.display(0.0).clipped);
        ch.ingest(1.0, 0.1);
        assert!(ch.display(0.1).clipped, "peak >= 1 latches clip");
        ch.reset_clipping();
        assert!(!ch.display(0.1).clipped);
    }

    #[test]
    fn stereo_channels_are_independent() {
        let mut m = StereoMeter::default();
        m.ingest(1.0, 0.01, 0.0); // loud left, quiet right
        let d = m.display(0.0);
        assert!(approx(d.left.level_db, 0.0));
        assert!(d.right.level_db < -20.0, "right quiet, got {}", d.right.level_db);
        m.reset();
        assert_eq!(m.display(0.0).left.level_db, METER_FLOOR_DB);
    }
}

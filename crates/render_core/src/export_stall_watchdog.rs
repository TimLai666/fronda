/// Pure state machine for export stall detection (Upstream #95).
///
/// Tracks export progress over time and detects when progress has not
/// advanced past an epsilon threshold within a configurable timeout period.
/// The caller supplies `current_time` in arbitrary units (seconds, ms, etc.),
/// making this fully testable with no platform clock dependency.
#[derive(Debug, Clone, PartialEq)]
pub struct ExportStallWatchdog {
    progress: f64,
    stall_timeout_seconds: u64,
    last_progress_time_units: u64,
    cancelled: bool,
}

/// Minimum progress change required to consider progress advanced (0.1%).
const PROGRESS_EPSILON: f64 = 0.001;

impl ExportStallWatchdog {
    /// Create a new watchdog with the given stall timeout.
    ///
    /// `stall_timeout_seconds` is the maximum allowed time (in caller-supplied
    /// time units) without meaningful progress before the export is considered
    /// stalled.
    pub fn new(stall_timeout_seconds: u64) -> Self {
        Self {
            progress: 0.0,
            stall_timeout_seconds,
            last_progress_time_units: 0,
            cancelled: false,
        }
    }

    /// Record a progress sample at the given time.
    ///
    /// If `|value - progress| > PROGRESS_EPSILON`, updates the stored progress
    /// (clamped to [0.0, 1.0]) and resets the stall timer to `current_time`.
    /// Otherwise, the sample is ignored as noise.
    pub fn update_progress(&mut self, value: f64, current_time: u64) {
        let clamped = value.clamp(0.0, 1.0);
        if (clamped - self.progress).abs() > PROGRESS_EPSILON {
            self.progress = clamped;
            self.last_progress_time_units = current_time;
        }
    }

    /// Check whether the export has stalled.
    ///
    /// Returns `false` if the export has been cancelled, or if the elapsed
    /// time since the last meaningful progress sample does not exceed the
    /// stall timeout.
    pub fn has_stalled(&self, current_time: u64) -> bool {
        if self.cancelled {
            return false;
        }
        current_time - self.last_progress_time_units > self.stall_timeout_seconds
    }

    /// Cancel the watchdog. Once cancelled, `has_stalled` always returns
    /// `false`.
    pub fn cancel(&mut self) {
        self.cancelled = true;
    }

    /// Get the current progress value.
    pub fn progress(&self) -> f64 {
        self.progress
    }

    /// Reset to initial state (progress 0.0, timer at 0, not cancelled).
    pub fn reset(&mut self) {
        self.progress = 0.0;
        self.last_progress_time_units = 0;
        self.cancelled = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_watchdog_starts_idle() {
        let wd = ExportStallWatchdog::new(120);
        assert!((wd.progress() - 0.0).abs() < f64::EPSILON);
        assert!(!wd.has_stalled(0));
        assert!(!wd.cancelled);
    }

    #[test]
    fn update_progress_advances_time() {
        let mut wd = ExportStallWatchdog::new(120);
        wd.update_progress(0.5, 10);
        // At time 11, only 1 unit elapsed — well within timeout.
        assert!(!wd.has_stalled(11));
        assert!((wd.progress() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn stall_detected_after_timeout() {
        let mut wd = ExportStallWatchdog::new(120);
        wd.update_progress(0.1, 0);
        // At time 121, 121 units have elapsed, which exceeds timeout of 120.
        assert!(wd.has_stalled(121));
    }

    #[test]
    fn stall_not_yet_detected_within_timeout() {
        let mut wd = ExportStallWatchdog::new(120);
        wd.update_progress(0.1, 0);
        // At time 119, elapsed time (119) is NOT > timeout (120).
        assert!(!wd.has_stalled(119));
    }

    #[test]
    fn cancel_prevents_stall_detection() {
        let mut wd = ExportStallWatchdog::new(120);
        wd.update_progress(0.1, 0);
        wd.cancel();
        // Even though elapsed time exceeds timeout, cancelled overrides it.
        assert!(!wd.has_stalled(200));
    }

    #[test]
    fn update_progress_resets_stall_timer() {
        let mut wd = ExportStallWatchdog::new(120);
        wd.update_progress(0.1, 0);
        // Update to 0.9 at time 200 — this resets the timer.
        wd.update_progress(0.9, 200);
        // At time 201, only 1 unit since last update — not stalled.
        assert!(!wd.has_stalled(201));
        // But at time 321, 121 units elapsed — stalled again.
        assert!(wd.has_stalled(321));
    }

    #[test]
    fn progress_clamped_to_0_1() {
        let mut wd = ExportStallWatchdog::new(120);
        wd.update_progress(-0.5, 1);
        assert!((wd.progress() - 0.0).abs() < f64::EPSILON);
        wd.update_progress(1.5, 2);
        assert!((wd.progress() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn epsilon_less_than_0_001_does_not_advance_time() {
        let mut wd = ExportStallWatchdog::new(120);
        wd.update_progress(0.0, 0);
        // A tiny change below epsilon should NOT update progress or time.
        wd.update_progress(0.0005, 100);
        assert!((wd.progress() - 0.0).abs() < f64::EPSILON);
        // Since time was never advanced, elapsed (100) <= timeout (120).
        assert!(!wd.has_stalled(100));
        // But well past timeout, stall is detected because timer never moved.
        assert!(wd.has_stalled(200));
    }

    #[test]
    fn default_timeout_120_seconds() {
        let wd = ExportStallWatchdog::new(120);
        assert_eq!(wd.stall_timeout_seconds, 120);
    }
}

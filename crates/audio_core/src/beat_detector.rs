//! On-device beat/downbeat detection (tool-surface-v2 `detect_beats`).
//!
//! Deterministic energy-flux pipeline: mono mixdown → hop-windowed energy →
//! half-wave-rectified onset envelope → tempo by autocorrelation (60–200 BPM)
//! → beat grid phase-fit to the strongest onsets → downbeats as the
//! strongest-phase every-4th beat. Times are SOURCE seconds.

/// Analysis result: beat and downbeat instants in source seconds + tempo.
#[derive(Debug, Clone, PartialEq)]
pub struct BeatAnalysis {
    /// Estimated tempo in beats per minute (0.0 when no periodicity found).
    pub bpm: f64,
    pub beats: Vec<f64>,
    pub downbeats: Vec<f64>,
}

const HOP: usize = 512;
const MIN_BPM: f64 = 60.0;
const MAX_BPM: f64 = 200.0;

/// Detect beats in interleaved PCM. Returns an empty analysis for silence or
/// aperiodic material (speech, ambience).
pub fn detect_beats(pcm: &[f32], channels: usize, sample_rate: u32) -> BeatAnalysis {
    let empty = BeatAnalysis {
        bpm: 0.0,
        beats: Vec::new(),
        downbeats: Vec::new(),
    };
    let channels = channels.max(1);
    let sr = sample_rate as f64;
    let frames = pcm.len() / channels;
    if frames < HOP * 8 {
        return empty;
    }

    // Mono energy per hop window.
    let hops = frames / HOP;
    let mut energy: Vec<f64> = Vec::with_capacity(hops);
    for h in 0..hops {
        let mut sum = 0.0f64;
        for i in 0..HOP {
            let frame = h * HOP + i;
            let mut sample = 0.0f64;
            for c in 0..channels {
                sample += pcm[frame * channels + c] as f64;
            }
            sample /= channels as f64;
            sum += sample * sample;
        }
        energy.push(sum / HOP as f64);
    }

    // Half-wave rectified energy flux = onset strength.
    let mut onset: Vec<f64> = vec![0.0; energy.len()];
    for i in 1..energy.len() {
        onset[i] = (energy[i] - energy[i - 1]).max(0.0);
    }
    let total: f64 = onset.iter().sum();
    if total <= f64::EPSILON {
        return empty;
    }

    // Tempo: autocorrelation of the onset envelope over the BPM-band lags.
    let hop_seconds = HOP as f64 / sr;
    let min_lag = ((60.0 / MAX_BPM) / hop_seconds).round() as usize;
    let max_lag = (((60.0 / MIN_BPM) / hop_seconds).round() as usize).min(onset.len() / 2);
    if min_lag == 0 || min_lag >= max_lag {
        return empty;
    }
    let lag_score = |lag: usize| -> f64 {
        let mut score = 0.0f64;
        for i in lag..onset.len() {
            score += onset[i] * onset[i - lag];
        }
        score / (onset.len() - lag) as f64
    };
    let mut best_lag = 0usize;
    let mut best_score = 0.0f64;
    for lag in min_lag..=max_lag {
        let score = lag_score(lag);
        if score > best_score {
            best_score = score;
            best_lag = lag;
        }
    }
    if best_lag == 0 || best_score <= f64::EPSILON {
        return empty;
    }
    // Accented bars double the period (120 BPM reads as 60): prefer the
    // half-lag subdivision whenever it still carries real correlation.
    while best_lag / 2 >= min_lag && lag_score(best_lag / 2) > 0.4 * best_score {
        best_lag /= 2;
        best_score = lag_score(best_lag);
    }
    let period_seconds = best_lag as f64 * hop_seconds;
    let bpm = 60.0 / period_seconds;

    // Beat grid: pick the phase (in hops) with the strongest onset support.
    let mut best_phase = 0usize;
    let mut best_phase_score = -1.0f64;
    for phase in 0..best_lag {
        let mut score = 0.0f64;
        let mut i = phase;
        while i < onset.len() {
            // Allow ±1 hop of slop around each grid point.
            let lo = i.saturating_sub(1);
            let hi = (i + 1).min(onset.len() - 1);
            score += onset[lo..=hi].iter().cloned().fold(0.0, f64::max);
            i += best_lag;
        }
        if score > best_phase_score {
            best_phase_score = score;
            best_phase = phase;
        }
    }

    let mut beats: Vec<f64> = Vec::new();
    let mut i = best_phase;
    while i < onset.len() {
        beats.push(i as f64 * hop_seconds);
        i += best_lag;
    }

    // Downbeats: the every-4th-beat phase with the strongest onsets.
    let mut best_db_phase = 0usize;
    let mut best_db_score = -1.0f64;
    for phase in 0..4.min(beats.len()) {
        let mut score = 0.0f64;
        let mut k = phase;
        while k < beats.len() {
            let hop_idx = (beats[k] / hop_seconds).round() as usize;
            let lo = hop_idx.saturating_sub(1);
            let hi = (hop_idx + 1).min(onset.len() - 1);
            score += onset[lo..=hi].iter().cloned().fold(0.0, f64::max);
            k += 4;
        }
        if score > best_db_score {
            best_db_score = score;
            best_db_phase = phase;
        }
    }
    let downbeats: Vec<f64> = beats
        .iter()
        .skip(best_db_phase)
        .step_by(4)
        .copied()
        .collect();

    BeatAnalysis {
        bpm,
        beats,
        downbeats,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: u32 = 44_100;

    /// A click track: short bursts every `interval` seconds, `accent_every`-th
    /// click louder (the bar downbeat).
    fn click_track(seconds: f64, bpm: f64, accent_every: usize, accent_offset: usize) -> Vec<f32> {
        let n = (seconds * SR as f64) as usize;
        let mut pcm = vec![0.0f32; n];
        let interval = 60.0 / bpm;
        let mut k = 0usize;
        let mut t = 0.0f64;
        while t < seconds {
            let start = (t * SR as f64) as usize;
            let amp = if (k + accent_every - accent_offset).is_multiple_of(accent_every) {
                0.9
            } else {
                0.4
            };
            for i in 0..((SR as usize) / 100).min(n.saturating_sub(start)) {
                pcm[start + i] = amp * (1.0 - i as f32 / (SR as f32 / 100.0));
            }
            t += interval;
            k += 1;
        }
        pcm
    }

    #[test]
    fn silence_has_no_beats() {
        let pcm = vec![0.0f32; SR as usize * 4];
        let a = detect_beats(&pcm, 1, SR);
        assert_eq!(a.bpm, 0.0);
        assert!(a.beats.is_empty());
        assert!(a.downbeats.is_empty());
    }

    #[test]
    fn click_track_at_120_bpm_detected() {
        let pcm = click_track(8.0, 120.0, 4, 0);
        let a = detect_beats(&pcm, 1, SR);
        assert!((a.bpm - 120.0).abs() < 3.0, "bpm={}", a.bpm);
        assert!(a.beats.len() >= 14, "beats={}", a.beats.len());
        // Beats land within 25 ms of the click grid (0.5s spacing).
        for b in &a.beats {
            let nearest = (b / 0.5).round() * 0.5;
            assert!((b - nearest).abs() < 0.025, "beat {b} off-grid");
        }
    }

    #[test]
    fn downbeats_are_every_fourth_beat_on_the_accent() {
        let pcm = click_track(8.0, 120.0, 4, 0);
        let a = detect_beats(&pcm, 1, SR);
        assert!(!a.downbeats.is_empty());
        assert!(
            a.downbeats.len() * 3 <= a.beats.len() + 3,
            "downbeats are a quarter of beats: {} vs {}",
            a.downbeats.len(),
            a.beats.len()
        );
        // The accented clicks sit on 2-second bars (4 beats at 120 BPM).
        for d in &a.downbeats {
            let nearest = (d / 2.0).round() * 2.0;
            assert!((d - nearest).abs() < 0.025, "downbeat {d} off the bar grid");
        }
    }

    #[test]
    fn stereo_interleaved_input_handled() {
        let mono = click_track(6.0, 100.0, 4, 0);
        let stereo: Vec<f32> = mono.iter().flat_map(|&s| [s, s]).collect();
        let a = detect_beats(&stereo, 2, SR);
        assert!((a.bpm - 100.0).abs() < 3.0, "bpm={}", a.bpm);
    }
}

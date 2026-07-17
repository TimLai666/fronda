/// A single RMS envelope frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RmsFrame {
    /// RMS value for this frame
    pub rms: f64,
    /// Time position in seconds
    pub time_seconds: f64,
}

/// Result of audio sync correlation.
#[derive(Debug, Clone, PartialEq)]
pub struct SyncOffset {
    /// Offset in frames at the project frame rate.
    /// Positive means the reference is ahead — its signal starts earlier, so the
    /// target's signal is delayed (starts later).
    pub offset_frames: i64,
    /// Confidence score (0.0 to 1.0). Higher = more confident.
    pub confidence: f64,
    /// The lag at which peak correlation was found (in frames of the RMS envelope).
    pub peak_lag_frames: i64,
}

/// Absolute floor on scored overlap, in RMS hops; #269 raises the effective floor to 3 seconds.
const MIN_OVERLAP_HOPS: usize = 16;

/// ± seconds around a capture-date seed; covers typical device clock skew
/// (Swift `SyncDefaults.dateSeedWindowSeconds`, upstream #269).
pub const DATE_SEED_WINDOW_SECONDS: f64 = 3.0;

/// A capture-date-implied search seed (upstream #269): the lags within
/// `center_seconds ± window_seconds` are searched first; the full window is
/// the fallback when the seeded peak's confidence is insufficient.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SyncSeed {
    /// Expected lag in seconds, in this correlator's sign convention
    /// (positive = the shared content sits later in the TARGET's file =
    /// the target started recording earlier).
    pub center_seconds: f64,
    /// ± seconds searched around the center. Capped at the global search
    /// window's width (Swift caps `seedWindow` at `maxLag`).
    pub window_seconds: f64,
}

/// Audio sync correlator using RMS-envelope cross-correlation.
pub struct AudioSyncCorrelator;

impl AudioSyncCorrelator {
    /// Extract RMS envelope from raw audio samples.
    ///
    /// Divides samples into frames of `frame_size` samples and computes
    /// the RMS for each frame.
    ///
    /// Parameters:
    /// - `samples`: Raw PCM samples (f64, -1.0 to 1.0)
    /// - `sample_rate`: Sample rate in Hz
    /// - `frame_size`: Number of samples per RMS frame (e.g., 1024)
    ///
    /// Returns Vec<RmsFrame> with time_seconds for each frame.
    pub fn extract_rms_envelope(
        samples: &[f64],
        sample_rate: f64,
        frame_size: usize,
    ) -> Vec<RmsFrame> {
        let frame_size = frame_size.max(1);
        let mut frames = Vec::with_capacity(samples.len().div_ceil(frame_size));

        for (i, chunk) in samples.chunks(frame_size).enumerate() {
            let len = chunk.len();
            if len == 0 {
                continue;
            }
            let sum_sq: f64 = chunk.iter().map(|s| s * s).sum();
            let rms = (sum_sq / len as f64).sqrt();
            let time_seconds = (i * frame_size) as f64 / sample_rate;
            frames.push(RmsFrame { rms, time_seconds });
        }

        frames
    }

    /// Compute cross-correlation between two RMS envelopes.
    ///
    /// Returns correlation values for each lag from -(reference.len()-1) to (reference.len()-1).
    /// Positive lag means the target's signal is shifted right (target delayed) — the
    /// reference is ahead / starts earlier (see the shifted-signal test).
    ///
    /// Uses the Pearson correlation coefficient computed per-lag over the
    /// overlapping portion of the two envelopes.
    pub fn cross_correlate(reference: &[RmsFrame], target: &[RmsFrame]) -> Vec<(i64, f64)> {
        if reference.is_empty() || target.is_empty() {
            return Vec::new();
        }

        let ref_vals: Vec<f64> = reference.iter().map(|f| f.rms).collect();
        let tgt_vals: Vec<f64> = target.iter().map(|f| f.rms).collect();

        let n = tgt_vals.len();
        let m = ref_vals.len();
        let num_lags = n + m - 1;
        let mut results = Vec::with_capacity(num_lags);

        // Lag range: -(m-1) to (n-1)
        // Positive lag = target's signal delayed; the target is indexed further in
        // (tgt_start = lag), so the reference is ahead / starts earlier.
        for lag in -(m as i64 - 1)..=(n as i64 - 1) {
            let ref_start = 0.max(-lag) as usize;
            let tgt_start = 0.max(lag) as usize;
            let len = (m.saturating_sub(ref_start)).min(n.saturating_sub(tgt_start));

            if len == 0 {
                results.push((lag, 0.0));
                continue;
            }

            let ref_slice = &ref_vals[ref_start..ref_start + len];
            let tgt_slice = &tgt_vals[tgt_start..tgt_start + len];

            // Compute means of the overlapping windows
            let ref_mean = ref_slice.iter().sum::<f64>() / len as f64;
            let tgt_mean = tgt_slice.iter().sum::<f64>() / len as f64;

            // Compute centered dot product and norms
            let mut dot = 0.0;
            let mut ref_norm_sq = 0.0;
            let mut tgt_norm_sq = 0.0;

            for (&r, &t) in ref_slice.iter().zip(tgt_slice.iter()) {
                let rc = r - ref_mean;
                let tc = t - tgt_mean;
                dot += rc * tc;
                ref_norm_sq += rc * rc;
                tgt_norm_sq += tc * tc;
            }

            let correlation = if ref_norm_sq > 0.0 && tgt_norm_sq > 0.0 {
                dot / (ref_norm_sq.sqrt() * tgt_norm_sq.sqrt())
            } else {
                0.0
            };

            results.push((lag, correlation));
        }

        results
    }

    /// Find the sync offset between two audio signals.
    ///
    /// Returns the offset and confidence, or None if signals are too short or
    /// no good match found.
    pub fn find_sync_offset(
        reference_samples: &[f64],
        target_samples: &[f64],
        sample_rate: f64,
        frame_size: usize,
        project_fps: f64,
    ) -> Option<SyncOffset> {
        Self::find_sync_offset_windowed(
            reference_samples,
            target_samples,
            sample_rate,
            frame_size,
            project_fps,
            None,
        )
    }

    /// [`AudioSyncCorrelator::find_sync_offset`] restricted to lags within
    /// ±`max_offset_seconds` (Swift `sync_audio`'s `searchWindowSeconds`).
    /// `None` searches all lags.
    pub fn find_sync_offset_windowed(
        reference_samples: &[f64],
        target_samples: &[f64],
        sample_rate: f64,
        frame_size: usize,
        project_fps: f64,
        max_offset_seconds: Option<f64>,
    ) -> Option<SyncOffset> {
        Self::find_sync_offset_seeded(
            reference_samples,
            target_samples,
            sample_rate,
            frame_size,
            project_fps,
            max_offset_seconds,
            None,
            0.0,
        )
    }

    /// [`AudioSyncCorrelator::find_sync_offset_windowed`] with an optional
    /// capture-date seed (upstream #269): the lags within `seed.center ±
    /// seed.window` are searched first, and that peak is returned when its
    /// confidence clears `min_confidence`; otherwise the full window is
    /// searched exactly as an unseeded run would.
    ///
    /// The seeded pass's gate and reported confidence use Swift's metric —
    /// the raw Pearson score at the winning lag, clamped to 0..1 — because a
    /// window that misses the true alignment can still contain a locally
    /// prominent (but weak) peak. The global pass keeps this correlator's
    /// existing peak-prominence confidence.
    #[allow(clippy::too_many_arguments)]
    pub fn find_sync_offset_seeded(
        reference_samples: &[f64],
        target_samples: &[f64],
        sample_rate: f64,
        frame_size: usize,
        project_fps: f64,
        max_offset_seconds: Option<f64>,
        seed: Option<SyncSeed>,
        min_confidence: f64,
    ) -> Option<SyncOffset> {
        if reference_samples.len() < frame_size || target_samples.len() < frame_size {
            return None;
        }

        // 1. Extract RMS envelopes
        let ref_rms = Self::extract_rms_envelope(reference_samples, sample_rate, frame_size);
        let tgt_rms = Self::extract_rms_envelope(target_samples, sample_rate, frame_size);

        if ref_rms.is_empty() || tgt_rms.is_empty() {
            return None;
        }

        let seconds_per_rms_frame = frame_size as f64 / sample_rate;
        let mut correlation = Self::cross_correlate(&ref_rms, &tgt_rms);
        // Thin-edge overlaps score spurious perfect correlations that can beat the true alignment (#269).
        let min_overlap_hops =
            MIN_OVERLAP_HOPS.max((3.0 * sample_rate / frame_size as f64).round() as usize) as i64;
        let (m, n) = (ref_rms.len() as i64, tgt_rms.len() as i64);
        correlation.retain(|(lag, _)| (m - 0.max(-*lag)).min(n - 0.max(*lag)) >= min_overlap_hops);
        let max_lag = max_offset_seconds.map(|s| (s / seconds_per_rms_frame).ceil() as i64);

        // 2. Seeded pass: only the ±window around the date-implied lag. Note
        // the seed's CENTER is not clamped to the global window — only the
        // window's width is capped (Swift `min(maxLag, seedWindow)`).
        if let Some(seed) = seed {
            let center = (seed.center_seconds / seconds_per_rms_frame).round() as i64;
            let mut width = ((seed.window_seconds / seconds_per_rms_frame).round() as i64).max(1);
            if let Some(cap) = max_lag {
                width = width.min(cap.max(1));
            }
            let windowed: Vec<(i64, f64)> = correlation
                .iter()
                .copied()
                .filter(|(lag, _)| (lag - center).abs() <= width)
                .collect();
            if let Some((lag, peak_val, _)) = Self::find_peak(&windowed) {
                let confidence = peak_val.clamp(0.0, 1.0);
                if confidence >= min_confidence {
                    return Some(Self::offset_at(lag, confidence, seconds_per_rms_frame, project_fps));
                }
            }
        }

        // 3. Global window + peak — identical to an unseeded run.
        if let Some(cap) = max_lag {
            correlation.retain(|(lag, _)| lag.abs() <= cap);
        }
        let (peak_lag, _, confidence) = Self::find_peak(&correlation)?;
        Some(Self::offset_at(
            peak_lag,
            confidence,
            seconds_per_rms_frame,
            project_fps,
        ))
    }

    /// Convert a peak lag (in RMS hops) to a project-frame offset.
    /// offset in project frames = (lag * frame_size / sample_rate) * project_fps
    fn offset_at(
        peak_lag: i64,
        confidence: f64,
        seconds_per_rms_frame: f64,
        project_fps: f64,
    ) -> SyncOffset {
        SyncOffset {
            offset_frames: (peak_lag as f64 * seconds_per_rms_frame * project_fps).round() as i64,
            confidence,
            peak_lag_frames: peak_lag,
        }
    }

    /// Find the peak in a correlation array.
    /// Returns (lag, correlation_value, confidence).
    fn find_peak(correlation: &[(i64, f64)]) -> Option<(i64, f64, f64)> {
        if correlation.is_empty() {
            return None;
        }

        let peak = correlation
            .iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))?;

        let lag = peak.0;
        let peak_val = peak.1;

        // Confidence: how much the peak stands above the rest
        // metric: (peak - mean) / (peak - min) when values differ enough
        let sum: f64 = correlation.iter().map(|(_, v)| v).sum();
        let mean = sum / correlation.len() as f64;
        let min_val = correlation
            .iter()
            .map(|(_, v)| *v)
            .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(0.0);
        let max_val = correlation
            .iter()
            .map(|(_, v)| *v)
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(0.0);

        let range = max_val - min_val;
        let confidence = if range > 1e-12 {
            ((peak_val - mean) / (max_val - min_val)).clamp(0.0, 1.0)
        } else {
            0.0
        };

        Some((lag, peak_val, confidence))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RATE: f64 = 48000.0;
    const FRAME_SIZE: usize = 1024;

    // ── RMS envelope extraction ──

    #[test]
    fn rms_silent_signal_returns_zero() {
        let samples = vec![0.0; 48000];
        let frames = AudioSyncCorrelator::extract_rms_envelope(&samples, SAMPLE_RATE, FRAME_SIZE);
        assert!(!frames.is_empty());
        for frame in &frames {
            assert!(
                frame.rms < 1e-12,
                "expected near-zero RMS, got {}",
                frame.rms
            );
        }
    }

    #[test]
    fn rms_constant_signal_returns_expected() {
        let amplitude = 0.5;
        let samples = vec![amplitude; 48000];
        let frames = AudioSyncCorrelator::extract_rms_envelope(&samples, SAMPLE_RATE, FRAME_SIZE);
        assert!(!frames.is_empty());
        let expected_rms = amplitude; // constant signal: RMS = abs(amplitude)
        for frame in &frames {
            let diff = (frame.rms - expected_rms).abs();
            assert!(
                diff < 1e-12,
                "expected RMS {expected_rms}, got {}",
                frame.rms
            );
        }
    }

    #[test]
    fn rms_frame_count_correct() {
        let num_samples = 48000usize;
        let samples = vec![0.5; num_samples];
        let frames = AudioSyncCorrelator::extract_rms_envelope(&samples, SAMPLE_RATE, FRAME_SIZE);
        let expected = num_samples.div_ceil(FRAME_SIZE);
        assert_eq!(frames.len(), expected, "frame count mismatch");
    }

    #[test]
    fn rms_time_seconds_correct() {
        let num_samples = 48000usize;
        let samples = vec![0.5; num_samples];
        let frames = AudioSyncCorrelator::extract_rms_envelope(&samples, SAMPLE_RATE, FRAME_SIZE);
        let seconds_per_frame = FRAME_SIZE as f64 / SAMPLE_RATE;
        for (i, frame) in frames.iter().enumerate() {
            let expected = i as f64 * seconds_per_frame;
            let diff = (frame.time_seconds - expected).abs();
            assert!(
                diff < 1e-12,
                "at index {i}: expected {expected}s, got {}s",
                frame.time_seconds
            );
        }
    }

    // ── Cross-correlation ──

    #[test]
    fn cross_corr_identical_peaks_at_zero() {
        let samples = make_sine_wave(48000, 440.0, SAMPLE_RATE);
        let rms = AudioSyncCorrelator::extract_rms_envelope(&samples, SAMPLE_RATE, FRAME_SIZE);
        let corr = AudioSyncCorrelator::cross_correlate(&rms, &rms);
        assert!(!corr.is_empty());

        let (lag, max_corr) = corr
            .iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .unwrap();
        assert_eq!(*lag, 0, "identical signal should peak at lag 0");
        assert!(
            *max_corr > 0.999,
            "self-correlation should be ~1.0, got {max_corr}"
        );
    }

    #[test]
    fn cross_corr_shifted_signal_peaks_at_shift() {
        // Use a shift that is an exact multiple of FRAME_SIZE so that
        // the RMS-frame boundary doesn't split the noise onset.
        let samples = make_noise(96000);
        let shift_samples = 5 * FRAME_SIZE; // 5120 samples
        let shifted: Vec<f64> = std::iter::repeat_n(0.0, shift_samples)
            .chain(samples.iter().copied())
            .take(samples.len())
            .collect();

        let ref_rms = AudioSyncCorrelator::extract_rms_envelope(&samples, SAMPLE_RATE, FRAME_SIZE);
        let tgt_rms = AudioSyncCorrelator::extract_rms_envelope(&shifted, SAMPLE_RATE, FRAME_SIZE);

        let corr = AudioSyncCorrelator::cross_correlate(&ref_rms, &tgt_rms);

        // Target is shifted right (silence first), so reference is ahead → positive lag
        let (peak_lag, _) = corr
            .iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .unwrap();

        // Expected lag in RMS frames: shift_samples / FRAME_SIZE = 5 (exact)
        let expected_lag = (shift_samples / FRAME_SIZE) as i64;
        assert_eq!(
            *peak_lag, expected_lag,
            "expected peak at lag {expected_lag}, got {peak_lag}"
        );
    }

    #[test]
    fn cross_corr_silent_signal_flat() {
        let signal = make_sine_wave(48000, 440.0, SAMPLE_RATE);
        let silent = vec![0.0; 48000];
        let ref_rms = AudioSyncCorrelator::extract_rms_envelope(&signal, SAMPLE_RATE, FRAME_SIZE);
        let tgt_rms = AudioSyncCorrelator::extract_rms_envelope(&silent, SAMPLE_RATE, FRAME_SIZE);

        let corr = AudioSyncCorrelator::cross_correlate(&ref_rms, &tgt_rms);
        assert!(!corr.is_empty());

        // All correlation values should be near zero
        for (_, val) in &corr {
            assert!(
                val.abs() < 1e-10,
                "expected near-zero correlation with silent signal, got {val}"
            );
        }
    }

    // ── Sync offset ──

    #[test]
    fn find_sync_offset_identical_signals() {
        // Noise, not a sine: a 440 Hz sine's RMS envelope repeats exactly every
        // 75 hops, so long signals tie at 1.0 on aliased lags.
        let samples = make_noise(8 * SAMPLE_RATE as usize);
        let result = AudioSyncCorrelator::find_sync_offset(
            &samples,
            &samples,
            SAMPLE_RATE,
            FRAME_SIZE,
            30.0,
        );
        assert!(
            result.is_some(),
            "expected Some result for identical signals"
        );
        let offset = result.unwrap();
        assert_eq!(
            offset.offset_frames, 0,
            "identical signals should have offset 0"
        );
        assert!(
            offset.confidence > 0.5,
            "confidence should be high for identical signals"
        );
    }

    #[test]
    fn find_sync_offset_shifted_signals() {
        let samples = make_noise(8 * SAMPLE_RATE as usize);
        let shift_samples = 5 * FRAME_SIZE; // 5120 samples = 5 RMS frames
        let shifted: Vec<f64> = std::iter::repeat_n(0.0, shift_samples)
            .chain(samples.iter().copied())
            .take(samples.len())
            .collect();

        let result = AudioSyncCorrelator::find_sync_offset(
            &samples,
            &shifted,
            SAMPLE_RATE,
            FRAME_SIZE,
            30.0,
        );
        assert!(result.is_some(), "expected Some result for shifted signals");

        let offset = result.unwrap();
        // 5120/48000 = 0.10666...s, at 30fps = 3.2 → round to 3 frames
        assert_eq!(offset.offset_frames, 3);
        assert!(offset.confidence > 0.0);
    }

    #[test]
    fn find_sync_offset_rejects_sub_three_second_overlap() {
        // Two 2-second signals: no lag can reach the 3-second overlap floor.
        let samples = make_noise(2 * SAMPLE_RATE as usize);
        let result = AudioSyncCorrelator::find_sync_offset(
            &samples,
            &samples,
            SAMPLE_RATE,
            FRAME_SIZE,
            30.0,
        );
        assert!(
            result.is_none(),
            "signals too short for the min-overlap floor must return None, got {result:?}"
        );
    }

    #[test]
    fn find_sync_offset_thin_edge_lag_cannot_win() {
        // Piecewise-constant per-RMS-frame amplitudes: envelope == amplitude
        // exactly. Two unrelated envelopes are rigged so a 2-hop edge overlap
        // (ref tail rising, tgt head rising) correlates at a perfect +1.0 —
        // the exact spurious match #269 guards against.
        let hops = 375usize; // 8 s at 48000/1024
        let env = |seed: f64, k: usize| 0.3 + 0.25 * ((k as f64 * seed).sin().abs());
        let mut ref_env: Vec<f64> = (0..hops).map(|k| env(0.731, k)).collect();
        let mut tgt_env: Vec<f64> = (0..hops).map(|k| env(1.917, k)).collect();
        ref_env[hops - 2] = 0.2;
        ref_env[hops - 1] = 0.9;
        tgt_env[0] = 0.3;
        tgt_env[1] = 0.95;
        let expand = |e: &[f64]| -> Vec<f64> {
            e.iter()
                .flat_map(|&a| std::iter::repeat_n(a, FRAME_SIZE))
                .collect()
        };

        let result = AudioSyncCorrelator::find_sync_offset(
            &expand(&ref_env),
            &expand(&tgt_env),
            SAMPLE_RATE,
            FRAME_SIZE,
            30.0,
        )
        .expect("long signals still produce a result");

        let min_overlap_hops =
            MIN_OVERLAP_HOPS.max((3.0 * SAMPLE_RATE / FRAME_SIZE as f64).round() as usize);
        let bound = (hops - min_overlap_hops) as i64;
        assert!(
            result.peak_lag_frames.abs() <= bound,
            "thin-edge lag won: |{}| > {bound}",
            result.peak_lag_frames
        );
    }

    // ── Capture-date seeded search (upstream #269) ──

    /// Per-hop white-ish envelope via an LCG: aperiodic, so wrong-lag
    /// correlations stay near zero (unlike sine-mix noise).
    fn lcg_env(seed: u64, hops: usize) -> Vec<f64> {
        let mut x = seed;
        (0..hops)
            .map(|_| {
                x = x
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                0.2 + 0.6 * ((x >> 33) as f64 / (1u64 << 31) as f64)
            })
            .collect()
    }

    fn expand_env(env: &[f64]) -> Vec<f64> {
        env.iter()
            .flat_map(|&a| std::iter::repeat_n(a, FRAME_SIZE))
            .collect()
    }

    #[test]
    fn good_seed_selects_the_seeded_alignment_over_a_stronger_global_peak() {
        // The reference holds the target's content twice: an exact copy at its
        // head (global winner, Pearson 1.0) and a degraded copy 8s in. A seed
        // pointing at the second copy must pick it — proving the seeded pass
        // searches ONLY the ±3s window around the seed.
        let hops = 375; // 8 s at 48000/1024
        let burst = lcg_env(7, hops);
        let other = lcg_env(99, hops);
        let degraded: Vec<f64> = burst
            .iter()
            .zip(other.iter())
            .map(|(&b, &o)| 0.8 * b + 0.2 * o)
            .collect();
        let mut ref_env = burst.clone();
        ref_env.extend_from_slice(&degraded);
        let reference = expand_env(&ref_env);
        let target = expand_env(&burst);

        let unseeded = AudioSyncCorrelator::find_sync_offset_windowed(
            &reference,
            &target,
            SAMPLE_RATE,
            FRAME_SIZE,
            30.0,
            Some(30.0),
        )
        .expect("global result");
        assert_eq!(unseeded.offset_frames, 0, "global search picks the exact head copy");

        // Rust sign convention: negative = content later in the REFERENCE.
        let seeded = AudioSyncCorrelator::find_sync_offset_seeded(
            &reference,
            &target,
            SAMPLE_RATE,
            FRAME_SIZE,
            30.0,
            Some(30.0),
            Some(SyncSeed {
                center_seconds: -8.0,
                window_seconds: DATE_SEED_WINDOW_SECONDS,
            }),
            0.5,
        )
        .expect("seeded result");
        // -375 hops * (1024/48000)s * 30fps = -240 frames exactly.
        assert_eq!(seeded.peak_lag_frames, -(hops as i64));
        assert_eq!(seeded.offset_frames, -240);
        assert!(
            seeded.confidence >= 0.5,
            "degraded copy still clears the gate: {}",
            seeded.confidence
        );
    }

    #[test]
    fn bad_seed_falls_back_to_global_and_matches_unseeded() {
        // Spec scenario: the capture-date seed points far from the true
        // alignment — the global fallback must return the SAME offset as an
        // unseeded run.
        let hops = 750; // 16 s
        let env = lcg_env(3, hops);
        let mut tgt_env = vec![0.0; 5];
        tgt_env.extend_from_slice(&env[..hops - 5]);
        let reference = expand_env(&env);
        let target = expand_env(&tgt_env);

        let unseeded = AudioSyncCorrelator::find_sync_offset_windowed(
            &reference,
            &target,
            SAMPLE_RATE,
            FRAME_SIZE,
            30.0,
            Some(30.0),
        )
        .expect("global result");
        assert_eq!(unseeded.offset_frames, 3, "5 hops * 0.02133s * 30fps rounds to 3");

        let seeded = AudioSyncCorrelator::find_sync_offset_seeded(
            &reference,
            &target,
            SAMPLE_RATE,
            FRAME_SIZE,
            30.0,
            Some(30.0),
            Some(SyncSeed {
                center_seconds: -10.0,
                window_seconds: DATE_SEED_WINDOW_SECONDS,
            }),
            0.5,
        )
        .expect("fallback result");
        assert_eq!(seeded, unseeded, "weak seeded window falls back to the unseeded result");
    }

    #[test]
    fn good_seed_agrees_with_global_on_a_plain_shift() {
        let hops = 750;
        let env = lcg_env(11, hops);
        let mut tgt_env = vec![0.0; 5];
        tgt_env.extend_from_slice(&env[..hops - 5]);
        let reference = expand_env(&env);
        let target = expand_env(&tgt_env);

        let seeded = AudioSyncCorrelator::find_sync_offset_seeded(
            &reference,
            &target,
            SAMPLE_RATE,
            FRAME_SIZE,
            30.0,
            Some(30.0),
            Some(SyncSeed {
                // 5 hops = 0.1067s; a nearby (imprecise) seed still hits.
                center_seconds: 0.1,
                window_seconds: DATE_SEED_WINDOW_SECONDS,
            }),
            0.5,
        )
        .expect("seeded result");
        assert_eq!(seeded.peak_lag_frames, 5);
        assert_eq!(seeded.offset_frames, 3);
        assert!(seeded.confidence >= 0.5);
    }

    #[test]
    fn find_sync_offset_too_short() {
        let short = vec![0.5; 512];
        let result =
            AudioSyncCorrelator::find_sync_offset(&short, &short, SAMPLE_RATE, FRAME_SIZE, 30.0);
        assert!(result.is_none(), "too-short signals should return None");
    }

    #[test]
    fn find_sync_offset_project_fps_conversion() {
        let samples = make_noise(8 * SAMPLE_RATE as usize);
        let shift_samples = 5 * FRAME_SIZE; // 5120 samples = 5 RMS frames
        let shifted: Vec<f64> = std::iter::repeat_n(0.0, shift_samples)
            .chain(samples.iter().copied())
            .take(samples.len())
            .collect();

        // 5120/48000 = 0.10666...s
        // At 24 fps: 0.10666 * 24 = 2.56 → round to 3
        let result_24 = AudioSyncCorrelator::find_sync_offset(
            &samples,
            &shifted,
            SAMPLE_RATE,
            FRAME_SIZE,
            24.0,
        );
        assert!(result_24.is_some());
        assert_eq!(result_24.unwrap().offset_frames, 3);

        // At 60 fps: 0.10666 * 60 = 6.4 → round to 6
        let result_60 = AudioSyncCorrelator::find_sync_offset(
            &samples,
            &shifted,
            SAMPLE_RATE,
            FRAME_SIZE,
            60.0,
        );
        assert!(result_60.is_some());
        assert_eq!(result_60.unwrap().offset_frames, 6);
    }

    // ── Peak finding ──

    #[test]
    fn find_peak_returns_highest_correlation() {
        let corr = vec![(-2, 0.1), (-1, 0.3), (0, 0.9), (1, 0.4), (2, 0.2)];
        let result = AudioSyncCorrelator::find_peak(&corr);
        assert!(result.is_some());
        let (lag, val, _) = result.unwrap();
        assert_eq!(lag, 0);
        assert!((val - 0.9).abs() < 1e-12);
    }

    #[test]
    fn find_peak_empty_correlation() {
        let corr: Vec<(i64, f64)> = vec![];
        let result = AudioSyncCorrelator::find_peak(&corr);
        assert!(result.is_none());
    }

    // ── Helpers ──

    fn make_sine_wave(num_samples: usize, freq: f64, sample_rate: f64) -> Vec<f64> {
        let amplitude = 0.5;
        (0..num_samples)
            .map(|i| {
                let t = i as f64 / sample_rate;
                amplitude * (2.0 * std::f64::consts::PI * freq * t).sin()
            })
            .collect()
    }

    /// Deterministic pseudo-noise signal with a flat envelope but aperiodic content.
    fn make_noise(num_samples: usize) -> Vec<f64> {
        (0..num_samples)
            .map(|i| {
                let x = i as f64 * 0.137;
                ((x * std::f64::consts::TAU).sin()
                    + (x * 2.71 * std::f64::consts::PI).cos()
                    + (x * 0.37 * std::f64::consts::TAU).sin())
                    * 0.3
            })
            .collect()
    }
}

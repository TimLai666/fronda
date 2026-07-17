//! Silero VAD (v5 ONNX, MIT — snakers4/silero-vad `src/silero_vad/data/silero_vad.onnx`)
//! behind the `vad` feature: decode 16 kHz mono via ffmpeg, run the model per
//! 512-sample window with state+context carry, post-process probabilities into
//! speech spans with the official `get_speech_timestamps` defaults, and serve
//! them through the `SpeechAnalyzer` seam (`remove_silence`'s span path).

use agent_contract::tool_exec::{SpeechAnalyzer, SpeechSpan};
use core_model::MediaSource;
use ort::session::Session;
use ort::value::Tensor;
use std::path::PathBuf;
use std::sync::Mutex;

static SILERO_VAD_ONNX: &[u8] = include_bytes!("../assets/models/silero_vad.onnx");

pub const VAD_SAMPLE_RATE: u32 = 16_000;
const WINDOW_SAMPLES: usize = 512;
const CONTEXT_SAMPLES: usize = 64;
const STATE_LEN: usize = 2 * 128;

// silero-vad `get_speech_timestamps` defaults.
const THRESHOLD: f32 = 0.5;
const NEG_THRESHOLD: f32 = THRESHOLD - 0.15;
const MIN_SPEECH_SECONDS: f64 = 0.25;
const MIN_SILENCE_SECONDS: f64 = 0.10;
const SPEECH_PAD_SECONDS: f64 = 0.03;

pub struct SileroVad {
    session: Session,
}

impl SileroVad {
    pub fn new(model_bytes: &[u8]) -> Result<Self, String> {
        let builder = Session::builder().map_err(|e| format!("silero session: {e}"))?;
        let mut builder = builder
            .with_intra_threads(1)
            .map_err(|e| format!("silero session: {e}"))?;
        let session = builder
            .commit_from_memory(model_bytes)
            .map_err(|e| format!("silero session: {e}"))?;
        Ok(Self { session })
    }

    pub fn bundled() -> Result<Self, String> {
        Self::new(SILERO_VAD_ONNX)
    }

    /// Detected speech spans in seconds over 16 kHz mono PCM.
    pub fn analyze(&mut self, pcm_16k_mono: &[f32]) -> Result<Vec<(f64, f64)>, String> {
        let probs = self.speech_probabilities(pcm_16k_mono)?;
        Ok(spans_from_probs(&probs, pcm_16k_mono.len()))
    }

    /// One speech probability per 512-sample window. The v5 model takes
    /// `input` [1, 576] (64 carried context samples + the window), `state`
    /// [2, 1, 128], scalar i64 `sr`; returns `output` [1, 1] and `stateN`.
    pub fn speech_probabilities(&mut self, pcm: &[f32]) -> Result<Vec<f32>, String> {
        let mut state = vec![0f32; STATE_LEN];
        let mut context = vec![0f32; CONTEXT_SAMPLES];
        let mut probs = Vec::with_capacity(pcm.len().div_ceil(WINDOW_SAMPLES));
        for chunk in pcm.chunks(WINDOW_SAMPLES) {
            let mut input = Vec::with_capacity(CONTEXT_SAMPLES + WINDOW_SAMPLES);
            input.extend_from_slice(&context);
            input.extend_from_slice(chunk);
            input.resize(CONTEXT_SAMPLES + WINDOW_SAMPLES, 0.0);
            context.copy_from_slice(&input[input.len() - CONTEXT_SAMPLES..]);

            let outputs = self
                .session
                .run(ort::inputs![
                    "input" => Tensor::from_array((vec![1i64, (CONTEXT_SAMPLES + WINDOW_SAMPLES) as i64], input)).map_err(|e| e.to_string())?,
                    "state" => Tensor::from_array((vec![2i64, 1, 128], state.clone())).map_err(|e| e.to_string())?,
                    "sr" => Tensor::from_array(((), vec![i64::from(VAD_SAMPLE_RATE)])).map_err(|e| e.to_string())?,
                ])
                .map_err(|e| format!("silero run: {e}"))?;

            let (_, out) = outputs["output"]
                .try_extract_tensor::<f32>()
                .map_err(|e| e.to_string())?;
            probs.push(*out.first().ok_or("silero: empty output")?);
            let (_, next_state) = outputs["stateN"]
                .try_extract_tensor::<f32>()
                .map_err(|e| e.to_string())?;
            if next_state.len() != STATE_LEN {
                return Err(format!("silero: unexpected state length {}", next_state.len()));
            }
            state.copy_from_slice(next_state);
        }
        Ok(probs)
    }
}

/// Window probabilities → merged speech spans in seconds, mirroring silero's
/// `get_speech_timestamps` (default params, `max_speech_duration_s` = inf).
fn spans_from_probs(probs: &[f32], total_samples: usize) -> Vec<(f64, f64)> {
    let sr = f64::from(VAD_SAMPLE_RATE);
    let min_speech = (MIN_SPEECH_SECONDS * sr) as usize;
    let min_silence = (MIN_SILENCE_SECONDS * sr) as usize;
    let pad = (SPEECH_PAD_SECONDS * sr) as usize;

    let mut speeches: Vec<(usize, usize)> = Vec::new();
    let mut triggered = false;
    let mut current_start = 0usize;
    let mut temp_end = 0usize;
    for (i, &p) in probs.iter().enumerate() {
        let cur = i * WINDOW_SAMPLES;
        if p >= THRESHOLD && temp_end != 0 {
            temp_end = 0;
        }
        if p >= THRESHOLD && !triggered {
            triggered = true;
            current_start = cur;
            continue;
        }
        if p < NEG_THRESHOLD && triggered {
            if temp_end == 0 {
                temp_end = cur;
            }
            if cur - temp_end >= min_silence {
                if temp_end - current_start > min_speech {
                    speeches.push((current_start, temp_end));
                }
                temp_end = 0;
                triggered = false;
            }
        }
    }
    if triggered && total_samples > current_start && total_samples - current_start > min_speech {
        speeches.push((current_start, total_samples));
    }

    // Official edge padding: half-split gaps narrower than 2*pad.
    let n = speeches.len();
    for i in 0..n {
        if i == 0 {
            speeches[i].0 = speeches[i].0.saturating_sub(pad);
        }
        if i + 1 < n {
            let gap = speeches[i + 1].0 - speeches[i].1;
            if gap < 2 * pad {
                speeches[i].1 += gap / 2;
                speeches[i + 1].0 = speeches[i + 1].0.saturating_sub(gap / 2);
            } else {
                speeches[i].1 = (speeches[i].1 + pad).min(total_samples);
                speeches[i + 1].0 = speeches[i + 1].0.saturating_sub(pad);
            }
        } else {
            speeches[i].1 = (speeches[i].1 + pad).min(total_samples);
        }
    }

    speeches
        .into_iter()
        .map(|(s, e)| (s as f64 / sr, e as f64 / sr))
        .collect()
}

enum VadState {
    Unloaded,
    Ready(Box<SileroVad>),
    Failed,
}

/// `SpeechAnalyzer` host implementation: resolve the source against the open
/// project root, decode 16 kHz mono via ffmpeg, run Silero. Session build is
/// lazy; any failure logs once and degrades to `None` (RMS fallback).
pub struct VadSpeechAnalyzer {
    project_root: PathBuf,
    vad: Mutex<VadState>,
}

impl VadSpeechAnalyzer {
    pub fn new(project_root: PathBuf) -> Self {
        Self {
            project_root,
            vad: Mutex::new(VadState::Unloaded),
        }
    }

    fn resolve_path(&self, source: &MediaSource) -> PathBuf {
        match source {
            MediaSource::External { absolute_path } => PathBuf::from(absolute_path),
            MediaSource::Project { relative_path } => self.project_root.join(relative_path),
        }
    }
}

impl SpeechAnalyzer for VadSpeechAnalyzer {
    fn analyze(&self, source: &MediaSource, _sample_rate: u32) -> Option<Vec<SpeechSpan>> {
        let pcm =
            crate::audio_export::decode_audio_pcm(&self.resolve_path(source), VAD_SAMPLE_RATE, 1)?;
        let mut guard = self.vad.lock().ok()?;
        if matches!(*guard, VadState::Unloaded) {
            *guard = match SileroVad::bundled() {
                Ok(v) => VadState::Ready(Box::new(v)),
                Err(e) => {
                    eprintln!("VAD unavailable, falling back to RMS: {e}");
                    VadState::Failed
                }
            };
        }
        let VadState::Ready(vad) = &mut *guard else {
            return None;
        };
        match vad.analyze(&pcm) {
            Ok(spans) => Some(
                spans
                    .into_iter()
                    .map(|(start_seconds, end_seconds)| SpeechSpan {
                        start_seconds,
                        end_seconds,
                    })
                    .collect(),
            ),
            Err(e) => {
                eprintln!("VAD analysis failed, falling back to RMS: {e}");
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_signature_is_the_expected_v5_shape() {
        let vad = SileroVad::bundled().expect("bundled model builds a session");
        let inputs: Vec<&str> = vad.session.inputs().iter().map(|o| o.name()).collect();
        let outputs: Vec<&str> = vad.session.outputs().iter().map(|o| o.name()).collect();
        assert_eq!(inputs, vec!["input", "state", "sr"], "model inputs");
        assert_eq!(outputs, vec!["output", "stateN"], "model outputs");
    }

    /// Speech-like synthesis: harmonics of a wavering ~110Hz F0 shaped by
    /// vowel formants, 5Hz syllabic amplitude modulation, a whiff of noise.
    fn synth_speech(seconds: f64) -> Vec<f32> {
        let sr = f64::from(VAD_SAMPLE_RATE);
        let n = (seconds * sr) as usize;
        let formants = [(700.0, 1.0), (1200.0, 0.7), (2600.0, 0.4)];
        let mut noise_state = 0x2545F491_4F6CDD1Du64;
        (0..n)
            .map(|i| {
                let t = i as f64 / sr;
                let f0 = 110.0 + 20.0 * (2.0 * std::f64::consts::PI * 0.8 * t).sin();
                let mut s = 0.0;
                for h in 1..=24 {
                    let freq = f0 * h as f64;
                    if freq > 3400.0 {
                        break;
                    }
                    let weight: f64 = formants
                        .iter()
                        .map(|&(fc, a)| a * (-((freq - fc) / 350.0).powi(2)).exp())
                        .sum();
                    s += weight * (2.0 * std::f64::consts::PI * freq * t).sin();
                }
                let syllable = 0.55 + 0.45 * (2.0 * std::f64::consts::PI * 5.0 * t).sin();
                noise_state ^= noise_state << 13;
                noise_state ^= noise_state >> 7;
                noise_state ^= noise_state << 17;
                let noise = (noise_state as f64 / u64::MAX as f64 - 0.5) * 0.02;
                ((s * 0.28 * syllable) + noise) as f32
            })
            .collect()
    }

    fn silence(seconds: f64) -> Vec<f32> {
        vec![0.0; (seconds * f64::from(VAD_SAMPLE_RATE)) as usize]
    }

    #[test]
    fn all_silence_yields_no_spans() {
        let mut vad = SileroVad::bundled().unwrap();
        let spans = vad.analyze(&silence(3.0)).unwrap();
        assert!(spans.is_empty(), "silence produced spans: {spans:?}");
    }

    #[test]
    fn speech_and_silence_spans_have_sane_bounds() {
        // 1.0s silence | 2.0s speech | 1.5s silence | 1.0s speech | 1.0s silence
        let mut pcm = silence(1.0);
        pcm.extend(synth_speech(2.0));
        pcm.extend(silence(1.5));
        pcm.extend(synth_speech(1.0));
        pcm.extend(silence(1.0));

        let mut vad = SileroVad::bundled().unwrap();
        let spans = vad.analyze(&pcm).unwrap();
        assert!(!spans.is_empty(), "synthetic speech went undetected");

        // Every span stays inside a true speech segment ± 0.35s slack, and
        // never bleeds into the deep-silence middle.
        let voiced = [(0.65, 3.35), (4.15, 5.85)];
        for &(s, e) in &spans {
            assert!(e > s, "degenerate span ({s}, {e})");
            assert!(
                voiced.iter().any(|&(vs, ve)| s >= vs && e <= ve),
                "span ({s:.2}, {e:.2}) escapes the voiced segments"
            );
        }
        let covered: f64 = spans.iter().map(|&(s, e)| e - s).sum();
        assert!(covered >= 1.5, "spans cover only {covered:.2}s of ~3s speech");
    }

    #[test]
    fn spans_from_probs_thresholds_min_silence_and_min_speech() {
        // 20 hot windows (~0.64s) then cold: one span from 0, padded end.
        let mut probs = vec![0.9f32; 20];
        probs.extend(vec![0.05f32; 20]);
        let total = 40 * WINDOW_SAMPLES;
        let spans = spans_from_probs(&probs, total);
        assert_eq!(spans.len(), 1);
        let (s, e) = spans[0];
        assert_eq!(s, 0.0);
        let expected_end = (20 * WINDOW_SAMPLES) as f64 / 16_000.0 + SPEECH_PAD_SECONDS;
        assert!((e - expected_end).abs() < 1e-9, "end {e} != {expected_end}");

        // A sub-min-silence dip does not split the span.
        let mut dipped = vec![0.9f32; 10];
        dipped.extend(vec![0.05f32; 2]); // ~64ms < 100ms min silence
        dipped.extend(vec![0.9f32; 10]);
        let spans = spans_from_probs(&dipped, 22 * WINDOW_SAMPLES);
        assert_eq!(spans.len(), 1, "sub-min-silence dip split the span");

        // Speech shorter than min_speech (250ms) is dropped.
        let mut short = vec![0.9f32; 3]; // ~96ms
        short.extend(vec![0.05f32; 40]);
        assert!(spans_from_probs(&short, 43 * WINDOW_SAMPLES).is_empty());

        // All-cold yields nothing.
        assert!(spans_from_probs(&[0.1; 30], 30 * WINDOW_SAMPLES).is_empty());
    }

    #[test]
    fn spans_from_probs_hysteresis_keeps_mid_probs_inside_speech() {
        // Between NEG_THRESHOLD and THRESHOLD the trigger state holds.
        let mut probs = vec![0.9f32; 10];
        probs.extend(vec![0.4f32; 10]); // above 0.35: still speech
        probs.extend(vec![0.05f32; 20]);
        let spans = spans_from_probs(&probs, 40 * WINDOW_SAMPLES);
        assert_eq!(spans.len(), 1);
        let expected_end = (20 * WINDOW_SAMPLES) as f64 / 16_000.0 + SPEECH_PAD_SECONDS;
        assert!((spans[0].1 - expected_end).abs() < 1e-9);
    }

    #[test]
    fn speech_analyzer_seam_decodes_and_analyzes_wav() {
        let dir = std::env::temp_dir().join("fronda-vad-seam");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("speech.wav");
        let mut pcm = silence(0.5);
        pcm.extend(synth_speech(1.5));
        pcm.extend(silence(0.5));
        audio_core::wav::write_wav(&path, &pcm, VAD_SAMPLE_RATE, 1).unwrap();

        let analyzer = VadSpeechAnalyzer::new(dir.clone());
        let source = MediaSource::External {
            absolute_path: path.to_string_lossy().to_string(),
        };
        let spans = analyzer
            .analyze(&source, 44_100)
            .expect("wav decodes and analyzes");
        assert!(!spans.is_empty(), "speech in the wav went undetected");
        for span in &spans {
            assert!(span.start_seconds >= 0.15 && span.end_seconds <= 2.35, "{span:?}");
        }

        let missing = MediaSource::External {
            absolute_path: dir.join("missing.wav").to_string_lossy().to_string(),
        };
        assert!(analyzer.analyze(&missing, 44_100).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

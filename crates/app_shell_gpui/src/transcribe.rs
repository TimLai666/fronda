//! Local whisper transcription (`transcribe-local` feature, decision D4):
//! whisper.cpp via whisper-rs serves the `TranscriptionProvider` seam. The
//! model is user-supplied — `whisperModelPath` in preferences.json points at a
//! GGML/GGUF file; a missing path reports transcription unavailable (honest,
//! no bundled fallback). Word granularity via token timestamps + split-on-word
//! + max_len 1 (each whisper segment ≈ one word).

use agent_contract::tool_exec::{TranscriptionProvider, WordStamp};
use core_model::MediaSource;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

pub const WHISPER_MODEL_PATH_KEY: &str = "whisperModelPath";
pub const WHISPER_SAMPLE_RATE: u32 = 16_000;

/// `whisperModelPath` from preferences.json. Missing file, unreadable JSON,
/// missing key, or a non-string/empty value → `None` (pane_prefs convention).
pub fn load_whisper_model_path(prefs_path: &Path) -> Option<PathBuf> {
    let text = std::fs::read_to_string(prefs_path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    let raw = value.get(WHISPER_MODEL_PATH_KEY)?.as_str()?.trim();
    (!raw.is_empty()).then(|| PathBuf::from(raw))
}

/// Timeline language → whisper language id: the primary BCP-47 subtag,
/// lowercased. `None`, empty, or "auto" → `None` (whisper auto-detect).
pub fn whisper_language(language: Option<&str>) -> Option<String> {
    let lang = language?.trim();
    let primary = lang.split('-').next().unwrap_or(lang).to_ascii_lowercase();
    (!primary.is_empty() && primary != "auto").then_some(primary)
}

/// Whisper segments (text, t0, t1 in centiseconds) → word stamps in seconds.
/// Trims whitespace, drops empty segments and non-speech markers whisper wraps
/// in brackets/parens ("[BLANK_AUDIO]", "(music)"), clamps end ≥ start.
pub fn stamps_from_segments(segments: &[(String, i64, i64)]) -> Vec<WordStamp> {
    segments
        .iter()
        .filter_map(|(text, t0, t1)| {
            let word = text.trim();
            if word.is_empty() || is_noise_marker(word) {
                return None;
            }
            let start_seconds = *t0 as f64 * 0.01;
            Some(WordStamp {
                word: word.to_string(),
                start_seconds,
                end_seconds: (*t1 as f64 * 0.01).max(start_seconds),
            })
        })
        .collect()
}

fn is_noise_marker(word: &str) -> bool {
    (word.starts_with('[') && word.ends_with(']'))
        || (word.starts_with('(') && word.ends_with(')'))
}

/// `TranscriptionProvider` host implementation: resolve the source against the
/// open project root, decode 16 kHz mono via ffmpeg, run whisper. The model
/// path is re-read from preferences each call (no restart needed to point at a
/// model); the loaded context is cached until the path changes.
pub struct WhisperTranscriber {
    project_root: PathBuf,
    prefs_path: PathBuf,
    ctx: Mutex<Option<(PathBuf, WhisperContext)>>,
}

impl WhisperTranscriber {
    pub fn new(project_root: PathBuf, prefs_path: PathBuf) -> Self {
        Self {
            project_root,
            prefs_path,
            ctx: Mutex::new(None),
        }
    }

    fn resolve_path(&self, source: &MediaSource) -> PathBuf {
        match source {
            MediaSource::External { absolute_path } => PathBuf::from(absolute_path),
            MediaSource::Project { relative_path } => self.project_root.join(relative_path),
        }
    }
}

impl TranscriptionProvider for WhisperTranscriber {
    fn transcribe(
        &self,
        source: &MediaSource,
        language: Option<&str>,
    ) -> Result<Vec<WordStamp>, String> {
        let model_path = load_whisper_model_path(&self.prefs_path).ok_or_else(|| {
            format!(
                "transcription is unavailable: set {WHISPER_MODEL_PATH_KEY} in preferences.json to a whisper GGML/GGUF model file."
            )
        })?;

        let mut guard = self.ctx.lock().map_err(|_| "whisper state poisoned".to_string())?;
        if guard.as_ref().is_none_or(|(cached, _)| cached != &model_path) {
            let ctx = WhisperContext::new_with_params(
                &model_path,
                WhisperContextParameters::default(),
            )
            .map_err(|e| {
                format!("whisper model load failed ({}): {e}", model_path.display())
            })?;
            *guard = Some((model_path, ctx));
        }
        let (_, ctx) = guard.as_ref().expect("context cached above");

        let media_path = self.resolve_path(source);
        let pcm = crate::audio_export::decode_audio_pcm(&media_path, WHISPER_SAMPLE_RATE, 1)
            .ok_or_else(|| {
                format!("Could not decode audio from '{}'.", media_path.display())
            })?;

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        let lang = whisper_language(language);
        params.set_language(lang.as_deref());
        params.set_token_timestamps(true);
        params.set_split_on_word(true);
        params.set_max_len(1);
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);

        let mut state = ctx
            .create_state()
            .map_err(|e| format!("whisper state creation failed: {e}"))?;
        state
            .full(params, &pcm)
            .map_err(|e| format!("whisper transcription failed: {e}"))?;

        let mut segments = Vec::new();
        for i in 0..state.full_n_segments() {
            let Some(segment) = state.get_segment(i) else {
                continue;
            };
            let text = segment.to_str_lossy().map_err(|e| e.to_string())?.into_owned();
            segments.push((text, segment.start_timestamp(), segment.end_timestamp()));
        }
        Ok(stamps_from_segments(&segments))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use audio_core::wav::write_wav;

    fn temp_prefs(name: &str, contents: Option<&str>) -> PathBuf {
        let dir = std::env::temp_dir().join("fronda-transcribe-tests");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join(name);
        let _ = std::fs::remove_file(&path);
        if let Some(text) = contents {
            std::fs::write(&path, text).unwrap();
        }
        path
    }

    #[test]
    fn model_path_pref_missing_file_key_or_malformed_returns_none() {
        assert_eq!(load_whisper_model_path(&temp_prefs("missing.json", None)), None);
        assert_eq!(
            load_whisper_model_path(&temp_prefs("corrupt.json", Some("{not json"))),
            None
        );
        assert_eq!(
            load_whisper_model_path(&temp_prefs(
                "no-key.json",
                Some(r#"{"mcpServerEnabled": true}"#)
            )),
            None
        );
        assert_eq!(
            load_whisper_model_path(&temp_prefs(
                "non-string.json",
                Some(r#"{"whisperModelPath": 7}"#)
            )),
            None
        );
        assert_eq!(
            load_whisper_model_path(&temp_prefs(
                "empty.json",
                Some(r#"{"whisperModelPath": "  "}"#)
            )),
            None
        );
    }

    #[test]
    fn model_path_pref_reads_the_key() {
        let path = temp_prefs(
            "set.json",
            Some(r#"{"whisperModelPath": "/models/ggml-base.bin", "paneVisibility": {}}"#),
        );
        assert_eq!(
            load_whisper_model_path(&path),
            Some(PathBuf::from("/models/ggml-base.bin"))
        );
    }

    #[test]
    fn whisper_language_maps_bcp47_and_auto() {
        assert_eq!(whisper_language(None), None);
        assert_eq!(whisper_language(Some("auto")), None);
        assert_eq!(whisper_language(Some("AUTO")), None);
        assert_eq!(whisper_language(Some("")), None);
        assert_eq!(whisper_language(Some("  ")), None);
        assert_eq!(whisper_language(Some("ja")), Some("ja".into()));
        assert_eq!(whisper_language(Some("en-US")), Some("en".into()));
        assert_eq!(whisper_language(Some("ZH-TW")), Some("zh".into()));
    }

    #[test]
    fn stamps_from_segments_maps_trims_and_skips_markers() {
        let segments = vec![
            (" Hello".to_string(), 0, 50),
            ("   ".to_string(), 50, 80),
            ("[BLANK_AUDIO]".to_string(), 80, 120),
            ("(music)".to_string(), 120, 150),
            (" world.".to_string(), 150, 140), // end < start clamps
        ];
        let stamps = stamps_from_segments(&segments);
        assert_eq!(stamps.len(), 2, "{stamps:?}");
        assert_eq!(stamps[0].word, "Hello");
        assert_eq!(stamps[0].start_seconds, 0.0);
        assert_eq!(stamps[0].end_seconds, 0.5);
        assert_eq!(stamps[1].word, "world.");
        assert_eq!(stamps[1].start_seconds, 1.5);
        assert_eq!(stamps[1].end_seconds, 1.5, "end clamps to start");
    }

    #[test]
    fn transcribe_without_model_path_reports_unavailable() {
        let prefs = temp_prefs("no-model.json", Some("{}"));
        let dir = std::env::temp_dir().join("fronda-transcribe-tests");
        let t = WhisperTranscriber::new(dir.clone(), prefs);
        let source = MediaSource::External {
            absolute_path: dir.join("a.wav").to_string_lossy().to_string(),
        };
        let err = t.transcribe(&source, None).unwrap_err();
        assert!(err.contains(WHISPER_MODEL_PATH_KEY), "{err}");
    }

    #[test]
    fn transcribe_with_bad_model_path_reports_load_failure() {
        let dir = std::env::temp_dir().join("fronda-transcribe-tests");
        let missing_model = dir.join("no-such-model.bin");
        let prefs = temp_prefs(
            "bad-model.json",
            Some(&format!(
                r#"{{"whisperModelPath": {}}}"#,
                serde_json::json!(missing_model.to_string_lossy())
            )),
        );
        let t = WhisperTranscriber::new(dir.clone(), prefs);
        let source = MediaSource::External {
            absolute_path: dir.join("a.wav").to_string_lossy().to_string(),
        };
        let err = t.transcribe(&source, None).unwrap_err();
        assert!(err.contains("model"), "{err}");
    }

    /// Real inference, gated on a user-supplied model: set FRONDA_WHISPER_MODEL
    /// to a ggml/gguf file to run. CI has no model — the test then skips.
    #[test]
    fn local_whisper_transcribes_silence_without_panicking() {
        let Some(model) = std::env::var_os("FRONDA_WHISPER_MODEL") else {
            eprintln!("skipping: FRONDA_WHISPER_MODEL not set");
            return;
        };
        let model = PathBuf::from(model);
        if !model.exists() {
            eprintln!("skipping: FRONDA_WHISPER_MODEL points at a missing file");
            return;
        }
        let dir = std::env::temp_dir().join("fronda-transcribe-tests");
        let _ = std::fs::create_dir_all(&dir);
        let prefs = temp_prefs(
            "real-model.json",
            Some(&format!(
                r#"{{"whisperModelPath": {}}}"#,
                serde_json::json!(model.to_string_lossy())
            )),
        );
        let wav = dir.join("silence.wav");
        let samples = vec![0.0f32; (WHISPER_SAMPLE_RATE / 2) as usize]; // 0.5s
        write_wav(&wav, &samples, WHISPER_SAMPLE_RATE, 1).unwrap();

        let t = WhisperTranscriber::new(dir.clone(), prefs);
        let source = MediaSource::External {
            absolute_path: wav.to_string_lossy().to_string(),
        };
        let stamps = t
            .transcribe(&source, Some("en"))
            .expect("silence transcribes without error");
        for s in &stamps {
            assert!(s.start_seconds >= 0.0 && s.end_seconds >= s.start_seconds, "{s:?}");
            assert!(!s.word.trim().is_empty(), "{s:?}");
        }
        let _ = std::fs::remove_file(&wav);
    }
}

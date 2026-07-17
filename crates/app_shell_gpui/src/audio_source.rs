//! Host `ClipAudioSource` for `remove_silence` (#174): decode a clip's source
//! audio to interleaved f32 PCM via ffmpeg (`audio_export::decode_audio_pcm`),
//! resolving `Project` sources against the open project root. Also reads the
//! recording capture date (#269 seeding) from ffmpeg format metadata.

use agent_contract::ClipAudioSource;
use core_model::MediaSource;
use ffmpeg_the_third as ffmpeg;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

fn init_ffmpeg() {
    static INIT: OnceLock<()> = OnceLock::new();
    INIT.get_or_init(|| {
        let _ = ffmpeg::init();
    });
}

pub struct ProjectAudioSource {
    project_root: PathBuf,
}

impl ProjectAudioSource {
    pub fn new(project_root: PathBuf) -> Self {
        Self { project_root }
    }

    fn resolve_path(&self, source: &MediaSource) -> PathBuf {
        match source {
            MediaSource::External { absolute_path } => PathBuf::from(absolute_path),
            MediaSource::Project { relative_path } => self.project_root.join(relative_path),
        }
    }
}

impl ClipAudioSource for ProjectAudioSource {
    fn decode_source_pcm(
        &self,
        source: &MediaSource,
        sample_rate: u32,
        channels: usize,
    ) -> Option<Vec<f32>> {
        crate::audio_export::decode_audio_pcm(
            &self.resolve_path(source),
            sample_rate,
            channels as u16,
        )
    }

    fn capture_date_seconds(&self, source: &MediaSource) -> Option<f64> {
        read_capture_date(&self.resolve_path(source))
    }

    fn timecode_frame_duration(&self, source: &MediaSource) -> Option<(i64, i64)> {
        read_timecode_frame_duration(&self.resolve_path(source))
    }
}

/// Format-level capture date: QuickTime `com.apple.quicktime.creationdate`,
/// falling back to the container `creation_time`. `None` on any failure.
fn read_capture_date(path: &Path) -> Option<f64> {
    init_ffmpeg();
    let ictx = ffmpeg::format::input(path).ok()?;
    let meta = ictx.metadata();
    meta.get("com.apple.quicktime.creationdate")
        .or_else(|| meta.get("creation_time"))
        .and_then(parse_capture_date)
}

/// Per-TC-frame duration from the container's timecode stream — NTSC 29.97
/// reads (1001, 30000). MOV/MP4 tmcd tracks demux as data streams tagged
/// 'tmcd' with no codec id, so the fourcc is the discriminator. The duration
/// is the inverted `avg_frame_rate` (the demuxer derives it from the tmcd
/// sample description); the stream time_base is only 1/timescale and would
/// mis-read NTSC as 1/30000. `None` when the file can't be opened, carries
/// no tmcd stream, or the rate is unset.
fn read_timecode_frame_duration(path: &Path) -> Option<(i64, i64)> {
    init_ffmpeg();
    const TMCD: u32 = u32::from_le_bytes(*b"tmcd");
    let ictx = ffmpeg::format::input(path).ok()?;
    for stream in ictx.streams() {
        let tag = unsafe { (*stream.parameters().as_ptr()).codec_tag };
        if tag != TMCD {
            continue;
        }
        // Reduce: mov timescales are muxer-chosen (25fps can demux 12800/512).
        let rate = stream.avg_frame_rate().reduce();
        if rate.numerator() > 0 && rate.denominator() > 0 {
            return Some((rate.denominator() as i64, rate.numerator() as i64));
        }
    }
    None
}

/// ISO8601 → epoch seconds. Accepts RFC3339 (`Z` / `+08:00`, fractional
/// seconds), QuickTime's colon-less offsets (`+0800`), ffmpeg's
/// space-separated form, and a bare naive datetime (read as UTC — sync
/// seeding only uses differences between sources).
pub fn parse_capture_date(raw: &str) -> Option<f64> {
    let s = raw.trim();
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
        return Some(epoch_seconds(dt.timestamp(), dt.timestamp_subsec_micros()));
    }
    for fmt in ["%Y-%m-%dT%H:%M:%S%.f%z", "%Y-%m-%d %H:%M:%S%.f%z"] {
        if let Ok(dt) = chrono::DateTime::parse_from_str(s, fmt) {
            return Some(epoch_seconds(dt.timestamp(), dt.timestamp_subsec_micros()));
        }
    }
    for fmt in ["%Y-%m-%dT%H:%M:%S%.f", "%Y-%m-%d %H:%M:%S%.f"] {
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(s, fmt) {
            let dt = dt.and_utc();
            return Some(epoch_seconds(dt.timestamp(), dt.timestamp_subsec_micros()));
        }
    }
    None
}

fn epoch_seconds(secs: i64, subsec_micros: u32) -> f64 {
    secs as f64 + subsec_micros as f64 / 1_000_000.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use audio_core::wav::write_wav;

    #[test]
    fn decodes_external_wav_via_seam() {
        let dir = std::env::temp_dir().join("fronda-audio-source-external");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("clip.wav");
        let samples: Vec<f32> = std::iter::repeat_n(0.5f32, 4800).collect(); // 0.1s @ 48k
        write_wav(&path, &samples, 48_000, 1).unwrap();

        let src = ProjectAudioSource::new(dir.clone());
        let source = MediaSource::External {
            absolute_path: path.to_string_lossy().to_string(),
        };
        let pcm = src
            .decode_source_pcm(&source, 48_000, 1)
            .expect("external wav decodes");
        assert!(
            pcm.iter().any(|s| s.abs() > 0.1),
            "decoded non-silent audio"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn resolves_project_source_against_root() {
        let dir = std::env::temp_dir().join("fronda-audio-source-project");
        let media = dir.join("media");
        let _ = std::fs::create_dir_all(&media);
        let path = media.join("a.wav");
        let samples: Vec<f32> = std::iter::repeat_n(0.3f32, 2400).collect();
        write_wav(&path, &samples, 48_000, 1).unwrap();

        let src = ProjectAudioSource::new(dir.clone());
        let source = MediaSource::Project {
            relative_path: "media/a.wav".to_string(),
        };
        let pcm = src
            .decode_source_pcm(&source, 48_000, 1)
            .expect("project source resolves against root and decodes");
        assert!(!pcm.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn parse_capture_date_rfc3339_forms() {
        assert_eq!(
            parse_capture_date("2021-01-01T00:00:00Z"),
            Some(1_609_459_200.0)
        );
        // Colon offset (same instant as above).
        assert_eq!(
            parse_capture_date("2021-01-01T08:00:00+08:00"),
            Some(1_609_459_200.0)
        );
        // Fractional seconds survive.
        assert_eq!(
            parse_capture_date("2021-01-01T00:00:00.250000Z"),
            Some(1_609_459_200.25)
        );
    }

    #[test]
    fn parse_capture_date_quicktime_colonless_offset() {
        // QuickTime `com.apple.quicktime.creationdate` writes `+0800`.
        assert_eq!(
            parse_capture_date("2021-01-01T08:00:00+0800"),
            Some(1_609_459_200.0)
        );
        assert_eq!(
            parse_capture_date("2021-01-01T08:00:00.5+0800"),
            Some(1_609_459_200.5)
        );
    }

    #[test]
    fn parse_capture_date_creation_time_forms() {
        // ffmpeg's normalized demux form.
        assert_eq!(
            parse_capture_date("2021-01-01T00:00:00.000000Z"),
            Some(1_609_459_200.0)
        );
        // Space-separated (matroska-style), no timezone → UTC.
        assert_eq!(
            parse_capture_date("2021-01-01 00:00:00"),
            Some(1_609_459_200.0)
        );
        assert_eq!(
            parse_capture_date("2021-01-01 00:00:00.500000"),
            Some(1_609_459_200.5)
        );
        // Bare naive with T separator.
        assert_eq!(
            parse_capture_date("2021-01-01T00:00:00"),
            Some(1_609_459_200.0)
        );
    }

    #[test]
    fn parse_capture_date_rejects_garbage() {
        assert_eq!(parse_capture_date(""), None);
        assert_eq!(parse_capture_date("not a date"), None);
        assert_eq!(parse_capture_date("2021-13-40T99:00:00Z"), None);
        assert_eq!(parse_capture_date("1609459200"), None);
    }

    #[test]
    fn capture_date_none_without_metadata_or_file() {
        let dir = std::env::temp_dir().join("fronda-audio-source-capture");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("plain.wav");
        let samples: Vec<f32> = std::iter::repeat_n(0.2f32, 480).collect();
        write_wav(&path, &samples, 48_000, 1).unwrap();

        let src = ProjectAudioSource::new(dir.clone());
        let plain = MediaSource::External {
            absolute_path: path.to_string_lossy().to_string(),
        };
        assert_eq!(
            src.capture_date_seconds(&plain),
            None,
            "wav without creation metadata → None, no error"
        );
        let missing = MediaSource::External {
            absolute_path: dir.join("missing.mov").to_string_lossy().to_string(),
        };
        assert_eq!(src.capture_date_seconds(&missing), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn timecode_frame_duration_none_without_tmcd() {
        // No tmcd fixture can live in-repo (design D1) — this pins the
        // None path: a plain stream never reads as a timecode stream, and
        // an unopenable file degrades to None instead of erroring.
        let dir = std::env::temp_dir().join("fronda-audio-source-tmcd");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("plain.wav");
        let samples: Vec<f32> = std::iter::repeat_n(0.1f32, 480).collect();
        write_wav(&path, &samples, 48_000, 1).unwrap();

        let src = ProjectAudioSource::new(dir.clone());
        let plain = MediaSource::External {
            absolute_path: path.to_string_lossy().to_string(),
        };
        assert_eq!(src.timecode_frame_duration(&plain), None);
        let missing = MediaSource::External {
            absolute_path: dir.join("missing.mov").to_string_lossy().to_string(),
        };
        assert_eq!(src.timecode_frame_duration(&missing), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

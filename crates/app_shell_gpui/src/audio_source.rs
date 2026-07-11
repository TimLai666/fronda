//! Host `ClipAudioSource` for `remove_silence` (#174): decode a clip's source
//! audio to interleaved f32 PCM via ffmpeg (`audio_export::decode_audio_pcm`),
//! resolving `Project` sources against the open project root.

use agent_contract::ClipAudioSource;
use core_model::MediaSource;
use std::path::PathBuf;

pub struct ProjectAudioSource {
    project_root: PathBuf,
}

impl ProjectAudioSource {
    pub fn new(project_root: PathBuf) -> Self {
        Self { project_root }
    }
}

impl ClipAudioSource for ProjectAudioSource {
    fn decode_source_pcm(
        &self,
        source: &MediaSource,
        sample_rate: u32,
        channels: usize,
    ) -> Option<Vec<f32>> {
        let path = match source {
            MediaSource::External { absolute_path } => PathBuf::from(absolute_path),
            MediaSource::Project { relative_path } => self.project_root.join(relative_path),
        };
        crate::audio_export::decode_audio_pcm(&path, sample_rate, channels as u16)
    }
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
}

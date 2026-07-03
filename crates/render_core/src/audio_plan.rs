//! Timeline → audio placement planning, then mixing.
//!
//! The pure bridge between the timeline model and `audio_core::audio_mixer`:
//! turn each audio-bearing clip into an [`AudioPlacement`] (offset + fades in
//! samples, per-clip volume) and mix them. Decoding is a `fetch_pcm` closure
//! (platform adapter, like the compositor's `fetch_source`), so this is pure and
//! unit-tested with synthetic PCM.

use audio_core::audio_mixer::{mix, samples_per_frame, AudioPlacement};
use core_model::{Clip, Timeline};
use timeline_core::TimelineMathExt;

/// Placement geometry (in output samples) for one clip at `sample_rate`/`fps`.
/// Decoding is separate — the caller attaches the PCM.
pub fn clip_placement_geometry(
    clip: &Clip,
    sample_rate: u32,
    fps: i64,
) -> (usize, usize, usize, f32) {
    let spf = samples_per_frame(sample_rate, fps);
    let start = clip.start_frame.max(0) as usize * spf;
    let fade_in = clip.fade_in_frames.max(0) as usize * spf;
    let fade_out = clip.fade_out_frames.max(0) as usize * spf;
    (start, fade_in, fade_out, clip.volume as f32)
}

/// Mix all audio-bearing clips of `timeline` into one interleaved buffer at
/// `sample_rate`/`channels`. `fetch_pcm(clip)` returns the clip's decoded
/// interleaved PCM at the mix rate/channels, or `None` when the clip has no
/// audio (the closure decides). Muted tracks are skipped. The buffer runs the
/// full timeline length so trailing silence is preserved.
pub fn mix_timeline_audio(
    timeline: &Timeline,
    sample_rate: u32,
    channels: usize,
    mut fetch_pcm: impl FnMut(&Clip) -> Option<Vec<f32>>,
) -> Vec<f32> {
    let fps = timeline.fps;
    let mut placements: Vec<AudioPlacement> = Vec::new();
    for track in &timeline.tracks {
        if track.muted {
            continue;
        }
        for clip in &track.clips {
            let Some(samples) = fetch_pcm(clip) else {
                continue;
            };
            let (start, fade_in, fade_out, volume) =
                clip_placement_geometry(clip, sample_rate, fps);
            placements.push(AudioPlacement {
                start_sample: start,
                samples,
                volume,
                fade_in_samples: fade_in,
                fade_out_samples: fade_out,
            });
        }
    }
    let min_frames = timeline.total_frames().max(0) as usize * samples_per_frame(sample_rate, fps);
    mix(&placements, channels, min_frames)
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_model::{ClipType, Track};

    fn audio_clip(id: &str, start: i64, dur: i64, volume: f64) -> Clip {
        core_model::Clip {
            id: id.into(),
            media_ref: format!("{id}-media"),
            media_type: ClipType::Audio,
            source_clip_type: ClipType::Audio,
            start_frame: start,
            duration_frames: dur,
            trim_start_frame: 0,
            trim_end_frame: 0,
            speed: 1.0,
            volume,
            opacity: 1.0,
            fade_in_frames: 0,
            fade_out_frames: 0,
            fade_in_interpolation: core_model::Interpolation::Linear,
            fade_out_interpolation: core_model::Interpolation::Linear,
            transform: Default::default(),
            crop: Default::default(),
            link_group_id: None,
            caption_group_id: None,
            text_content: None,
            text_style: None,
            opacity_track: None,
            position_track: None,
            scale_track: None,
            rotation_track: None,
            crop_track: None,
            volume_track: None,
            effects: None,
            shape_style: None,
            stroke_progress_track: None,
            compound_timeline_id: None,
            blend_mode: Default::default(),
            chroma_key: None,
        }
    }

    fn timeline_with(tracks: Vec<Track>) -> Timeline {
        Timeline {
            fps: 30,
            width: 1920,
            height: 1080,
            tracks,
            settings_configured: true,
            selected_clip_ids: Default::default(),
            transcription_language: None,
            compound_timelines: Default::default(),
        }
    }

    fn track(id: &str, muted: bool, clips: Vec<Clip>) -> Track {
        Track {
            id: id.into(),
            r#type: ClipType::Audio,
            muted,
            hidden: false,
            sync_locked: false,
            clips,
        }
    }

    #[test]
    fn geometry_converts_frames_to_samples() {
        let mut c = audio_clip("c", 2, 10, 0.5);
        c.fade_in_frames = 1;
        c.fade_out_frames = 2;
        // 48kHz / 30fps = 1600 samples/frame.
        let (start, fin, fout, vol) = clip_placement_geometry(&c, 48_000, 30);
        assert_eq!(start, 2 * 1600);
        assert_eq!(fin, 1600);
        assert_eq!(fout, 3200);
        assert_eq!(vol, 0.5);
    }

    #[test]
    fn mixes_placed_clip_at_sample_offset() {
        // One mono clip at frame 1. 30 Hz / 30 fps = 1 sample/frame, so the clip
        // begins at output sample 1.
        let tl = timeline_with(vec![track("a", false, vec![audio_clip("c", 1, 3, 1.0)])]);
        let out = mix_timeline_audio(&tl, 30, 1, |_| Some(vec![0.5, 0.5, 0.5]));
        assert_eq!(out[0], 0.0, "frame 0 silent");
        assert_eq!(out[1], 0.5, "clip starts at frame 1 → sample 1");
    }

    #[test]
    fn muted_track_is_skipped() {
        let tl = timeline_with(vec![track("a", true, vec![audio_clip("c", 0, 3, 1.0)])]);
        let out = mix_timeline_audio(&tl, 30, 1, |_| Some(vec![1.0, 1.0, 1.0]));
        assert!(out.iter().all(|&s| s == 0.0), "muted track contributes nothing");
    }

    #[test]
    fn clip_without_pcm_is_skipped() {
        let tl = timeline_with(vec![track("a", false, vec![audio_clip("c", 0, 3, 1.0)])]);
        let out = mix_timeline_audio(&tl, 30, 1, |_| None);
        assert!(out.iter().all(|&s| s == 0.0));
    }
}

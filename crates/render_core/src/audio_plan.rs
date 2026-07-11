//! Timeline → audio placement planning, then mixing.
//!
//! The pure bridge between the timeline model and `audio_core::audio_mixer`:
//! turn each audio-bearing clip into an [`AudioPlacement`] (offset + fades in
//! samples, per-clip volume) and mix them. Decoding is a `fetch_pcm` closure
//! (platform adapter, like the compositor's `fetch_source`), so this is pure and
//! unit-tested with synthetic PCM.

use audio_core::audio_mixer::{mix, resample_linear, samples_per_frame, AudioPlacement};
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

/// Take the clip's trimmed source range from the full decoded `source` PCM and
/// time-stretch it to the clip's timeline duration (so `speed != 1` changes both
/// duration and pitch, matching a classic speed change). `spf` is samples per
/// project frame. Returns silence-padded output of exactly `duration * spf`
/// per-channel frames.
fn extract_clip_audio(source: &[f32], channels: usize, clip: &Clip, spf: usize) -> Vec<f32> {
    let out_frames = (clip.duration_frames.max(0) as usize) * spf;
    if channels == 0 || out_frames == 0 {
        return Vec::new();
    }
    let total_frames = source.len() / channels;
    let mut out = vec![0.0f32; out_frames * channels];
    // Source sample-frames the clip consumes at its speed. Computed at SAMPLE
    // granularity (not rounded to whole project frames) so a slow-mo short clip
    // still consumes a non-zero slice instead of collapsing to silence.
    let consumed = (out_frames as f64 * clip.speed.max(0.0)).round() as usize;
    let src_start = (clip.trim_start_frame.max(0) as usize * spf).min(total_frames);
    if consumed > 0 {
        let src_end = (src_start + consumed).min(total_frames);
        let available = src_end.saturating_sub(src_start);
        if available > 0 {
            // Resample the AVAILABLE excerpt at the clip's intended speed ratio; when
            // the source is short this fills only the front and the tail stays silent
            // (rather than stretching the short excerpt across the whole duration).
            let resampled_len = if available >= consumed {
                out_frames
            } else {
                ((available as f64 / consumed as f64) * out_frames as f64).round() as usize
            }
            .min(out_frames);
            if resampled_len > 0 {
                let excerpt = &source[src_start * channels..src_end * channels];
                let resampled = resample_linear(excerpt, channels, resampled_len);
                let n = resampled.len().min(out.len());
                out[..n].copy_from_slice(&resampled[..n]);
            }
        }
    }

    // Bake the per-frame volume envelope (static volume or keyframed volume
    // track) so automation is honoured; the placement gain is then 1.0.
    if spf > 0 {
        for proj_frame in 0..clip.duration_frames.max(0) {
            let gain = timeline_core::resolved_volume_at(clip, proj_frame) as f32;
            if (gain - 1.0).abs() < f32::EPSILON {
                continue;
            }
            let base = proj_frame as usize * spf * channels;
            let end = (base + spf * channels).min(out.len());
            for s in &mut out[base..end] {
                *s *= gain;
            }
        }
    }
    out
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
    fetch_pcm: impl FnMut(&Clip) -> Option<Vec<f32>>,
) -> Vec<f32> {
    mix_timeline_audio_with_timelines(
        timeline,
        &std::collections::HashMap::new(),
        sample_rate,
        channels,
        fetch_pcm,
    )
}

/// [`mix_timeline_audio`] with the project's sibling timelines so nested-timeline
/// carriers (upstream #255) mix their child audio in.
pub fn mix_timeline_audio_with_timelines(
    timeline: &Timeline,
    timelines: &std::collections::HashMap<String, Timeline>,
    sample_rate: u32,
    channels: usize,
    mut fetch_pcm: impl FnMut(&Clip) -> Option<Vec<f32>>,
) -> Vec<f32> {
    let has_nests = timeline
        .tracks
        .iter()
        .flat_map(|t| &t.clips)
        .any(|c| c.source_clip_type == core_model::ClipType::Sequence);
    let flattened;
    let timeline: &Timeline = if !has_nests {
        timeline
    } else {
        flattened = timeline_core::flatten_nests(timeline, &|id: &str| timelines.get(id).cloned());
        &flattened
    };

    let fps = timeline.fps;
    let spf = samples_per_frame(sample_rate, fps);
    let mut placements: Vec<AudioPlacement> = Vec::new();
    for track in &timeline.tracks {
        if track.muted {
            continue;
        }
        for clip in &track.clips {
            let Some(source) = fetch_pcm(clip) else {
                continue;
            };
            // `fetch_pcm` returns the whole source; take only the clip's
            // trimmed range and time-stretch it (speed) to its timeline length.
            let samples = extract_clip_audio(&source, channels, clip, spf);
            // Volume (static + keyframed) is baked into `samples`, so the
            // placement gain is unity; geometry still supplies fades.
            let (start, fade_in, fade_out, _volume) =
                clip_placement_geometry(clip, sample_rate, fps);
            placements.push(AudioPlacement {
                start_sample: start,
                samples,
                volume: 1.0,
                fade_in_samples: fade_in,
                fade_out_samples: fade_out,
                fade_in_smooth: clip.fade_in_interpolation == core_model::Interpolation::Smooth,
                fade_out_smooth: clip.fade_out_interpolation == core_model::Interpolation::Smooth,
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
            multicam_group_id: None,
            text_animation: None,
            word_timings: None,
        }
    }

    fn timeline_with(tracks: Vec<Track>) -> Timeline {
        Timeline {
            id: String::new(),
            name: String::new(),
            fps: 30,
            width: 1920,
            height: 1080,
            tracks,
            settings_configured: true,
            selected_clip_ids: Default::default(),
            transcription_language: None,
            folder_id: None,
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
            display_height: 50.0,
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
    fn trim_start_offsets_into_the_source() {
        // 30 Hz / 30 fps → 1 sample/frame. Clip trims 2 frames in, 4 frames long.
        let mut clip = audio_clip("c", 0, 4, 1.0);
        clip.trim_start_frame = 2;
        let tl = timeline_with(vec![track("a", false, vec![clip])]);
        // Source of 10 mono frames 0.0, 0.1, ... 0.9.
        let source: Vec<f32> = (0..10).map(|i| i as f32 / 10.0).collect();
        let out = mix_timeline_audio(&tl, 30, 1, |_| Some(source.clone()));
        // Placed from frame 0: the trimmed excerpt starts at source frame 2.
        assert!(
            (out[0] - 0.2).abs() < 1e-6,
            "trim skips to 0.2, got {}",
            out[0]
        );
        assert!((out[3] - 0.5).abs() < 1e-6, "got {}", out[3]);
    }

    #[test]
    fn speed_time_stretches_to_timeline_length() {
        // 2x speed, 2-frame clip consumes 4 source frames, output stays 2 frames.
        let mut clip = audio_clip("c", 0, 2, 2.0);
        clip.trim_start_frame = 0;
        let tl = timeline_with(vec![track("a", false, vec![clip])]);
        let source: Vec<f32> = (0..8).map(|i| i as f32 / 10.0).collect();
        let out = mix_timeline_audio(&tl, 30, 1, |_| Some(source.clone()));
        // Timeline length is 2 frames, so exactly 2 output samples for the clip.
        assert_eq!(
            out.len(),
            2,
            "output matches the timeline duration, not the source"
        );
    }

    #[test]
    fn static_volume_scales_the_placed_audio() {
        let clip = audio_clip("c", 0, 3, 0.5);
        let tl = timeline_with(vec![track("a", false, vec![clip])]);
        let out = mix_timeline_audio(&tl, 30, 1, |_| Some(vec![1.0, 1.0, 1.0]));
        for s in &out {
            assert!((s - 0.5).abs() < 1e-6, "0.5 gain baked in, got {s}");
        }
    }

    #[test]
    fn keyframed_volume_is_baked_per_frame() {
        // Volume keyframes are dB: -40 dB (≈0.01 linear) ramping to 0 dB (unity).
        let mut clip = audio_clip("c", 0, 3, 1.0);
        clip.volume_track = Some(core_model::KeyframeTrack {
            keyframes: vec![
                core_model::Keyframe {
                    frame: 0,
                    value: -40.0,
                    interpolation_out: core_model::Interpolation::Linear,
                },
                core_model::Keyframe {
                    frame: 2,
                    value: 0.0,
                    interpolation_out: core_model::Interpolation::Linear,
                },
            ],
        });
        let tl = timeline_with(vec![track("a", false, vec![clip])]);
        let out = mix_timeline_audio(&tl, 30, 1, |_| Some(vec![1.0, 1.0, 1.0]));
        // dB ramps up over the clip, so the linear samples ramp up.
        assert!(
            out[0] < out[1] && out[1] < out[2],
            "volume automation ramps: {out:?}"
        );
        assert!(out[0] < 0.1, "starts near silent, got {}", out[0]);
    }

    #[test]
    fn short_source_pads_with_silence_not_stretch() {
        // 30 Hz / 30 fps → 1 sample/frame. A 10-frame clip at 1× wants 10 source
        // frames but the source only has 4 → the first 4 play, the tail is silence
        // (NOT the 4 frames stretched across all 10).
        let clip = audio_clip("c", 0, 10, 1.0);
        let tl = timeline_with(vec![track("a", false, vec![clip])]);
        let source: Vec<f32> = vec![1.0, 1.0, 1.0, 1.0];
        let out = mix_timeline_audio(&tl, 30, 1, |_| Some(source.clone()));
        assert_eq!(out.len(), 10);
        assert!(
            (out[0] - 1.0).abs() < 1e-6,
            "front is real audio, got {}",
            out[0]
        );
        assert!(
            (out[3] - 1.0).abs() < 1e-6,
            "front is real audio, got {}",
            out[3]
        );
        assert!(out[9].abs() < 1e-6, "tail must be silent, got {}", out[9]);
    }

    #[test]
    fn slow_motion_short_clip_is_not_silent() {
        // 300 Hz / 30 fps → 10 samples/frame. A 1-frame clip at 0.3× consumes
        // round(1×0.3)=0 whole PROJECT frames under naive rounding → whole-clip
        // silence. Sample-granular consumption (round(10×0.3)=3) keeps it audible.
        let mut clip = audio_clip("c", 0, 1, 1.0);
        clip.speed = 0.3;
        let tl = timeline_with(vec![track("a", false, vec![clip])]);
        let out = mix_timeline_audio(&tl, 300, 1, |_| Some(vec![1.0; 20]));
        assert!(
            out.iter().any(|&s| s.abs() > 0.0),
            "slow-mo short clip must not be silent: {out:?}"
        );
    }

    #[test]
    fn muted_track_is_skipped() {
        let tl = timeline_with(vec![track("a", true, vec![audio_clip("c", 0, 3, 1.0)])]);
        let out = mix_timeline_audio(&tl, 30, 1, |_| Some(vec![1.0, 1.0, 1.0]));
        assert!(
            out.iter().all(|&s| s == 0.0),
            "muted track contributes nothing"
        );
    }

    #[test]
    fn clip_without_pcm_is_skipped() {
        let tl = timeline_with(vec![track("a", false, vec![audio_clip("c", 0, 3, 1.0)])]);
        let out = mix_timeline_audio(&tl, 30, 1, |_| None);
        assert!(out.iter().all(|&s| s == 0.0));
    }
}

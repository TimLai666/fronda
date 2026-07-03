//! Timeline audio export: decode each clip's audio, mix, and write a WAV stem.
//!
//! Decoding uses statically-linked ffmpeg (resampled to f32 interleaved at the
//! mix rate/channels); mixing and WAV encoding are the pure `audio_core` cores.
//! WAV is chosen over an mp4 audio mux so the whole path is verifiable end to
//! end against a self-generated PCM WAV fixture (ffmpeg always decodes PCM).

use crate::video_export::source_path;
use audio_core::wav::write_wav;
use core_model::{Clip, MediaManifest, Timeline};
use ffmpeg::format::Sample;
use ffmpeg::util::channel_layout::ChannelLayout;
use ffmpeg_the_third as ffmpeg;
use render_core::audio_plan::mix_timeline_audio;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

fn init_ffmpeg() {
    static INIT: OnceLock<()> = OnceLock::new();
    INIT.get_or_init(|| {
        let _ = ffmpeg::init();
    });
}

/// Decode `source`'s audio as interleaved f32 PCM resampled to
/// `target_rate`/`target_channels`. `None` when there is no audio stream or on
/// any decode failure.
pub fn decode_audio_pcm(
    source: &Path,
    target_rate: u32,
    target_channels: u16,
) -> Option<Vec<f32>> {
    init_ffmpeg();
    let mut ictx = ffmpeg::format::input(source).ok()?;
    let stream = ictx.streams().best(ffmpeg::media::Type::Audio)?;
    let stream_index = stream.index();
    let ctx = ffmpeg::codec::context::Context::from_parameters(stream.parameters()).ok()?;
    let mut decoder = ctx.decoder().audio().ok()?;

    let dst_format = Sample::F32(ffmpeg::format::sample::Type::Packed);
    let channels = target_channels as usize;
    let mut out: Vec<f32> = Vec::new();
    let mut frame = ffmpeg::frame::Audio::empty();
    // Built lazily from the first decoded frame — the decoder's pre-decode
    // format/rate can be unset, which makes swr fail.
    let mut resampler: Option<ffmpeg::software::resampling::Context> = None;

    let drain = |resampler: &mut ffmpeg::software::resampling::Context,
                 out: &mut Vec<f32>,
                 frame: &ffmpeg::frame::Audio| {
        let mut resampled = ffmpeg::frame::Audio::empty();
        if resampler.run(frame, &mut resampled).is_ok() {
            let n = resampled.samples() * channels;
            let plane = resampled.plane::<f32>(0);
            out.extend_from_slice(&plane[..n.min(plane.len())]);
        }
    };

    // Normalize a decoded frame's channel layout to a mask-backed canonical
    // layout. The ffmpeg-the-third resampler needs mask-backed layouts (it
    // unwraps `.mask()`), and swr rejects a run whose frame layout differs from
    // the configured one — so we pin both to `default_for_channels`.
    let normalize = |frame: &mut ffmpeg::frame::Audio| {
        let n = frame.ch_layout().channels().max(1);
        frame.set_ch_layout(ChannelLayout::default_for_channels(n));
    };
    let ensure_resampler =
        |frame: &ffmpeg::frame::Audio| -> Option<ffmpeg::software::resampling::Context> {
            let n = frame.ch_layout().channels().max(1);
            ffmpeg::software::resampling::Context::get2(
                frame.format(),
                ChannelLayout::default_for_channels(n),
                frame.rate(),
                dst_format,
                ChannelLayout::default_for_channels(target_channels as u32),
                target_rate,
            )
            .ok()
        };

    for (s, packet) in ictx.packets().filter_map(Result::ok) {
        if s.index() != stream_index {
            continue;
        }
        if decoder.send_packet(&packet).is_err() {
            continue;
        }
        while decoder.receive_frame(&mut frame).is_ok() {
            normalize(&mut frame);
            if resampler.is_none() {
                resampler = ensure_resampler(&frame);
            }
            if let Some(r) = resampler.as_mut() {
                drain(r, &mut out, &frame);
            }
        }
    }
    let _ = decoder.send_eof();
    while decoder.receive_frame(&mut frame).is_ok() {
        normalize(&mut frame);
        if resampler.is_none() {
            resampler = ensure_resampler(&frame);
        }
        if let Some(r) = resampler.as_mut() {
            drain(r, &mut out, &frame);
        }
    }
    // Flush any samples the resampler is holding.
    if let Some(r) = resampler.as_mut() {
        loop {
            let mut resampled = ffmpeg::frame::Audio::empty();
            match r.flush(&mut resampled) {
                Ok(_) => {
                    let n = resampled.samples() * channels;
                    if n == 0 {
                        break;
                    }
                    let plane = resampled.plane::<f32>(0);
                    out.extend_from_slice(&plane[..n.min(plane.len())]);
                }
                Err(_) => break,
            }
        }
    }
    Some(out)
}

/// Decode `source`'s audio and downsample it to `buckets` waveform peaks for
/// timeline display. `None` when the source has no decodable audio.
pub fn clip_waveform_peaks(source: &Path, buckets: usize) -> Option<Vec<f32>> {
    // Mono at a modest rate is plenty for a display envelope.
    let pcm = decode_audio_pcm(source, 8_000, 1)?;
    if pcm.is_empty() {
        return None;
    }
    Some(audio_core::audio_mixer::compute_peaks(&pcm, 1, buckets))
}

/// Mix every audio-bearing clip of `timeline` and write a WAV stem to `out`.
pub fn export_audio_wav(
    timeline: &Timeline,
    manifest: &MediaManifest,
    project_root: &Path,
    out: &Path,
    sample_rate: u32,
    channels: u16,
) -> Result<(), String> {
    let paths: HashMap<&str, PathBuf> = manifest
        .entries
        .iter()
        .filter_map(|e| source_path(e, project_root).map(|p| (e.id.as_str(), p)))
        .collect();

    let mixed = mix_timeline_audio(timeline, sample_rate, channels as usize, |clip: &Clip| {
        let path = paths.get(clip.media_ref.as_str())?;
        decode_audio_pcm(path, sample_rate, channels)
    });

    write_wav(out, &mixed, sample_rate, channels)
}

#[cfg(test)]
mod tests {
    use super::*;
    use audio_core::wav::write_wav;
    use core_model::{
        ClipType, Crop, Interpolation, MediaManifestEntry, MediaSource, Track, Transform,
    };

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join("fronda-audio-export-tests").join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A 0.1s mono 440-ish ramp WAV at 48kHz, written with our own encoder.
    fn make_wav(path: &Path, rate: u32, channels: u16, frames: usize) {
        let mut samples = Vec::with_capacity(frames * channels as usize);
        for i in 0..frames {
            let v = ((i % 100) as f32 / 100.0) * 0.5;
            for _ in 0..channels {
                samples.push(v);
            }
        }
        write_wav(path, &samples, rate, channels).unwrap();
    }

    #[test]
    fn decodes_generated_wav_back_to_pcm() {
        let dir = temp_dir("decode");
        let src = dir.join("tone.wav");
        make_wav(&src, 48_000, 1, 4800); // 0.1s mono

        let pcm = decode_audio_pcm(&src, 48_000, 1).expect("wav should decode");
        // Same rate/channels → roughly the same sample count (± resampler priming).
        assert!(
            (pcm.len() as i64 - 4800).abs() < 200,
            "got {} samples",
            pcm.len()
        );
        // Values stay in range and are non-trivial (not all silence).
        assert!(pcm.iter().all(|s| s.abs() <= 1.0));
        assert!(pcm.iter().any(|s| s.abs() > 0.1));
    }

    #[test]
    fn waveform_peaks_from_generated_wav() {
        let dir = temp_dir("peaks");
        let src = dir.join("tone.wav");
        make_wav(&src, 48_000, 1, 4800); // ramps 0..0.5 repeatedly
        let peaks = clip_waveform_peaks(&src, 16).expect("wav has audio");
        assert_eq!(peaks.len(), 16);
        assert!(peaks.iter().all(|&p| (0.0..=1.0).contains(&p)));
        assert!(peaks.iter().any(|&p| p > 0.1), "envelope is non-trivial");
    }

    #[test]
    fn resamples_to_target_rate() {
        let dir = temp_dir("resample");
        let src = dir.join("tone.wav");
        make_wav(&src, 48_000, 1, 4800); // 0.1s at 48k
        let pcm = decode_audio_pcm(&src, 24_000, 1).expect("decode");
        // Half the rate → ~half the samples for the same duration.
        assert!(
            (pcm.len() as i64 - 2400).abs() < 200,
            "got {} samples",
            pcm.len()
        );
    }

    fn audio_clip(media_ref: &str, start: i64, dur: i64) -> Clip {
        Clip {
            id: format!("{media_ref}-clip"),
            media_ref: media_ref.into(),
            media_type: ClipType::Audio,
            source_clip_type: ClipType::Audio,
            start_frame: start,
            duration_frames: dur,
            trim_start_frame: 0,
            trim_end_frame: 0,
            speed: 1.0,
            volume: 1.0,
            opacity: 1.0,
            fade_in_frames: 0,
            fade_out_frames: 0,
            fade_in_interpolation: Interpolation::Linear,
            fade_out_interpolation: Interpolation::Linear,
            transform: Transform::default(),
            crop: Crop::default(),
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

    #[test]
    fn exports_timeline_audio_to_wav() {
        let dir = temp_dir("export");
        let src = dir.join("clipA.wav");
        make_wav(&src, 48_000, 2, 9600); // 0.2s stereo

        let timeline = Timeline {
            fps: 30,
            width: 1920,
            height: 1080,
            tracks: vec![Track {
                id: "a1".into(),
                r#type: ClipType::Audio,
                muted: false,
                hidden: false,
                sync_locked: false,
                clips: vec![audio_clip("m1", 0, 6)],
            }],
            settings_configured: true,
            selected_clip_ids: Default::default(),
            transcription_language: None,
            compound_timelines: Default::default(),
        };
        let mut manifest = MediaManifest::default();
        manifest.entries.push(MediaManifestEntry {
            id: "m1".into(),
            name: "m1".into(),
            r#type: ClipType::Audio,
            source: MediaSource::External {
                absolute_path: src.to_string_lossy().into_owned(),
            },
            duration: 0.2,
            generation_input: None,
            source_width: None,
            source_height: None,
            source_fps: None,
            has_audio: Some(true),
            folder_id: None,
            cached_remote_url: None,
            cached_remote_url_expires_at: None,
            source_timecode_frame: None,
            source_timecode_quanta: None,
            source_timecode_drop_frame: None,
            ai_tags: None,
            ai_description: None,
            ai_label_status: None,
        });

        let out = dir.join("stem.wav");
        export_audio_wav(&timeline, &manifest, &dir, &out, 48_000, 2).expect("export");
        let bytes = std::fs::read(&out).unwrap();
        assert!(bytes.len() > 44, "wav has data beyond the header");
        assert_eq!(&bytes[0..4], b"RIFF");
    }
}

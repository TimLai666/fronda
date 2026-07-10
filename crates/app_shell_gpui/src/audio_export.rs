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
pub fn decode_audio_pcm(source: &Path, target_rate: u32, target_channels: u16) -> Option<Vec<f32>> {
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

/// Export a project timeline to an mp4 with both video and audio. Composites
/// each frame (reusing one decoder per source), mixes the timeline audio, and
/// muxes an AAC stream when there is any non-silent audio (otherwise video-only).
/// Both streams start at PTS 0, so they stay in sync.
#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_arguments)]
pub fn export_project_with_audio(
    timeline: &Timeline,
    manifest: &MediaManifest,
    timelines: &HashMap<String, Timeline>,
    project_root: &Path,
    output: &Path,
    width: u32,
    height: u32,
    codec: crate::video_export::VideoCodec,
    output_fps: i64,
    progress: &std::sync::atomic::AtomicU64,
) -> Result<(), String> {
    use crate::video_export::{source_time_seconds, Mp4Encoder, SourceDecoder};
    use render_core::compositor::compose_frame_with_timelines;
    use std::sync::atomic::Ordering;
    use timeline_core::TimelineMathExt;

    let total_timeline = timeline.total_frames();
    if total_timeline <= 0 {
        return Err("timeline has no frames to export".into());
    }
    let fps = timeline.fps.max(1);
    // Output fps drives the encoder + how many frames we emit; each output frame
    // samples the timeline at out_frame * timeline_fps / out_fps (frame-rate
    // conversion). Audio is time-based, so its duration matches regardless.
    let out_fps = if output_fps > 0 { output_fps } else { fps };
    let total = (total_timeline * out_fps / fps).max(1);
    let (arate, ach) = (48_000u32, 2u16);

    let paths: HashMap<String, PathBuf> = manifest
        .entries
        .iter()
        .filter_map(|e| source_path(e, project_root).map(|p| (e.id.clone(), p)))
        .collect();

    let mixed = render_core::audio_plan::mix_timeline_audio_with_timelines(
        timeline,
        timelines,
        arate,
        ach as usize,
        |clip: &Clip| {
            let path = paths.get(clip.media_ref.as_str())?;
            decode_audio_pcm(path, arate, ach)
        },
    );
    let has_audio = mixed.iter().any(|&s| s != 0.0);

    let audio_params = has_audio.then_some((arate, ach));
    let mut enc =
        Mp4Encoder::new_av_codec(output, width, height, out_fps as i32, audio_params, codec)?;

    let ew = (width & !1).max(2) as usize;
    let eh = (height & !1).max(2) as usize;
    let mut decoders: HashMap<String, Option<SourceDecoder>> = HashMap::new();
    for out_frame in 0..total {
        // Map this output frame back to a timeline frame (frame-rate conversion).
        let tframe = out_frame * fps / out_fps;
        let mut fetch = |clip: &Clip, local_frame: i64| {
            let path = paths.get(clip.media_ref.as_str())?;
            decoders
                .entry(clip.media_ref.clone())
                .or_insert_with(|| SourceDecoder::open(path))
                .as_mut()?
                .frame_at_seconds(source_time_seconds(clip, local_frame, fps))
        };
        let img =
            compose_frame_with_timelines(timeline, manifest, timelines, tframe, ew, eh, &mut fetch);
        enc.write_frame(&img)?;
        // Report video progress as 0..=95%; the trailing 5% covers audio + mux.
        progress.store(
            ((out_frame + 1) as u64 * 95 / total as u64).min(95),
            Ordering::Relaxed,
        );
    }
    if has_audio {
        enc.write_audio(&mixed)?;
    }
    let result = enc.finish();
    progress.store(100, Ordering::Relaxed);
    result
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
        let dir = std::env::temp_dir()
            .join("fronda-audio-export-tests")
            .join(name);
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
            multicam_group_id: None,
            text_animation: None,
            word_timings: None,
        }
    }

    fn video_clip(media_ref: &str, start: i64, dur: i64) -> Clip {
        let mut c = audio_clip(media_ref, start, dur);
        c.media_type = ClipType::Video;
        c.source_clip_type = ClipType::Video;
        c.transform = Transform::from_top_left(0.0, 0.0, 1.0, 1.0);
        c
    }

    fn external_entry(
        id: &str,
        path: &Path,
        kind: ClipType,
        has_audio: bool,
    ) -> MediaManifestEntry {
        MediaManifestEntry {
            id: id.into(),
            name: id.into(),
            r#type: kind,
            source: MediaSource::External {
                absolute_path: path.to_string_lossy().into_owned(),
            },
            duration: 1.0,
            generation_input: None,
            source_width: None,
            source_height: None,
            source_fps: None,
            has_audio: Some(has_audio),
            folder_id: None,
            cached_remote_url: None,
            cached_remote_url_expires_at: None,
            source_timecode_frame: None,
            source_timecode_quanta: None,
            source_timecode_drop_frame: None,
            ai_tags: None,
            ai_description: None,
            ai_label_status: None,
            generation_status: None,
        }
    }

    fn external_entry_video(id: &str, path: &Path) -> MediaManifestEntry {
        external_entry(id, path, ClipType::Video, false)
    }

    fn external_entry_audio(id: &str, path: &Path) -> MediaManifestEntry {
        external_entry(id, path, ClipType::Audio, true)
    }

    #[test]
    fn export_project_with_audio_muxes_video_and_audio() {
        use crate::video_export::encoder_available;
        if !encoder_available() || ffmpeg::encoder::find(ffmpeg::codec::Id::AAC).is_none() {
            eprintln!("skipping: no video/AAC encoder");
            return;
        }
        let dir = temp_dir("project-av");
        // Audio source (WAV) + a video source (the committed fixture).
        let wav = dir.join("a.wav");
        make_wav(&wav, 48_000, 2, 9600); // 0.2s stereo
        let video_fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/testclip.mp4");

        let mut manifest = MediaManifest::default();
        manifest
            .entries
            .push(external_entry_video("v1", &video_fixture));
        manifest.entries.push(external_entry_audio("a1", &wav));

        let timeline = Timeline {
            id: String::new(),
            name: String::new(),
            fps: 15,
            width: 64,
            height: 48,
            tracks: vec![
                Track {
                    id: "vid".into(),
                    r#type: ClipType::Video,
                    muted: false,
                    hidden: false,
                    sync_locked: false,
                   display_height: 50.0,
                    clips: vec![video_clip("v1", 0, 3)],
                },
                Track {
                    id: "aud".into(),
                    r#type: ClipType::Audio,
                    muted: false,
                    hidden: false,
                    sync_locked: false,
                   display_height: 50.0,
                    clips: vec![audio_clip("a1", 0, 3)],
                },
            ],
            settings_configured: true,
            selected_clip_ids: Default::default(),
            transcription_language: None,
            folder_id: None,
            compound_timelines: Default::default(),
        };

        let out = dir.join("out.mp4");
        let progress = std::sync::atomic::AtomicU64::new(0);
        export_project_with_audio(
            &timeline,
            &manifest,
            &HashMap::new(),
            &dir,
            &out,
            64,
            48,
            crate::video_export::VideoCodec::H264,
            0, // 0 = use the timeline fps
            &progress,
        )
        .expect("av export");
        assert_eq!(
            progress.load(std::sync::atomic::Ordering::Relaxed),
            100,
            "progress completes"
        );
        assert!(std::fs::metadata(&out).unwrap().len() > 0);
        assert!(decode_audio_pcm(&out, 48_000, 2).is_some_and(|p| !p.is_empty()));
        assert!(crate::video_export::decode_frame_rgba(&out, 0.0).is_some());
    }

    #[test]
    fn exports_at_a_different_output_fps() {
        use crate::video_export::encoder_available;
        if !encoder_available() {
            return;
        }
        let video_fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/testclip.mp4");
        let dir = temp_dir("fps-conv");
        let mut manifest = MediaManifest::default();
        manifest.entries.push(external_entry_video("v1", &video_fixture));
        let timeline = Timeline {
            id: String::new(),
            name: String::new(),
            fps: 15,
            width: 64,
            height: 48,
            tracks: vec![Track {
                id: "vid".into(),
                r#type: ClipType::Video,
                muted: false,
                hidden: false,
                sync_locked: false,
               display_height: 50.0,
                clips: vec![video_clip("v1", 0, 4)],
            }],
            settings_configured: true,
            selected_clip_ids: Default::default(),
            transcription_language: None,
            folder_id: None,
            compound_timelines: Default::default(),
        };
        let out = dir.join("out30.mp4");
        let progress = std::sync::atomic::AtomicU64::new(0);
        // 15fps timeline → 30fps output (frame-rate conversion, ~2x the frames).
        export_project_with_audio(
            &timeline,
            &manifest,
            &HashMap::new(),
            &dir,
            &out,
            64,
            48,
            crate::video_export::VideoCodec::H264,
            30,
            &progress,
        )
        .expect("fps-converted export");
        assert!(std::fs::metadata(&out).unwrap().len() > 0);
        assert!(crate::video_export::decode_frame_rgba(&out, 0.0).is_some());
    }

    #[test]
    fn exports_timeline_audio_to_wav() {
        let dir = temp_dir("export");
        let src = dir.join("clipA.wav");
        make_wav(&src, 48_000, 2, 9600); // 0.2s stereo

        let timeline = Timeline {
            id: String::new(),
            name: String::new(),
            fps: 30,
            width: 1920,
            height: 1080,
            tracks: vec![Track {
                id: "a1".into(),
                r#type: ClipType::Audio,
                muted: false,
                hidden: false,
                sync_locked: false,
               display_height: 50.0,
                clips: vec![audio_clip("m1", 0, 6)],
            }],
            settings_configured: true,
            selected_clip_ids: Default::default(),
            transcription_language: None,
            folder_id: None,
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
            generation_status: None,
        });

        let out = dir.join("stem.wav");
        export_audio_wav(&timeline, &manifest, &dir, &out, 48_000, 2).expect("export");
        let bytes = std::fs::read(&out).unwrap();
        assert!(bytes.len() > 44, "wav has data beyond the header");
        assert_eq!(&bytes[0..4], b"RIFF");
    }
}

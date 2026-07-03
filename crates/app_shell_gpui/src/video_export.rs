//! Pixel-render video export via statically-linked ffmpeg.
//!
//! Composites each timeline frame (render_core) then encodes the sequence to
//! an mp4. Decode and encode are the only platform pieces; all frame math and
//! compositing live in the pure `render_core::compositor`. ffmpeg is compiled
//! into the binary, so no ffmpeg executable is needed at runtime.
//!
//! The encoder prefers H.264 and falls back to MPEG-4 Part 2, which is present
//! in stock ffmpeg builds even without the (GPL) libx264 encoder.

use core_model::{Clip, MediaManifest, MediaSource, Timeline};
use ffmpeg_the_third as ffmpeg;
use ffmpeg::format::Pixel;
use ffmpeg::Rational;
use render_core::compositor::{compose_frame, RgbaImage};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use timeline_core::TimelineMathExt;

fn init_ffmpeg() {
    static INIT: OnceLock<()> = OnceLock::new();
    INIT.get_or_init(|| {
        let _ = ffmpeg::init();
    });
}

/// First available mp4-compatible video encoder: H.264 if present, else the
/// always-available native MPEG-4 Part 2. `None` when neither is compiled in.
fn pick_encoder() -> Option<ffmpeg::codec::Codec> {
    [ffmpeg::codec::Id::H264, ffmpeg::codec::Id::MPEG4]
        .into_iter()
        .find_map(ffmpeg::encoder::find)
}

/// True when any usable video encoder is available in the linked ffmpeg.
pub fn encoder_available() -> bool {
    init_ffmpeg();
    pick_encoder().is_some()
}

fn err<T>(context: &str, e: ffmpeg::Error) -> Result<T, String> {
    Err(format!("{context}: {e}"))
}

/// Incremental RGBA → mp4 encoder. Feed frames with [`write_frame`], then
/// [`finish`] to flush and mux the trailer.
pub struct Mp4Encoder {
    octx: ffmpeg::format::context::Output,
    encoder: ffmpeg::codec::encoder::video::Encoder,
    scaler: ffmpeg::software::scaling::Context,
    width: u32,
    height: u32,
    stream_index: usize,
    enc_time_base: Rational,
    ost_time_base: Rational,
    next_pts: i64,
    finished: bool,
}

impl Mp4Encoder {
    /// Open `output` for writing at `width`x`height` (rounded down to even, as
    /// YUV420P requires) and `fps` frames per second.
    pub fn new(output: &Path, width: u32, height: u32, fps: i32) -> Result<Self, String> {
        init_ffmpeg();
        let width = width & !1;
        let height = height & !1;
        if width == 0 || height == 0 {
            return Err("output dimensions must be >= 2".into());
        }
        if fps <= 0 {
            return Err("fps must be positive".into());
        }
        let codec = pick_encoder().ok_or("no H.264/MPEG-4 encoder in linked ffmpeg")?;

        let mut octx = match ffmpeg::format::output(&output) {
            Ok(o) => o,
            Err(e) => return err("open output", e),
        };
        let global_header = octx
            .format()
            .flags()
            .contains(ffmpeg::format::flag::Flags::GLOBAL_HEADER);

        let enc_time_base = Rational(1, fps);
        let ctx = ffmpeg::codec::context::Context::new_with_codec(codec);
        let mut enc = match ctx.encoder().video() {
            Ok(e) => e,
            Err(e) => return err("create video encoder", e),
        };
        enc.set_width(width);
        enc.set_height(height);
        enc.set_format(Pixel::YUV420P);
        enc.set_time_base(enc_time_base);
        enc.set_frame_rate(Some(Rational(fps, 1)));
        enc.set_gop(12);
        enc.set_bit_rate(bit_rate_for(width, height, fps));
        if global_header {
            enc.set_flags(ffmpeg::codec::Flags::GLOBAL_HEADER);
        }
        let encoder = match enc.open_as(codec) {
            Ok(e) => e,
            Err(e) => return err("open encoder", e),
        };

        let stream_index;
        {
            let mut ost = match octx.add_stream(codec) {
                Ok(s) => s,
                Err(e) => return err("add stream", e),
            };
            ost.set_parameters(ffmpeg::codec::Parameters::from(&encoder));
            ost.set_time_base(enc_time_base);
            stream_index = ost.index();
        }

        if let Err(e) = octx.write_header() {
            return err("write header", e);
        }
        let ost_time_base = octx
            .stream(stream_index)
            .map(|s| s.time_base())
            .unwrap_or(enc_time_base);

        let scaler = match ffmpeg::software::scaling::Context::get(
            Pixel::RGBA,
            width,
            height,
            Pixel::YUV420P,
            width,
            height,
            ffmpeg::software::scaling::Flags::BILINEAR,
        ) {
            Ok(s) => s,
            Err(e) => return err("create scaler", e),
        };

        Ok(Self {
            octx,
            encoder,
            scaler,
            width,
            height,
            stream_index,
            enc_time_base,
            ost_time_base,
            next_pts: 0,
            finished: false,
        })
    }

    /// Encode one RGBA frame. Its dimensions must match the encoder's.
    pub fn write_frame(&mut self, image: &RgbaImage) -> Result<(), String> {
        if image.width as u32 != self.width || image.height as u32 != self.height {
            return Err(format!(
                "frame is {}x{}, encoder expects {}x{}",
                image.width, image.height, self.width, self.height
            ));
        }
        let mut rgba = ffmpeg::frame::Video::new(Pixel::RGBA, self.width, self.height);
        {
            let stride = rgba.stride(0);
            let row_bytes = self.width as usize * 4;
            let data = rgba.data_mut(0);
            for y in 0..self.height as usize {
                let dst = &mut data[y * stride..y * stride + row_bytes];
                let src = &image.pixels[y * row_bytes..y * row_bytes + row_bytes];
                dst.copy_from_slice(src);
            }
        }
        let mut yuv = ffmpeg::frame::Video::new(Pixel::YUV420P, self.width, self.height);
        if let Err(e) = self.scaler.run(&rgba, &mut yuv) {
            return err("scale frame", e);
        }
        yuv.set_pts(Some(self.next_pts));
        self.next_pts += 1;
        if let Err(e) = self.encoder.send_frame(&yuv) {
            return err("send frame", e);
        }
        self.drain()
    }

    fn drain(&mut self) -> Result<(), String> {
        let mut packet = ffmpeg::codec::packet::Packet::empty();
        while self.encoder.receive_packet(&mut packet).is_ok() {
            packet.set_stream(self.stream_index);
            packet.rescale_ts(self.enc_time_base, self.ost_time_base);
            if let Err(e) = packet.write_interleaved(&mut self.octx) {
                return err("write packet", e);
            }
        }
        Ok(())
    }

    /// Flush the encoder and write the container trailer.
    pub fn finish(mut self) -> Result<(), String> {
        self.finish_inner()
    }

    fn finish_inner(&mut self) -> Result<(), String> {
        if self.finished {
            return Ok(());
        }
        self.finished = true;
        if let Err(e) = self.encoder.send_eof() {
            return err("send eof", e);
        }
        self.drain()?;
        if let Err(e) = self.octx.write_trailer() {
            return err("write trailer", e);
        }
        Ok(())
    }
}

impl Drop for Mp4Encoder {
    fn drop(&mut self) {
        // Best-effort flush if the caller dropped without finish().
        let _ = self.finish_inner();
    }
}

/// Target bit rate heuristic: ~0.1 bits per pixel-second, floored so tiny test
/// clips still get a sane rate.
fn bit_rate_for(width: u32, height: u32, fps: i32) -> usize {
    let pixels = width as u64 * height as u64 * fps.max(1) as u64;
    ((pixels / 10).max(200_000)) as usize
}

/// Encode a finite stream of same-sized RGBA frames to `output`.
pub fn encode_rgba_frames(
    output: &Path,
    width: u32,
    height: u32,
    fps: i32,
    frames: impl IntoIterator<Item = RgbaImage>,
) -> Result<(), String> {
    let mut enc = Mp4Encoder::new(output, width, height, fps)?;
    let mut count = 0u64;
    for frame in frames {
        enc.write_frame(&frame)?;
        count += 1;
    }
    if count == 0 {
        return Err("no frames to encode".into());
    }
    enc.finish()
}

/// Composite every frame of `timeline` and encode to `output`.
///
/// `decode(clip, timeline_frame)` supplies each clip's decoded source frame as
/// RGBA at its native resolution; the compositor positions, crops, scales, and
/// blends it. Returning `None` leaves that clip absent for the frame.
pub fn export_timeline(
    timeline: &Timeline,
    manifest: &MediaManifest,
    output: &Path,
    width: u32,
    height: u32,
    fps: i32,
    total_frames: i64,
    mut decode: impl FnMut(&Clip, i64) -> Option<RgbaImage>,
) -> Result<(), String> {
    if total_frames <= 0 {
        return Err("timeline has no frames to export".into());
    }
    let mut enc = Mp4Encoder::new(output, width, height, fps)?;
    let w = enc.width as usize;
    let h = enc.height as usize;
    for frame in 0..total_frames {
        let img = compose_frame(timeline, manifest, frame, w, h, |clip| decode(clip, frame));
        enc.write_frame(&img)?;
    }
    enc.finish()
}

/// Real-time offset into a clip's source for a given absolute timeline frame.
///
/// Timeline math is in project frames, so the source offset is
/// `trim_start + rel * speed` project frames (matching `source_frames_consumed`)
/// converted to seconds by the project fps. `rel` is clamped to the clip range.
pub fn source_time_seconds(clip: &Clip, timeline_frame: i64, project_fps: i64) -> f64 {
    let rel = (timeline_frame - clip.start_frame).max(0) as f64;
    let src_frame = clip.trim_start_frame as f64 + rel * clip.speed.max(0.0);
    src_frame / project_fps.max(1) as f64
}

/// Absolute filesystem path for a manifest entry, given the project root that
/// project-relative sources are resolved against. `None` for sources without a
/// local file (e.g. remote-only entries).
pub fn source_path(entry: &core_model::MediaManifestEntry, project_root: &Path) -> Option<PathBuf> {
    match &entry.source {
        MediaSource::External { absolute_path } => Some(PathBuf::from(absolute_path)),
        MediaSource::Project { relative_path } => Some(project_root.join(relative_path)),
    }
}

/// Composite and encode an entire project timeline to `output`.
///
/// Resolves each clip's source file from `manifest` (relative sources against
/// `project_root`), decodes the source frame at the clip's mapped time, and
/// composites via render_core. Decoding seeks per frame, so this favours
/// correctness over speed; a sequential-decode fast path is a follow-up.
pub fn export_project(
    timeline: &Timeline,
    manifest: &MediaManifest,
    project_root: &Path,
    output: &Path,
    width: u32,
    height: u32,
) -> Result<(), String> {
    let total = timeline.total_frames();
    if total <= 0 {
        return Err("timeline has no frames to export".into());
    }
    let fps = timeline.fps;
    let paths: HashMap<&str, PathBuf> = manifest
        .entries
        .iter()
        .filter_map(|e| source_path(e, project_root).map(|p| (e.id.as_str(), p)))
        .collect();

    // One decoder per source, opened on first use and reused for every frame —
    // avoids reopening the file per frame.
    let mut decoders: HashMap<String, Option<SourceDecoder>> = HashMap::new();
    export_timeline(
        timeline,
        manifest,
        output,
        width,
        height,
        fps as i32,
        total,
        |clip, frame| {
            let path = paths.get(clip.media_ref.as_str())?;
            let decoder = decoders
                .entry(clip.media_ref.clone())
                .or_insert_with(|| SourceDecoder::open(path));
            decoder
                .as_mut()?
                .frame_at_seconds(source_time_seconds(clip, frame, fps))
        },
    )
}

/// A video source opened once and reused across many frame requests. Avoids the
/// per-frame file-open + stream-probe cost of [`decode_frame_rgba`], which makes
/// a full-timeline export dramatically faster.
pub struct SourceDecoder {
    ictx: ffmpeg::format::context::Input,
    decoder: ffmpeg::decoder::Video,
    scaler: ffmpeg::software::scaling::Context,
    video_index: usize,
    width: u32,
    height: u32,
}

impl SourceDecoder {
    /// Open `source`'s best video stream. `None` when it has no decodable video.
    pub fn open(source: &Path) -> Option<Self> {
        init_ffmpeg();
        let ictx = ffmpeg::format::input(source).ok()?;
        let stream = ictx.streams().best(ffmpeg::media::Type::Video)?;
        let video_index = stream.index();
        let decoder_ctx =
            ffmpeg::codec::context::Context::from_parameters(stream.parameters()).ok()?;
        let decoder = decoder_ctx.decoder().video().ok()?;
        let (width, height) = (decoder.width(), decoder.height());
        if width == 0 || height == 0 {
            return None;
        }
        let scaler = ffmpeg::software::scaling::Context::get(
            decoder.format(),
            width,
            height,
            Pixel::RGBA,
            width,
            height,
            ffmpeg::software::scaling::Flags::BILINEAR,
        )
        .ok()?;
        Some(Self {
            ictx,
            decoder,
            scaler,
            video_index,
            width,
            height,
        })
    }

    /// Decode the frame nearest `time_seconds` as native-resolution RGBA, reusing
    /// the already-open context. `None` on decode failure.
    pub fn frame_at_seconds(&mut self, time_seconds: f64) -> Option<RgbaImage> {
        if time_seconds >= 0.0 {
            let ts = (time_seconds * 1_000_000.0) as i64;
            let _ = self.ictx.seek(ts, ..=ts);
        }
        let mut frame = ffmpeg::frame::Video::empty();
        let mut rgba = ffmpeg::frame::Video::empty();
        let mut got = false;
        for res in self.ictx.packets() {
            let Ok((packet_stream, packet)) = res else {
                break;
            };
            if packet_stream.index() != self.video_index {
                continue;
            }
            if self.decoder.send_packet(&packet).is_err() {
                continue;
            }
            if self.decoder.receive_frame(&mut frame).is_ok()
                && self.scaler.run(&frame, &mut rgba).is_ok()
            {
                got = true;
                break;
            }
        }
        if !got {
            return None;
        }
        let (w, h) = (self.width as usize, self.height as usize);
        let stride = rgba.stride(0);
        let data = rgba.data(0);
        let row_bytes = w * 4;
        let mut pixels = Vec::with_capacity(w * h * 4);
        for y in 0..h {
            pixels.extend_from_slice(&data[y * stride..y * stride + row_bytes]);
        }
        Some(RgbaImage {
            width: w,
            height: h,
            pixels,
        })
    }
}

/// Decode the frame nearest `time_seconds` from `source` as RGBA at the
/// source's native resolution. `None` on any decode failure.
pub fn decode_frame_rgba(source: &Path, time_seconds: f64) -> Option<RgbaImage> {
    init_ffmpeg();
    let mut ictx = ffmpeg::format::input(source).ok()?;
    let stream = ictx.streams().best(ffmpeg::media::Type::Video)?;
    let video_index = stream.index();
    let decoder_ctx = ffmpeg::codec::context::Context::from_parameters(stream.parameters()).ok()?;
    let mut decoder = decoder_ctx.decoder().video().ok()?;

    let (w, h) = (decoder.width(), decoder.height());
    if w == 0 || h == 0 {
        return None;
    }
    let mut scaler = ffmpeg::software::scaling::Context::get(
        decoder.format(),
        w,
        h,
        Pixel::RGBA,
        w,
        h,
        ffmpeg::software::scaling::Flags::BILINEAR,
    )
    .ok()?;

    if time_seconds > 0.0 {
        let ts = (time_seconds * 1_000_000.0) as i64;
        let _ = ictx.seek(ts, ..ts);
    }

    let mut frame = ffmpeg::frame::Video::empty();
    let mut rgba = ffmpeg::frame::Video::empty();
    let mut got = false;
    'outer: for res in ictx.packets() {
        let Ok((packet_stream, packet)) = res else {
            break;
        };
        if packet_stream.index() != video_index {
            continue;
        }
        if decoder.send_packet(&packet).is_err() {
            continue;
        }
        while decoder.receive_frame(&mut frame).is_ok() {
            if scaler.run(&frame, &mut rgba).is_ok() {
                got = true;
                break 'outer;
            }
        }
    }
    if !got {
        let _ = decoder.send_eof();
        if decoder.receive_frame(&mut frame).is_ok() {
            got = scaler.run(&frame, &mut rgba).is_ok();
        }
    }
    if !got {
        return None;
    }

    let stride = rgba.stride(0);
    let data = rgba.data(0);
    let row_bytes = w as usize * 4;
    let mut pixels = Vec::with_capacity(w as usize * h as usize * 4);
    for y in 0..h as usize {
        pixels.extend_from_slice(&data[y * stride..y * stride + row_bytes]);
    }
    Some(RgbaImage {
        width: w as usize,
        height: h as usize,
        pixels,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_model::{
        ClipType, Crop, Interpolation, MediaManifestEntry, Track, Transform,
    };

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join("fronda-video-export-tests").join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn full_frame_clip(id: &str, media_ref: &str, start: i64, dur: i64) -> Clip {
        Clip {
            id: id.into(),
            media_ref: media_ref.into(),
            media_type: ClipType::Video,
            source_clip_type: ClipType::Video,
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
            transform: Transform::from_top_left(0.0, 0.0, 1.0, 1.0),
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

    fn single_video_timeline(clip: Clip) -> Timeline {
        Timeline {
            fps: 15,
            width: 64,
            height: 48,
            tracks: vec![Track {
                id: "v1".into(),
                r#type: ClipType::Video,
                muted: false,
                hidden: false,
                sync_locked: false,
                clips: vec![clip],
            }],
            settings_configured: true,
            selected_clip_ids: std::collections::HashSet::new(),
            transcription_language: None,
            compound_timelines: std::collections::HashMap::new(),
        }
    }

    fn external_entry(id: &str, absolute_path: &Path) -> MediaManifestEntry {
        MediaManifestEntry {
            id: id.into(),
            name: id.into(),
            r#type: ClipType::Video,
            source: MediaSource::External {
                absolute_path: absolute_path.to_string_lossy().into_owned(),
            },
            duration: 1.0,
            generation_input: None,
            source_width: None,
            source_height: None,
            source_fps: None,
            has_audio: None,
            folder_id: None,
            cached_remote_url: None,
            cached_remote_url_expires_at: None,
            source_timecode_frame: None,
            source_timecode_quanta: None,
            source_timecode_drop_frame: None,
            ai_tags: None,
            ai_description: None,
            ai_label_status: None,
        }
    }

    #[test]
    fn rejects_odd_or_zero_dimensions() {
        let dir = temp_dir("dims");
        assert!(Mp4Encoder::new(&dir.join("a.mp4"), 0, 48, 15).is_err());
        // 1x1 rounds down to 0x0 → rejected.
        assert!(Mp4Encoder::new(&dir.join("b.mp4"), 1, 1, 15).is_err());
        assert!(Mp4Encoder::new(&dir.join("c.mp4"), 64, 48, 0).is_err());
    }

    #[test]
    fn encode_then_decode_roundtrip() {
        if !encoder_available() {
            eprintln!("skipping: no video encoder in linked ffmpeg");
            return;
        }
        let dir = temp_dir("roundtrip");
        let out = dir.join("clip.mp4");
        let (w, h, fps) = (64u32, 48u32, 15i32);

        // 10 solid strong-red frames.
        let frames = (0..10).map(|_| RgbaImage::solid(w as usize, h as usize, [220, 30, 30, 255]));
        encode_rgba_frames(&out, w, h, fps, frames).expect("encode should succeed");

        let meta = std::fs::metadata(&out).expect("output file exists");
        assert!(meta.len() > 0, "output is non-empty");

        // Decode the first frame back — dimensions preserved, colour stays
        // clearly red-dominant despite lossy YUV420P encoding.
        let decoded = decode_frame_rgba(&out, 0.0).expect("output should decode");
        assert_eq!(decoded.width, w as usize);
        assert_eq!(decoded.height, h as usize);
        let (r, g, b) = (decoded.pixels[0], decoded.pixels[1], decoded.pixels[2]);
        assert!(
            r > 120 && r > g && r > b,
            "red-dominant, got rgb=({r},{g},{b})"
        );
    }

    #[test]
    fn encode_zero_frames_errors() {
        if !encoder_available() {
            return;
        }
        let dir = temp_dir("empty");
        let out = dir.join("clip.mp4");
        let frames: Vec<RgbaImage> = Vec::new();
        assert!(encode_rgba_frames(&out, 64, 48, 15, frames).is_err());
    }

    #[test]
    fn frame_size_mismatch_errors() {
        if !encoder_available() {
            return;
        }
        let dir = temp_dir("mismatch");
        let mut enc = Mp4Encoder::new(&dir.join("clip.mp4"), 64, 48, 15).unwrap();
        let wrong = RgbaImage::solid(32, 24, [0, 0, 0, 255]);
        assert!(enc.write_frame(&wrong).is_err());
    }

    #[test]
    fn source_time_seconds_maps_project_frames() {
        let mut clip = full_frame_clip("c", "m", 10, 30);
        clip.trim_start_frame = 6; // 6 project frames into the source
        clip.speed = 2.0;
        // At the clip's first timeline frame: only the trim offset, at 15 fps.
        assert!((source_time_seconds(&clip, 10, 15) - 6.0 / 15.0).abs() < 1e-9);
        // 5 frames in at 2x speed → source frame 6 + 10 = 16 → 16/15 s.
        assert!((source_time_seconds(&clip, 15, 15) - 16.0 / 15.0).abs() < 1e-9);
        // Frames before the clip clamp to the start (rel = 0).
        assert!((source_time_seconds(&clip, 0, 15) - 6.0 / 15.0).abs() < 1e-9);
    }

    #[test]
    fn external_source_path_is_absolute() {
        let entry = external_entry("m", Path::new("/media/clip.mp4"));
        let p = source_path(&entry, Path::new("/project")).unwrap();
        assert_eq!(p, PathBuf::from("/media/clip.mp4"));
    }

    #[test]
    fn source_decoder_reads_frames_from_one_open() {
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/testclip.mp4");
        let mut dec = SourceDecoder::open(&fixture).expect("fixture opens");
        // Two frames from a single open decoder — no reopen between them.
        let a = dec.frame_at_seconds(0.0).expect("frame 0");
        let b = dec.frame_at_seconds(0.1).expect("frame near 0.1s");
        assert_eq!((a.width, a.height), (160, 120));
        assert_eq!((b.width, b.height), (160, 120));
        assert_eq!(a.pixels.len(), 160 * 120 * 4);
    }

    #[test]
    fn export_project_with_fixture_roundtrips() {
        if !encoder_available() {
            eprintln!("skipping: no video encoder in linked ffmpeg");
            return;
        }
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/testclip.mp4");
        assert!(fixture.is_file(), "fixture missing: {}", fixture.display());

        let dir = temp_dir("export-project");
        let out = dir.join("out.mp4");
        let clip = full_frame_clip("c1", "m1", 0, 3);
        let timeline = single_video_timeline(clip);
        let mut manifest = MediaManifest::default();
        manifest.entries.push(external_entry("m1", &fixture));

        export_project(&timeline, &manifest, &dir, &out, 64, 48)
            .expect("real fixture export should succeed");

        assert!(std::fs::metadata(&out).unwrap().len() > 0);
        let decoded = decode_frame_rgba(&out, 0.0).expect("exported video decodes");
        assert_eq!(decoded.width, 64);
        assert_eq!(decoded.height, 48);
        // The composited frame is fully opaque (source fills the canvas).
        assert_eq!(decoded.pixels[3], 255);
    }
}

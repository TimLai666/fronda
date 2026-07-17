//! Pixel-render video export via statically-linked ffmpeg.
//!
//! Composites each timeline frame (render_core) then encodes the sequence to
//! an mp4. Decode and encode are the only platform pieces; all frame math and
//! compositing live in the pure `render_core::compositor`. ffmpeg is compiled
//! into the binary, so no ffmpeg executable is needed at runtime.
//!
//! Supports H.264 (default, always available), ProRes (native, .mov), and H.265
//! (libx265 when present, else H.264). Optional AAC audio mux, output-fps
//! frame-rate conversion, and still-image sources (via the `image` crate, since
//! the linked ffmpeg has no image decoders).

use core_model::{Clip, MediaManifest, MediaSource, Timeline};
use ffmpeg::format::sample::Type as SampleType;
use ffmpeg::format::{Pixel, Sample};
use ffmpeg::util::channel_layout::ChannelLayout;
use ffmpeg::Rational;
use ffmpeg_the_third as ffmpeg;
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

/// Error message for a cancelled export; hosts match with [`is_cancel_err`].
pub const EXPORT_CANCELED_ERR: &str = "Export canceled";

/// Whether an export error string is the cancellation sentinel.
pub fn is_cancel_err(error: &str) -> bool {
    error == EXPORT_CANCELED_ERR
}

/// Hidden sibling staging file for `dest` — same directory, so the final
/// [`commit_staged`] rename stays atomic. Mirrors Swift
/// `ExportService.stagingURL`: `.{stem}-{nonce}.partial[.{ext}]`.
pub fn staging_path(dest: &Path) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NONCE: AtomicU64 = AtomicU64::new(0);
    let stem = dest
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("export");
    let nonce = format!(
        "{}-{}",
        std::process::id(),
        NONCE.fetch_add(1, Ordering::Relaxed)
    );
    let name = match dest.extension().and_then(|e| e.to_str()) {
        Some(ext) if !ext.is_empty() => format!(".{stem}-{nonce}.partial.{ext}"),
        _ => format!(".{stem}-{nonce}.partial"),
    };
    dest.with_file_name(name)
}

/// Move the finished staging file over `dest`. `rename` replaces atomically on
/// POSIX; on Windows it fails when `dest` exists, so remove-then-rename there.
pub fn commit_staged(staging: &Path, dest: &Path) -> Result<(), String> {
    match std::fs::rename(staging, dest) {
        Ok(()) => Ok(()),
        Err(first) => {
            if dest.exists() {
                std::fs::remove_file(dest)
                    .map_err(|e| format!("replace {}: {e}", dest.display()))?;
                std::fs::rename(staging, dest)
                    .map_err(|e| format!("commit {}: {e}", dest.display()))
            } else {
                Err(format!("commit {}: {first}", dest.display()))
            }
        }
    }
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

/// Resolve the encoder for a requested codec. H.265 prefers `libx265` and falls
/// back to H.264 when it is absent — many Windows builds only expose the Media
/// Foundation HEVC encoder, which rejects our input, so producing a valid H.264
/// file beats a broken HEVC one.
fn find_video_encoder(codec: VideoCodec) -> Option<ffmpeg::codec::Codec> {
    match codec {
        // HDR must be a real 10-bit HEVC encoder — NO SDR/8-bit fallback
        // (Issue #138). `build` turns the None into an explicit error.
        VideoCodec::H265Hdr => ffmpeg::encoder::find_by_name("libx265"),
        VideoCodec::H265 => ffmpeg::encoder::find_by_name("libx265")
            .or_else(|| ffmpeg::encoder::find(ffmpeg::codec::Id::H264))
            .or_else(pick_encoder),
        other => ffmpeg::encoder::find(other.codec_id()).or_else(pick_encoder),
    }
}

fn err<T>(context: &str, e: ffmpeg::Error) -> Result<T, String> {
    Err(format!("{context}: {e}"))
}

/// AAC audio stream muxed alongside the video (optional).
struct AudioTrack {
    encoder: ffmpeg::codec::encoder::audio::Encoder,
    stream_index: usize,
    enc_time_base: Rational,
    ost_time_base: Rational,
    frame_size: usize,
    channels: usize,
    layout_mask: ffmpeg::util::channel_layout::ChannelLayoutMask,
    next_pts: i64,
}

/// Incremental RGBA → mp4 encoder. Feed frames with [`write_frame`], optionally
/// audio with [`write_audio`], then [`finish`] to flush and mux the trailer.
pub struct Mp4Encoder {
    octx: ffmpeg::format::context::Output,
    encoder: ffmpeg::codec::encoder::video::Encoder,
    scaler: ffmpeg::software::scaling::Context,
    width: u32,
    height: u32,
    pix: Pixel,
    stream_index: usize,
    enc_time_base: Rational,
    ost_time_base: Rational,
    next_pts: i64,
    audio: Option<AudioTrack>,
    finished: bool,
}

/// Supported export video codecs. ProRes writes 10-bit 4:2:2 to a `.mov`;
/// H.264/H.265 write 8-bit YUV420P to an `.mp4`. `H265Hdr` (Issue #138/#59)
/// writes 10-bit HEVC Main10 with BT.2020 + HLG color tags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoCodec {
    H264,
    H265,
    ProRes,
    /// HEVC Main10 for 10-bit HDR (BT.2020 primaries, ARIB-STD-B67 / HLG
    /// transfer). Requires a real 10-bit HEVC encoder (libx265) — never falls
    /// back to an 8-bit / SDR encode.
    H265Hdr,
}

impl VideoCodec {
    fn codec_id(self) -> ffmpeg::codec::Id {
        match self {
            VideoCodec::H264 => ffmpeg::codec::Id::H264,
            VideoCodec::H265 | VideoCodec::H265Hdr => ffmpeg::codec::Id::HEVC,
            VideoCodec::ProRes => ffmpeg::codec::Id::PRORES,
        }
    }

    fn pixel_format(self) -> Pixel {
        match self {
            VideoCodec::ProRes => Pixel::YUV422P10LE,
            VideoCodec::H265Hdr => Pixel::YUV420P10LE,
            _ => Pixel::YUV420P,
        }
    }

    /// Whether this codec produces a 10-bit HDR (BT.2020 + HLG) stream.
    fn is_hdr(self) -> bool {
        matches!(self, VideoCodec::H265Hdr)
    }
}

impl Mp4Encoder {
    /// Open `output` for writing H.264 video at `width`x`height` and `fps`.
    pub fn new(output: &Path, width: u32, height: u32, fps: i32) -> Result<Self, String> {
        Self::build(output, width, height, fps, None, VideoCodec::H264)
    }

    /// Like [`new`] but also muxes an AAC audio stream at `audio_rate` /
    /// `audio_channels`; feed samples with [`write_audio`].
    pub fn new_with_audio(
        output: &Path,
        width: u32,
        height: u32,
        fps: i32,
        audio_rate: u32,
        audio_channels: u16,
    ) -> Result<Self, String> {
        Self::build(
            output,
            width,
            height,
            fps,
            Some((audio_rate, audio_channels)),
            VideoCodec::H264,
        )
    }

    /// Like [`new_with_audio`] but with an explicit video codec.
    pub fn new_av_codec(
        output: &Path,
        width: u32,
        height: u32,
        fps: i32,
        audio: Option<(u32, u16)>,
        codec: VideoCodec,
    ) -> Result<Self, String> {
        Self::build(output, width, height, fps, audio, codec)
    }

    fn build(
        output: &Path,
        width: u32,
        height: u32,
        fps: i32,
        audio: Option<(u32, u16)>,
        video_codec: VideoCodec,
    ) -> Result<Self, String> {
        init_ffmpeg();
        let width = width & !1;
        let height = height & !1;
        if width == 0 || height == 0 {
            return Err("output dimensions must be >= 2".into());
        }
        if fps <= 0 {
            return Err("fps must be positive".into());
        }
        let pix = video_codec.pixel_format();
        let codec = match find_video_encoder(video_codec) {
            Some(c) => c,
            None if video_codec.is_hdr() => {
                // Never silently degrade HDR to SDR (Issue #138).
                return Err(
                    "HDR export (HEVC Main10) requires the libx265 10-bit encoder, \
                     which is not available in this ffmpeg build"
                        .into(),
                );
            }
            None => return Err("no usable video encoder in linked ffmpeg".into()),
        };

        let mut octx = match ffmpeg::format::output(output) {
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
        enc.set_format(pix);
        enc.set_time_base(enc_time_base);
        enc.set_frame_rate(Some(Rational(fps, 1)));
        enc.set_gop(12);
        enc.set_bit_rate(bit_rate_for(width, height, fps));
        if video_codec.is_hdr() {
            // Tag the stream BT.2020 primaries + HLG (ARIB-STD-B67) transfer so
            // players render it as HDR (Issue #138). color_primaries / color_trc
            // have no typed setter in ffmpeg-the-third; write the codec-context
            // fields directly, exactly as `set_colorspace` does internally.
            enc.set_colorspace(ffmpeg::color::Space::BT2020NCL);
            enc.set_color_range(ffmpeg::color::Range::MPEG);
            unsafe {
                let ctx = enc.as_mut_ptr();
                (*ctx).color_primaries = ffmpeg::color::Primaries::BT2020.into();
                (*ctx).color_trc = ffmpeg::color::TransferCharacteristic::ARIB_STD_B67.into();
            }
        }
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

        // Build the audio encoder + stream before write_header (both streams
        // must exist first).
        let mut audio_track = match audio {
            Some((rate, channels)) => {
                Some(build_audio_track(&mut octx, rate, channels, global_header)?)
            }
            None => None,
        };

        if let Err(e) = octx.write_header() {
            return err("write header", e);
        }
        let ost_time_base = octx
            .stream(stream_index)
            .map(|s| s.time_base())
            .unwrap_or(enc_time_base);
        if let Some(track) = audio_track.as_mut() {
            track.ost_time_base = octx
                .stream(track.stream_index)
                .map(|s| s.time_base())
                .unwrap_or(track.enc_time_base);
        }

        let scaler = match ffmpeg::software::scaling::Context::get(
            Pixel::RGBA,
            width,
            height,
            pix,
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
            pix,
            stream_index,
            enc_time_base,
            ost_time_base,
            next_pts: 0,
            audio: audio_track,
            finished: false,
        })
    }

    /// Encode interleaved f32 PCM (at the audio rate/channels passed to
    /// [`new_with_audio`]) into the audio stream. No-op without an audio track.
    pub fn write_audio(&mut self, pcm: &[f32]) -> Result<(), String> {
        let Some(track) = self.audio.as_mut() else {
            return Ok(());
        };
        let ch = track.channels;
        let fs = track.frame_size.max(1);
        for chunk in pcm.chunks(fs * ch) {
            let n = chunk.len() / ch;
            let mut frame =
                ffmpeg::frame::Audio::new(Sample::F32(SampleType::Planar), fs, track.layout_mask);
            for c in 0..ch {
                let plane = frame.plane_mut::<f32>(c);
                for (i, slot) in plane.iter_mut().enumerate().take(fs) {
                    *slot = if i < n { chunk[i * ch + c] } else { 0.0 };
                }
            }
            frame.set_pts(Some(track.next_pts));
            track.next_pts += fs as i64;
            if let Err(e) = track.encoder.send_frame(&frame) {
                return err("send audio frame", e);
            }
            Self::drain_audio(track, &mut self.octx)?;
        }
        Ok(())
    }

    fn drain_audio(
        track: &mut AudioTrack,
        octx: &mut ffmpeg::format::context::Output,
    ) -> Result<(), String> {
        let mut packet = ffmpeg::codec::packet::Packet::empty();
        while track.encoder.receive_packet(&mut packet).is_ok() {
            packet.set_stream(track.stream_index);
            packet.rescale_ts(track.enc_time_base, track.ost_time_base);
            if let Err(e) = packet.write_interleaved(octx) {
                return err("write audio packet", e);
            }
        }
        Ok(())
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
                // The compositor buffer is STRAIGHT alpha; swscale RGBA→YUV drops
                // alpha, so composite over black first (video has an opaque black
                // background). Opaque pixels are unchanged.
                for x in 0..self.width as usize {
                    let i = x * 4;
                    let a = src[i + 3] as u16;
                    dst[i] = (src[i] as u16 * a / 255) as u8;
                    dst[i + 1] = (src[i + 1] as u16 * a / 255) as u8;
                    dst[i + 2] = (src[i + 2] as u16 * a / 255) as u8;
                    dst[i + 3] = 255;
                }
            }
        }
        let mut yuv = ffmpeg::frame::Video::new(self.pix, self.width, self.height);
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
        if let Some(track) = self.audio.as_mut() {
            if let Err(e) = track.encoder.send_eof() {
                return err("send audio eof", e);
            }
            Self::drain_audio(track, &mut self.octx)?;
        }
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

/// Build and open the AAC audio encoder + add its stream to `octx`. The mix rate
/// must match `rate` so no resampling is needed (packed f32 → planar frames).
fn build_audio_track(
    octx: &mut ffmpeg::format::context::Output,
    rate: u32,
    channels: u16,
    global_header: bool,
) -> Result<AudioTrack, String> {
    let acodec =
        ffmpeg::encoder::find(ffmpeg::codec::Id::AAC).ok_or("no AAC encoder in linked ffmpeg")?;
    let layout = ChannelLayout::default_for_channels(channels as u32);
    let layout_mask = layout.mask().ok_or("audio channel layout has no mask")?;
    let enc_time_base = Rational(1, rate as i32);

    let actx = ffmpeg::codec::context::Context::new_with_codec(acodec);
    let mut aenc = match actx.encoder().audio() {
        Ok(e) => e,
        Err(e) => return err("create audio encoder", e),
    };
    aenc.set_rate(rate as i32);
    aenc.set_ch_layout(layout);
    aenc.set_format(Sample::F32(SampleType::Planar));
    aenc.set_bit_rate(128_000);
    aenc.set_time_base(enc_time_base);
    if global_header {
        aenc.set_flags(ffmpeg::codec::Flags::GLOBAL_HEADER);
    }
    let encoder = match aenc.open_as(acodec) {
        Ok(e) => e,
        Err(e) => return err("open audio encoder", e),
    };
    // AAC reports 1024; fall back to 1024 if a codec reports 0 (variable).
    let frame_size = match encoder.frame_size() as usize {
        0 => 1024,
        n => n,
    };

    let stream_index;
    {
        let mut ost = match octx.add_stream(acodec) {
            Ok(s) => s,
            Err(e) => return err("add audio stream", e),
        };
        ost.set_parameters(ffmpeg::codec::Parameters::from(&encoder));
        ost.set_time_base(enc_time_base);
        stream_index = ost.index();
    }

    Ok(AudioTrack {
        encoder,
        stream_index,
        enc_time_base,
        ost_time_base: enc_time_base,
        frame_size,
        channels: channels as usize,
        layout_mask,
        next_pts: 0,
    })
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

/// Extensions decoded via the `image` crate — the linked ffmpeg has no image
/// decoders (e.g. no PNG decoder), so stills go through `image`, not ffmpeg.
fn is_image_source(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .as_deref(),
        Some("png" | "jpg" | "jpeg" | "bmp" | "gif" | "webp")
    )
}

/// Load a still image as native-resolution RGBA via the `image` crate.
fn load_still_rgba(path: &Path) -> Option<RgbaImage> {
    let img = image::open(path).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    Some(RgbaImage {
        width: w as usize,
        height: h as usize,
        pixels: img.into_raw(),
    })
}

/// ffmpeg-backed video source opened once and reused across frame requests.
pub struct VideoDecoder {
    ictx: ffmpeg::format::context::Input,
    decoder: ffmpeg::decoder::Video,
    scaler: ffmpeg::software::scaling::Context,
    video_index: usize,
    width: u32,
    height: u32,
}

/// A source opened once and reused across many frame requests, avoiding the
/// per-frame file-open + stream-probe cost of [`decode_frame_rgba`]. Video
/// decodes via ffmpeg; still images (which the linked ffmpeg cannot decode) are
/// loaded once via the `image` crate and returned for every frame.
pub enum SourceDecoder {
    Video(VideoDecoder),
    Still(RgbaImage),
}

impl SourceDecoder {
    /// Open `source`. `None` when it has no decodable video / image.
    pub fn open(source: &Path) -> Option<Self> {
        if is_image_source(source) {
            return load_still_rgba(source).map(SourceDecoder::Still);
        }
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
        Some(SourceDecoder::Video(VideoDecoder {
            ictx,
            decoder,
            scaler,
            video_index,
            width,
            height,
        }))
    }

    /// Decode the frame nearest `time_seconds` as native-resolution RGBA. A still
    /// returns its single image for any time.
    pub fn frame_at_seconds(&mut self, time_seconds: f64) -> Option<RgbaImage> {
        match self {
            SourceDecoder::Still(img) => Some(img.clone()),
            SourceDecoder::Video(v) => v.frame_at_seconds(time_seconds),
        }
    }
}

impl VideoDecoder {
    fn frame_at_seconds(&mut self, time_seconds: f64) -> Option<RgbaImage> {
        if time_seconds > 0.0 {
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
/// source's native resolution. Still images load via the `image` crate; video
/// decodes via ffmpeg. `None` on any decode failure.
pub fn decode_frame_rgba(source: &Path, time_seconds: f64) -> Option<RgbaImage> {
    if is_image_source(source) {
        return load_still_rgba(source);
    }
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
    use core_model::{ClipType, Crop, Interpolation, MediaManifestEntry, Track, Transform};

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir()
            .join("fronda-video-export-tests")
            .join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn staging_path_is_hidden_partial_sibling() {
        let dest = Path::new("/out/dir/Timeline.mp4");
        let staging = staging_path(dest);
        assert_eq!(staging.parent(), dest.parent(), "same directory");
        let name = staging.file_name().unwrap().to_str().unwrap();
        assert!(name.starts_with(".Timeline-"), "hidden, stem-based: {name}");
        assert!(name.contains(".partial."), "marked partial: {name}");
        assert!(name.ends_with(".mp4"), "keeps the extension: {name}");
        assert_ne!(staging, staging_path(dest), "nonce makes each unique");

        let bare = staging_path(Path::new("/out/noext"));
        assert!(bare.file_name().unwrap().to_str().unwrap().ends_with(".partial"));
    }

    #[test]
    fn commit_staged_replaces_existing_destination() {
        let dir = temp_dir("commit-staged");
        let dest = dir.join("out.mp4");
        std::fs::write(&dest, b"old").unwrap();
        let staging = staging_path(&dest);
        std::fs::write(&staging, b"new").unwrap();
        commit_staged(&staging, &dest).unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), b"new");
        assert!(!staging.exists(), "staging file consumed");

        // Fresh destination (no pre-existing file) also commits.
        let dest2 = dir.join("fresh.mp4");
        let staging2 = staging_path(&dest2);
        std::fs::write(&staging2, b"data").unwrap();
        commit_staged(&staging2, &dest2).unwrap();
        assert_eq!(std::fs::read(&dest2).unwrap(), b"data");
    }

    #[test]
    fn cancel_err_sentinel_matches() {
        assert!(is_cancel_err(EXPORT_CANCELED_ERR));
        assert!(!is_cancel_err("disk full"));
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
            multicam_group_id: None,
            text_animation: None,
            word_timings: None,
        }
    }

    fn single_video_timeline(clip: Clip) -> Timeline {
        Timeline {
            id: String::new(),
            name: String::new(),
            fps: 15,
            width: 64,
            height: 48,
            tracks: vec![Track {
                id: "v1".into(),
                r#type: ClipType::Video,
                muted: false,
                hidden: false,
                sync_locked: false,
                display_height: 50.0,
                clips: vec![clip],
            }],
            settings_configured: true,
            selected_clip_ids: std::collections::HashSet::new(),
            transcription_language: None,
            folder_id: None,
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
            generation_status: None,
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

    /// Try a full encode+decode round-trip for a codec; `Ok(true)` if it works,
    /// `Ok(false)` if the encoder is unusable in this build (e.g. an MF encoder
    /// that rejects the input). Test helper.
    fn try_codec(dir: &std::path::Path, codec: VideoCodec, ext: &str) -> bool {
        let out = dir.join(format!("clip-{codec:?}.{ext}"));
        let mut enc = match Mp4Encoder::new_av_codec(&out, 64, 48, 15, None, codec) {
            Ok(e) => e,
            Err(_) => return false,
        };
        for _ in 0..5 {
            if enc
                .write_frame(&RgbaImage::solid(64, 48, [200, 30, 30, 255]))
                .is_err()
            {
                return false;
            }
        }
        if enc.finish().is_err() {
            return false;
        }
        std::fs::metadata(&out)
            .map(|m| m.len() > 0)
            .unwrap_or(false)
            && decode_frame_rgba(&out, 0.0).is_some()
    }

    #[test]
    fn h264_export_round_trips() {
        if !encoder_available() {
            return;
        }
        let dir = temp_dir("codec-h264");
        // H.264 must always work (it's the default and universally available).
        assert!(
            try_codec(&dir, VideoCodec::H264, "mp4"),
            "H.264 round-trips"
        );
    }

    #[test]
    fn codec_selection_is_usable_or_reported() {
        if !encoder_available() {
            return;
        }
        let dir = temp_dir("codec-all");
        // H.265 (often only via Media Foundation) and ProRes may or may not work
        // in a given ffmpeg build; log the support matrix rather than assert.
        for (codec, ext) in [(VideoCodec::H265, "mp4"), (VideoCodec::ProRes, "mov")] {
            eprintln!("codec {codec:?} usable = {}", try_codec(&dir, codec, ext));
        }
    }

    /// Probe a written video stream's (pixel format, primaries, transfer).
    /// Test helper for the HDR color-tag assertion.
    fn probe_video_color(
        path: &std::path::Path,
    ) -> Option<(
        Pixel,
        ffmpeg::color::Primaries,
        ffmpeg::color::TransferCharacteristic,
    )> {
        init_ffmpeg();
        let ictx = ffmpeg::format::input(path).ok()?;
        let stream = ictx.streams().best(ffmpeg::media::Type::Video)?;
        let decoder = ffmpeg::codec::context::Context::from_parameters(stream.parameters())
            .ok()?
            .decoder()
            .video()
            .ok()?;
        Some((
            decoder.format(),
            decoder.color_primaries(),
            decoder.color_transfer_characteristic(),
        ))
    }

    #[test]
    fn hdr_export_is_10bit_bt2020_hlg_or_errors() {
        if !encoder_available() {
            return;
        }
        let dir = temp_dir("codec-hdr");
        let out = dir.join("hdr.mp4");
        match Mp4Encoder::new_av_codec(&out, 64, 48, 15, None, VideoCodec::H265Hdr) {
            Ok(mut enc) => {
                for _ in 0..5 {
                    enc.write_frame(&RgbaImage::solid(64, 48, [200, 30, 30, 255]))
                        .expect("HDR frame encodes");
                }
                enc.finish().expect("HDR mux finishes");
                assert!(std::fs::metadata(&out).unwrap().len() > 0);
                // The 10-bit pixel format and BT.2020 + HLG tags survive a
                // re-decode — no silent SDR / 8-bit degrade.
                let (pix, prim, trc) = probe_video_color(&out).expect("re-decode the HDR stream");
                assert_eq!(pix, Pixel::YUV420P10LE, "HDR output must be 10-bit");
                assert_eq!(
                    prim,
                    ffmpeg::color::Primaries::BT2020,
                    "HDR output must tag BT.2020 primaries"
                );
                assert_eq!(
                    trc,
                    ffmpeg::color::TransferCharacteristic::ARIB_STD_B67,
                    "HDR output must tag HLG transfer"
                );
            }
            Err(e) => {
                // Acceptable ONLY as an explicit HDR error — never a silent SDR
                // file. Builds without libx265 hit this branch.
                assert!(
                    e.contains("HDR") || e.contains("libx265"),
                    "HDR unavailability must be an explicit error, got: {e}"
                );
                assert!(
                    !out.exists() || std::fs::metadata(&out).map(|m| m.len()).unwrap_or(0) == 0,
                    "a failed HDR export must not leave a (misleading SDR) file"
                );
                eprintln!("HDR encoder unavailable (expected without libx265): {e}");
            }
        }
    }

    #[test]
    fn source_decoder_decodes_a_still_image() {
        // Image clips reference stills; SourceDecoder loads them via the `image`
        // crate (the linked ffmpeg has no PNG decoder).
        let dir = temp_dir("still");
        let png = dir.join("still.png");
        let mut img = image::RgbaImage::new(32, 24);
        for p in img.pixels_mut() {
            *p = image::Rgba([10, 200, 40, 255]);
        }
        img.save_with_format(&png, image::ImageFormat::Png).unwrap();

        let mut dec = SourceDecoder::open(&png).expect("PNG opens as a source");
        let frame = dec.frame_at_seconds(0.0).expect("still decodes");
        assert_eq!((frame.width, frame.height), (32, 24));
        // Green channel dominates (the fill was green).
        assert!(frame.pixels[1] > frame.pixels[0] && frame.pixels[1] > frame.pixels[2]);
    }

    #[test]
    fn encode_video_plus_audio_muxes_both_streams() {
        if ffmpeg::encoder::find(ffmpeg::codec::Id::AAC).is_none() || !encoder_available() {
            eprintln!("skipping: no AAC or video encoder in linked ffmpeg");
            return;
        }
        let dir = temp_dir("av");
        let out = dir.join("av.mp4");
        let (w, h, fps) = (64u32, 48u32, 15i32);

        let mut enc = Mp4Encoder::new_with_audio(&out, w, h, fps, 48_000, 2).expect("open av");
        for _ in 0..10 {
            enc.write_frame(&RgbaImage::solid(
                w as usize,
                h as usize,
                [200, 30, 30, 255],
            ))
            .unwrap();
        }
        // ~0.5s of a quiet stereo tone at 48 kHz.
        let samples: Vec<f32> = (0..48_000)
            .flat_map(|i| {
                let v = ((i as f32 * 0.05).sin()) * 0.2;
                [v, v]
            })
            .collect();
        enc.write_audio(&samples).unwrap();
        enc.finish().unwrap();

        assert!(std::fs::metadata(&out).unwrap().len() > 0);
        // Both streams must survive the mux and re-decode.
        let audio = crate::audio_export::decode_audio_pcm(&out, 48_000, 2);
        assert!(
            audio.as_ref().is_some_and(|p| !p.is_empty()),
            "audio stream decodes"
        );
        assert!(
            decode_frame_rgba(&out, 0.0).is_some(),
            "video stream decodes"
        );
    }

    #[test]
    fn source_decoder_reads_frames_from_one_open() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/testclip.mp4");
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
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/testclip.mp4");
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

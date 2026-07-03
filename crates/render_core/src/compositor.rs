//! CPU frame compositor — flattens a timeline frame into an RGBA8 pixel buffer.
//!
//! This is the pixel-compositing core the video exporter and software preview
//! build on. Media decoding stays a platform adapter: callers pass a
//! `fetch_source` closure that returns each clip's decoded RGBA frame, so this
//! module is pure and fully unit-testable with synthetic sources.
//!
//! v1 covers layer order, per-clip transform (position + scale), crop, and
//! opacity with source-over alpha blending (nearest-neighbour sampling).
//! Rotation, blend modes, effects, and bilinear sampling are follow-ups.

use core_model::{Clip, ClipType, MediaManifest, Timeline};

/// An RGBA8 image (row-major, 4 bytes/pixel: R, G, B, A).
#[derive(Debug, Clone, PartialEq)]
pub struct RgbaImage {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
}

impl RgbaImage {
    /// A transparent-black image of the given size.
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            pixels: vec![0; width * height * 4],
        }
    }

    /// A solid-colour image (useful for tests / mattes).
    pub fn solid(width: usize, height: usize, rgba: [u8; 4]) -> Self {
        let mut pixels = Vec::with_capacity(width * height * 4);
        for _ in 0..width * height {
            pixels.extend_from_slice(&rgba);
        }
        Self {
            width,
            height,
            pixels,
        }
    }

    #[inline]
    fn pixel(&self, x: usize, y: usize) -> [u8; 4] {
        let i = (y * self.width + x) * 4;
        [
            self.pixels[i],
            self.pixels[i + 1],
            self.pixels[i + 2],
            self.pixels[i + 3],
        ]
    }
}

/// Composite `src` into `canvas` scaled to fill the pixel rect `dst`, sampling
/// only the normalized sub-rectangle `src_region` of the source (for crop), with
/// `opacity` (0..1) applied and source-over alpha blending. Nearest-neighbour.
///
/// `dst` and `src_region` are `(x, y, w, h)`. `dst` is in canvas pixels and may
/// extend past the canvas edges (it is clipped). `src_region` is fractions in
/// `0..1` of the source.
#[allow(clippy::too_many_arguments)]
pub fn blit_scaled(
    canvas: &mut RgbaImage,
    src: &RgbaImage,
    src_region: (f64, f64, f64, f64),
    dst: (f64, f64, f64, f64),
    opacity: f64,
) {
    let (dx, dy, dw, dh) = dst;
    if dw <= 0.0 || dh <= 0.0 || src.width == 0 || src.height == 0 || opacity <= 0.0 {
        return;
    }
    let opacity = opacity.clamp(0.0, 1.0);
    let (sx0, sy0, sw, sh) = src_region;

    // Canvas pixel span covered by the destination rect (clipped to canvas).
    let x_start = dx.floor().max(0.0) as usize;
    let y_start = dy.floor().max(0.0) as usize;
    let x_end = ((dx + dw).ceil() as isize).clamp(0, canvas.width as isize) as usize;
    let y_end = ((dy + dh).ceil() as isize).clamp(0, canvas.height as isize) as usize;

    for cy in y_start..y_end {
        // v in 0..1 across the destination height → source row.
        let v = (cy as f64 + 0.5 - dy) / dh;
        if !(0.0..1.0).contains(&v) {
            continue;
        }
        let sfy = (sy0 + v * sh) * src.height as f64;
        let sy = (sfy as isize).clamp(0, src.height as isize - 1) as usize;
        for cx in x_start..x_end {
            let u = (cx as f64 + 0.5 - dx) / dw;
            if !(0.0..1.0).contains(&u) {
                continue;
            }
            let sfx = (sx0 + u * sw) * src.width as f64;
            let sx = (sfx as isize).clamp(0, src.width as isize - 1) as usize;

            let [sr, sg, sb, sa] = src.pixel(sx, sy);
            let a = (sa as f64 / 255.0) * opacity;
            if a <= 0.0 {
                continue;
            }
            let ci = (cy * canvas.width + cx) * 4;
            for k in 0..3 {
                let s = [sr, sg, sb][k] as f64;
                let d = canvas.pixels[ci + k] as f64;
                canvas.pixels[ci + k] = (s * a + d * (1.0 - a)).round().clamp(0.0, 255.0) as u8;
            }
            // Straight-alpha over: out_a = src_a + dst_a*(1-src_a).
            let da = canvas.pixels[ci + 3] as f64 / 255.0;
            let out_a = a + da * (1.0 - a);
            canvas.pixels[ci + 3] = (out_a * 255.0).round().clamp(0.0, 255.0) as u8;
        }
    }
}

/// Visual clips on `timeline` that are on screen at `frame`, bottom track first
/// (render order), skipping text/shape overlays (not composited by v1) and clips
/// with no media.
fn visible_clips(timeline: &Timeline, frame: i64) -> Vec<&Clip> {
    let mut out: Vec<&Clip> = Vec::new();
    // Video tracks bottom-to-top = the order they appear in `tracks`.
    for track in &timeline.tracks {
        if track.r#type == ClipType::Audio || track.hidden {
            continue;
        }
        for clip in &track.clips {
            if clip.media_ref.is_empty()
                || matches!(clip.media_type, ClipType::Text | ClipType::Shape)
            {
                continue;
            }
            if frame >= clip.start_frame && frame < clip.start_frame + clip.duration_frames {
                out.push(clip);
            }
        }
    }
    out
}

/// Flatten `timeline` at `frame` into a `width`×`height` RGBA image.
///
/// `fetch_source(clip)` returns the clip's decoded RGBA frame (the decode/seek
/// is the caller's platform adapter). Each visible clip is placed by its
/// transform, cropped, faded by opacity, and blended over the layers below.
pub fn compose_frame(
    timeline: &Timeline,
    _manifest: &MediaManifest,
    frame: i64,
    width: usize,
    height: usize,
    mut fetch_source: impl FnMut(&Clip) -> Option<RgbaImage>,
) -> RgbaImage {
    let mut canvas = RgbaImage::new(width, height);
    let (cw, ch) = (width as f64, height as f64);

    for clip in visible_clips(timeline, frame) {
        let Some(src) = fetch_source(clip) else {
            continue;
        };
        let t = &clip.transform;
        let dw = t.width * cw;
        let dh = t.height * ch;
        let dx = t.center_x * cw - dw / 2.0;
        let dy = t.center_y * ch - dh / 2.0;

        // Crop → the source sub-rectangle that stays visible.
        let c = &clip.crop;
        let src_region = (
            c.left,
            c.top,
            (1.0 - c.left - c.right).max(0.0),
            (1.0 - c.top - c.bottom).max(0.0),
        );

        blit_scaled(
            &mut canvas,
            &src,
            src_region,
            (dx, dy, dw, dh),
            clip.opacity,
        );
    }

    canvas
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_model::{Clip, Crop, Interpolation, Track, Transform};

    fn clip(id: &str, media_ref: &str, start: i64, dur: i64) -> Clip {
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

    fn tl(clips: Vec<Clip>) -> Timeline {
        Timeline {
            fps: 30,
            width: 4,
            height: 4,
            tracks: vec![Track {
                id: "v1".into(),
                r#type: ClipType::Video,
                muted: false,
                hidden: false,
                sync_locked: false,
                clips,
            }],
            settings_configured: true,
            selected_clip_ids: std::collections::HashSet::new(),
            transcription_language: None,
            compound_timelines: std::collections::HashMap::new(),
        }
    }

    fn px(img: &RgbaImage, x: usize, y: usize) -> [u8; 4] {
        img.pixel(x, y)
    }

    #[test]
    fn blit_fills_dst_rect_opaque() {
        let mut canvas = RgbaImage::new(4, 4);
        let red = RgbaImage::solid(2, 2, [255, 0, 0, 255]);
        // Fill the whole 4x4 canvas.
        blit_scaled(&mut canvas, &red, (0.0, 0.0, 1.0, 1.0), (0.0, 0.0, 4.0, 4.0), 1.0);
        for y in 0..4 {
            for x in 0..4 {
                assert_eq!(px(&canvas, x, y), [255, 0, 0, 255], "({x},{y})");
            }
        }
    }

    #[test]
    fn blit_half_opacity_blends_over_black() {
        let mut canvas = RgbaImage::solid(2, 2, [0, 0, 0, 255]);
        let white = RgbaImage::solid(1, 1, [255, 255, 255, 255]);
        blit_scaled(&mut canvas, &white, (0.0, 0.0, 1.0, 1.0), (0.0, 0.0, 2.0, 2.0), 0.5);
        // 255*0.5 + 0*0.5 = 127.5 → 128.
        assert_eq!(px(&canvas, 0, 0), [128, 128, 128, 255]);
    }

    #[test]
    fn compose_places_clip_by_transform() {
        // A clip filling the top-left quadrant (center 0.25,0.25, size 0.5,0.5).
        let mut c = clip("c1", "m1", 0, 30);
        c.transform = Transform::from_top_left(0.0, 0.0, 0.5, 0.5);
        let timeline = tl(vec![c]);
        let out = compose_frame(&timeline, &MediaManifest::default(), 0, 4, 4, |_| {
            Some(RgbaImage::solid(2, 2, [0, 0, 255, 255]))
        });
        // Top-left 2x2 is blue; bottom-right stays transparent.
        assert_eq!(px(&out, 0, 0), [0, 0, 255, 255]);
        assert_eq!(px(&out, 1, 1), [0, 0, 255, 255]);
        assert_eq!(px(&out, 3, 3), [0, 0, 0, 0]);
    }

    #[test]
    fn compose_layers_top_over_bottom() {
        let bg = clip("bg", "m1", 0, 30); // full frame (default transform)
        let mut top = clip("top", "m2", 0, 30);
        top.transform = Transform::from_top_left(0.0, 0.0, 0.5, 0.5);
        let timeline = tl(vec![bg, top]); // bg first (bottom), top later (above)
        let out = compose_frame(&timeline, &MediaManifest::default(), 0, 4, 4, |c| {
            if c.id == "bg" {
                Some(RgbaImage::solid(4, 4, [255, 0, 0, 255])) // red
            } else {
                Some(RgbaImage::solid(2, 2, [0, 255, 0, 255])) // green
            }
        });
        assert_eq!(px(&out, 0, 0), [0, 255, 0, 255], "top-left is the green overlay");
        assert_eq!(px(&out, 3, 3), [255, 0, 0, 255], "elsewhere is the red bg");
    }

    #[test]
    fn compose_skips_clip_outside_its_frame_range() {
        let c = clip("c1", "m1", 10, 5); // visible only on frames 10..15
        let timeline = tl(vec![c]);
        let mut fetched = false;
        let out = compose_frame(&timeline, &MediaManifest::default(), 0, 4, 4, |_| {
            fetched = true;
            Some(RgbaImage::solid(4, 4, [1, 2, 3, 255]))
        });
        assert!(!fetched, "clip not fetched when off-frame");
        assert_eq!(px(&out, 0, 0), [0, 0, 0, 0], "empty canvas");
    }
}

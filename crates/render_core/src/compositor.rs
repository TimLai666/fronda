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

use core_model::{BlendMode, Clip, ClipType, MediaManifest, Timeline};

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

/// Separable blend function for one colour channel (both in `0..=1`). Issue #203.
/// The four non-separable HSL modes route through [`blend_rgb`] instead; here
/// they degenerate to `Normal` (a single channel can't carry HSL semantics).
pub fn blend_channel(mode: BlendMode, s: f64, d: f64) -> f64 {
    use BlendMode::*;
    let r = match mode {
        Normal | Hue | Saturation | Color | Luminosity => s,
        Multiply => s * d,
        Screen => 1.0 - (1.0 - s) * (1.0 - d),
        Overlay => {
            if d <= 0.5 {
                2.0 * s * d
            } else {
                1.0 - 2.0 * (1.0 - s) * (1.0 - d)
            }
        }
        Darken => s.min(d),
        Lighten => s.max(d),
        ColorDodge => {
            if s >= 1.0 {
                1.0
            } else {
                (d / (1.0 - s)).min(1.0)
            }
        }
        ColorBurn => {
            if s <= 0.0 {
                0.0
            } else {
                1.0 - ((1.0 - d) / s).min(1.0)
            }
        }
        HardLight => {
            if s <= 0.5 {
                2.0 * s * d
            } else {
                1.0 - 2.0 * (1.0 - s) * (1.0 - d)
            }
        }
        SoftLight => {
            if s <= 0.5 {
                d - (1.0 - 2.0 * s) * d * (1.0 - d)
            } else {
                let dd = if d <= 0.25 {
                    ((16.0 * d - 12.0) * d + 4.0) * d
                } else {
                    d.sqrt()
                };
                d + (2.0 * s - 1.0) * (dd - d)
            }
        }
        Difference => (s - d).abs(),
        Exclusion => s + d - 2.0 * s * d,
    };
    r.clamp(0.0, 1.0)
}

// ── Non-separable (HSL) blend helpers, per the W3C Compositing spec ──────────

fn lum(c: [f64; 3]) -> f64 {
    0.3 * c[0] + 0.59 * c[1] + 0.11 * c[2]
}

/// Clamp a colour into `[0,1]` per channel while preserving its luminosity.
fn clip_color(c: [f64; 3]) -> [f64; 3] {
    let l = lum(c);
    let n = c[0].min(c[1]).min(c[2]);
    let x = c[0].max(c[1]).max(c[2]);
    let mut out = c;
    if n < 0.0 && l - n > 0.0 {
        for v in &mut out {
            *v = l + (*v - l) * l / (l - n);
        }
    }
    if x > 1.0 && x - l > 0.0 {
        for v in &mut out {
            *v = l + (*v - l) * (1.0 - l) / (x - l);
        }
    }
    out
}

fn set_lum(c: [f64; 3], l: f64) -> [f64; 3] {
    let d = l - lum(c);
    clip_color([c[0] + d, c[1] + d, c[2] + d])
}

fn sat(c: [f64; 3]) -> f64 {
    c[0].max(c[1]).max(c[2]) - c[0].min(c[1]).min(c[2])
}

/// Scale a colour's saturation to `s`, keeping its relative channel ordering.
fn set_sat(c: [f64; 3], s: f64) -> [f64; 3] {
    let mut idx = [0usize, 1, 2];
    idx.sort_by(|&a, &b| c[a].partial_cmp(&c[b]).unwrap_or(std::cmp::Ordering::Equal));
    let (imin, imid, imax) = (idx[0], idx[1], idx[2]);
    let mut out = [0.0; 3];
    if c[imax] > c[imin] {
        out[imid] = (c[imid] - c[imin]) * s / (c[imax] - c[imin]);
        out[imax] = s;
    }
    out[imin] = 0.0;
    out
}

/// Blend a source RGB triple over a destination one (both `0..=1`). Separable
/// modes apply [`blend_channel`] per channel; the four HSL modes (Hue,
/// Saturation, Color, Luminosity) use whole-pixel non-separable math. Issue #203.
pub fn blend_rgb(mode: BlendMode, s: [f64; 3], d: [f64; 3]) -> [f64; 3] {
    use BlendMode::*;
    match mode {
        Hue => set_lum(set_sat(s, sat(d)), lum(d)),
        Saturation => set_lum(set_sat(d, sat(s)), lum(d)),
        Color => set_lum(s, lum(d)),
        Luminosity => set_lum(d, lum(s)),
        other => [
            blend_channel(other, s[0], d[0]),
            blend_channel(other, s[1], d[1]),
            blend_channel(other, s[2], d[2]),
        ],
    }
}

/// Bilinearly sample `src` at continuous pixel coordinates `(fx, fy)`, returning
/// a **premultiplied** RGBA in `0..=1`. Premultiplying before interpolation keeps
/// edges against transparent pixels from fringing toward the transparent colour.
/// Coordinates are clamped to the source bounds (edge clamp).
fn sample_bilinear(src: &RgbaImage, fx: f64, fy: f64) -> [f64; 4] {
    let x = fx.clamp(0.0, src.width as f64 - 1.0);
    let y = fy.clamp(0.0, src.height as f64 - 1.0);
    let x0 = x.floor() as usize;
    let y0 = y.floor() as usize;
    let x1 = (x0 + 1).min(src.width - 1);
    let y1 = (y0 + 1).min(src.height - 1);
    let tx = x - x0 as f64;
    let ty = y - y0 as f64;
    let premul = |xx: usize, yy: usize| {
        let [r, g, b, a] = src.pixel(xx, yy);
        let af = a as f64 / 255.0;
        [
            r as f64 / 255.0 * af,
            g as f64 / 255.0 * af,
            b as f64 / 255.0 * af,
            af,
        ]
    };
    let p00 = premul(x0, y0);
    let p10 = premul(x1, y0);
    let p01 = premul(x0, y1);
    let p11 = premul(x1, y1);
    let mut out = [0.0; 4];
    for k in 0..4 {
        let top = p00[k] + (p10[k] - p00[k]) * tx;
        let bot = p01[k] + (p11[k] - p01[k]) * tx;
        out[k] = top + (bot - top) * ty;
    }
    out
}

/// Composite `src` into `canvas` scaled to fill the pixel rect `dst`, rotated
/// `rotation_degrees` about the rect's centre, sampling only the normalized
/// sub-rectangle `src_region` of the source (for crop), with `opacity` (0..1)
/// and source-over alpha blending. Bilinear (inverse-mapped, premultiplied).
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
    rotation_degrees: f64,
    blend: BlendMode,
) {
    let (dx, dy, dw, dh) = dst;
    if dw <= 0.0 || dh <= 0.0 || src.width == 0 || src.height == 0 || opacity <= 0.0 {
        return;
    }
    let opacity = opacity.clamp(0.0, 1.0);
    let (sx0, sy0, sw, sh) = src_region;
    let (hw, hh) = (dw / 2.0, dh / 2.0);
    let (cx, cy) = (dx + hw, dy + hh);
    let rad = rotation_degrees * std::f64::consts::PI / 180.0;
    let (sin, cos) = (rad.sin(), rad.cos());

    // Canvas AABB of the rotated destination rect (clipped to the canvas).
    let corners = [(-hw, -hh), (hw, -hh), (hw, hh), (-hw, hh)];
    let (mut min_x, mut min_y, mut max_x, mut max_y) =
        (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for (lx, ly) in corners {
        let wx = cx + lx * cos - ly * sin;
        let wy = cy + lx * sin + ly * cos;
        min_x = min_x.min(wx);
        min_y = min_y.min(wy);
        max_x = max_x.max(wx);
        max_y = max_y.max(wy);
    }
    let x_start = min_x.floor().max(0.0) as usize;
    let y_start = min_y.floor().max(0.0) as usize;
    let x_end = ((max_x.ceil() as isize).clamp(0, canvas.width as isize)) as usize;
    let y_end = ((max_y.ceil() as isize).clamp(0, canvas.height as isize)) as usize;

    for py in y_start..y_end {
        for px in x_start..x_end {
            // Un-rotate the pixel centre into the rect's local (axis-aligned) frame.
            let rx = px as f64 + 0.5 - cx;
            let ry = py as f64 + 0.5 - cy;
            let lx = rx * cos + ry * sin;
            let ly = -rx * sin + ry * cos;
            let u = (lx + hw) / dw;
            let v = (ly + hh) / dh;
            if !(0.0..1.0).contains(&u) || !(0.0..1.0).contains(&v) {
                continue;
            }
            // Continuous source position (pixel-centre convention: subtract 0.5).
            let fx = (sx0 + u * sw) * src.width as f64 - 0.5;
            let fy = (sy0 + v * sh) * src.height as f64 - 0.5;
            let [pr, pg, pb, pa] = sample_bilinear(src, fx, fy);
            let a = pa * opacity;
            if a <= 0.0 {
                continue;
            }
            let ci = (py * canvas.width + px) * 4;
            // Un-premultiply to straight colour for the blend function.
            let s_rgb = [pr / pa, pg / pa, pb / pa];
            let d_rgb = [
                canvas.pixels[ci] as f64 / 255.0,
                canvas.pixels[ci + 1] as f64 / 255.0,
                canvas.pixels[ci + 2] as f64 / 255.0,
            ];
            let blended = blend_rgb(blend, s_rgb, d_rgb);
            for k in 0..3 {
                let out = blended[k] * a + d_rgb[k] * (1.0 - a);
                canvas.pixels[ci + k] = (out * 255.0).round().clamp(0.0, 255.0) as u8;
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
        // Keyframe tracks are clip-relative; resolve transform/crop/opacity at
        // this frame so animated clips render correctly.
        let rel = frame - clip.start_frame;
        let t = timeline_core::resolved_transform_at(clip, rel);
        let c = timeline_core::resolved_crop_at(clip, rel);
        let opacity = timeline_core::resolved_opacity_at(clip, rel);
        let dw = t.width * cw;
        let dh = t.height * ch;
        let dx = t.center_x * cw - dw / 2.0;
        let dy = t.center_y * ch - dh / 2.0;

        // Crop → the source sub-rectangle that stays visible.
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
            opacity,
            t.rotation,
            clip.blend_mode,
        );
    }

    canvas
}

/// Compose every frame in `[0, total_frames)` and hand each to
/// `sink(frame_index, &image)`. This is the pure render driver a video encoder
/// plugs into: `fetch_source(clip, frame)` decodes each clip's source frame
/// (platform adapter), and `sink` receives the flattened RGBA frame (the encoder
/// writes it). Timeline math and compositing are pure and unit-tested here.
#[allow(clippy::too_many_arguments)]
pub fn render_sequence(
    timeline: &Timeline,
    manifest: &MediaManifest,
    total_frames: i64,
    width: usize,
    height: usize,
    mut fetch_source: impl FnMut(&Clip, i64) -> Option<RgbaImage>,
    mut sink: impl FnMut(i64, &RgbaImage),
) {
    for frame in 0..total_frames.max(0) {
        let img = compose_frame(timeline, manifest, frame, width, height, |c| {
            fetch_source(c, frame)
        });
        sink(frame, &img);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_model::{Clip, Crop, Interpolation, Keyframe, KeyframeTrack, Track, Transform};

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
        blit_scaled(
            &mut canvas,
            &red,
            (0.0, 0.0, 1.0, 1.0),
            (0.0, 0.0, 4.0, 4.0),
            1.0,
            0.0,
            BlendMode::Normal,
        );
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
        blit_scaled(
            &mut canvas,
            &white,
            (0.0, 0.0, 1.0, 1.0),
            (0.0, 0.0, 2.0, 2.0),
            0.5,
            0.0,
            BlendMode::Normal,
        );
        // 255*0.5 + 0*0.5 = 127.5 → 128.
        assert_eq!(px(&canvas, 0, 0), [128, 128, 128, 255]);
    }

    #[test]
    fn blit_rotation_90_swaps_axes() {
        // A 4-wide, 2-tall dst rect rotated 90° covers a 2-wide, 4-tall area on
        // an 8x8 canvas, centered — its rotated footprint stays on-canvas.
        let mut canvas = RgbaImage::new(8, 8);
        let green = RgbaImage::solid(1, 1, [0, 255, 0, 255]);
        // dst rect (x=2,y=3,w=4,h=2) → center (4,4); rotated 90° spans x∈[3,5), y∈[2,6).
        blit_scaled(
            &mut canvas,
            &green,
            (0.0, 0.0, 1.0, 1.0),
            (2.0, 3.0, 4.0, 2.0),
            1.0,
            90.0,
            BlendMode::Normal,
        );
        // Center is filled; a point on the (now narrow) horizontal axis is empty.
        assert_eq!(px(&canvas, 4, 4), [0, 255, 0, 255], "center filled");
        assert_eq!(px(&canvas, 4, 2), [0, 255, 0, 255], "extends vertically");
        assert_eq!(px(&canvas, 1, 4), [0, 0, 0, 0], "narrow horizontally");
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

    #[test]
    fn blend_channel_separable_modes() {
        use BlendMode::*;
        assert!((blend_channel(Normal, 0.3, 0.7) - 0.3).abs() < 1e-9);
        assert!((blend_channel(Multiply, 0.5, 0.5) - 0.25).abs() < 1e-9);
        assert!((blend_channel(Screen, 0.5, 0.5) - 0.75).abs() < 1e-9);
        assert!((blend_channel(Darken, 0.3, 0.7) - 0.3).abs() < 1e-9);
        assert!((blend_channel(Lighten, 0.3, 0.7) - 0.7).abs() < 1e-9);
        assert!((blend_channel(Difference, 0.7, 0.3) - 0.4).abs() < 1e-9);
        // Non-separable HSL modes fall back to Normal (src) for now.
        assert!((blend_channel(Color, 0.3, 0.7) - 0.3).abs() < 1e-9);
    }

    #[test]
    fn compose_samples_opacity_keyframes() {
        let mut c = clip("c1", "m1", 0, 100);
        c.transform = Transform::from_top_left(0.0, 0.0, 1.0, 1.0); // full frame
        c.opacity_track = Some(KeyframeTrack {
            keyframes: vec![
                Keyframe {
                    frame: 0,
                    value: 0.0,
                    interpolation_out: Interpolation::Linear,
                },
                Keyframe {
                    frame: 100,
                    value: 1.0,
                    interpolation_out: Interpolation::Linear,
                },
            ],
        });
        let timeline = tl(vec![c]);
        // At frame 0 the keyframed opacity is 0 → nothing drawn.
        let f0 = compose_frame(&timeline, &MediaManifest::default(), 0, 2, 2, |_| {
            Some(RgbaImage::solid(2, 2, [255, 255, 255, 255]))
        });
        assert_eq!(px(&f0, 0, 0), [0, 0, 0, 0]);
        // At frame 50 the opacity is ~0.5 → half-alpha result.
        let f50 = compose_frame(&timeline, &MediaManifest::default(), 50, 2, 2, |_| {
            Some(RgbaImage::solid(2, 2, [255, 255, 255, 255]))
        });
        let p = px(&f50, 0, 0);
        assert!(p[3] > 100 && p[3] < 160, "alpha ~0.5, got {}", p[3]);
    }

    #[test]
    fn compose_applies_blend_mode() {
        let bg = clip("bg", "m1", 0, 30);
        let mut top = clip("top", "m2", 0, 30);
        top.blend_mode = BlendMode::Multiply;
        let timeline = tl(vec![bg, top]);
        let out = compose_frame(&timeline, &MediaManifest::default(), 0, 2, 2, |c| {
            if c.id == "bg" {
                Some(RgbaImage::solid(2, 2, [255, 255, 255, 255])) // white
            } else {
                Some(RgbaImage::solid(2, 2, [128, 128, 128, 255])) // gray, multiply
            }
        });
        // Multiply gray over white ≈ gray.
        assert_eq!(px(&out, 0, 0), [128, 128, 128, 255]);
    }

    #[test]
    fn render_sequence_composes_every_frame() {
        let mut c = clip("c1", "m1", 0, 4);
        c.transform = Transform::from_top_left(0.0, 0.0, 1.0, 1.0);
        let timeline = tl(vec![c]);
        let mut frames: Vec<(i64, [u8; 4])> = Vec::new();
        let mut decoded = 0;
        render_sequence(
            &timeline,
            &MediaManifest::default(),
            4,
            2,
            2,
            |_clip, frame| {
                decoded += 1;
                // Encode the frame index into the red channel to prove per-frame decode.
                Some(RgbaImage::solid(2, 2, [frame as u8 * 10, 0, 0, 255]))
            },
            |frame, img| frames.push((frame, px(img, 0, 0))),
        );
        assert_eq!(frames.len(), 4);
        assert_eq!(decoded, 4);
        assert_eq!(frames[0], (0, [0, 0, 0, 255]));
        assert_eq!(frames[3], (3, [30, 0, 0, 255]));
    }

    #[test]
    fn sample_bilinear_interpolates_between_pixels() {
        // 2x1 source: black then white, both opaque.
        let mut src = RgbaImage::new(2, 1);
        src.pixels[0..4].copy_from_slice(&[0, 0, 0, 255]);
        src.pixels[4..8].copy_from_slice(&[255, 255, 255, 255]);
        // Midway between the two pixel centres → mid-grey, fully opaque.
        let mid = sample_bilinear(&src, 0.5, 0.0);
        assert!((mid[0] - 0.5).abs() < 1e-6, "r {}", mid[0]);
        assert!((mid[3] - 1.0).abs() < 1e-6, "a {}", mid[3]);
        // At the exact pixel centres, no blending.
        assert_eq!(sample_bilinear(&src, 0.0, 0.0), [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(sample_bilinear(&src, 1.0, 0.0), [1.0, 1.0, 1.0, 1.0]);
    }

    #[test]
    fn blend_rgb_separable_matches_per_channel() {
        // Multiply of mid-grey over mid-grey → quarter-grey, per channel.
        let out = blend_rgb(BlendMode::Multiply, [0.5, 0.5, 0.5], [0.5, 0.5, 0.5]);
        for v in out {
            assert!((v - 0.25).abs() < 1e-9, "got {v}");
        }
    }

    #[test]
    fn blend_rgb_color_takes_dst_luminosity() {
        // Color: source hue/sat, destination luminosity.
        let dst = [0.5, 0.5, 0.5];
        let out = blend_rgb(BlendMode::Color, [0.0, 0.0, 1.0], dst);
        assert!((lum(out) - lum(dst)).abs() < 1e-6, "lum {}", lum(out));
    }

    #[test]
    fn blend_rgb_luminosity_takes_src_luminosity() {
        // Luminosity: destination hue/sat, source luminosity.
        let src = [0.5, 0.5, 0.5];
        let out = blend_rgb(BlendMode::Luminosity, src, [1.0, 0.0, 0.0]);
        assert!((lum(out) - lum(src)).abs() < 1e-6, "lum {}", lum(out));
        // Destination hue is preserved: red channel stays the largest.
        assert!(out[0] >= out[1] && out[0] >= out[2]);
    }

    #[test]
    fn blend_rgb_hue_and_saturation_keep_dst_luminosity() {
        let dst = [0.2, 0.6, 0.9];
        let hue = blend_rgb(BlendMode::Hue, [0.9, 0.1, 0.1], dst);
        let sat = blend_rgb(BlendMode::Saturation, [0.9, 0.1, 0.1], dst);
        assert!((lum(hue) - lum(dst)).abs() < 1e-6);
        assert!((lum(sat) - lum(dst)).abs() < 1e-6);
    }

    #[test]
    fn render_sequence_zero_frames_yields_nothing() {
        let timeline = tl(vec![clip("c1", "m1", 0, 4)]);
        let mut count = 0;
        render_sequence(
            &timeline,
            &MediaManifest::default(),
            0,
            2,
            2,
            |_, _| Some(RgbaImage::solid(2, 2, [255, 255, 255, 255])),
            |_, _| count += 1,
        );
        assert_eq!(count, 0);
    }
}

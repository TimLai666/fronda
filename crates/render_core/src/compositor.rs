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

use core_model::shape_style::{Rgba, ShapeKind, ShapeStyle};
use core_model::{BlendMode, ChromaKey, Clip, ClipType, MediaManifest, Timeline};

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
            // W3C order: backdrop-zero check first (black backdrop stays black).
            if d <= 0.0 {
                0.0
            } else if s >= 1.0 {
                1.0
            } else {
                (d / (1.0 - s)).min(1.0)
            }
        }
        ColorBurn => {
            // W3C order: backdrop-one check first (white backdrop stays white).
            if d >= 1.0 {
                1.0
            } else if s <= 0.0 {
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
            let da = canvas.pixels[ci + 3] as f64 / 255.0;
            let blended = blend_rgb(blend, s_rgb, d_rgb);
            // Straight-alpha source-over. out_a = src_a + dst_a*(1-src_a); the RGB is
            // the premultiplied result divided by out_a. The backdrop term must be
            // weighted by its own alpha `da` and the result normalized by out_a —
            // both are no-ops only when the backdrop is opaque (da==1), which is why
            // the previous (da-omitting, un-normalized) form looked right in tests
            // but stored premultiplied colour over the transparent canvas.
            let out_a = a + da * (1.0 - a);
            let inv_out_a = if out_a > 0.0 { 1.0 / out_a } else { 0.0 };
            for k in 0..3 {
                // The blend only takes effect in proportion to the backdrop's alpha
                // (W3C); over transparent areas the source shows unblended.
                let effective = (1.0 - da) * s_rgb[k] + da * blended[k];
                let out = (effective * a + d_rgb[k] * da * (1.0 - a)) * inv_out_a;
                canvas.pixels[ci + k] = (out * 255.0).round().clamp(0.0, 255.0) as u8;
            }
            canvas.pixels[ci + 3] = (out_a * 255.0).round().clamp(0.0, 255.0) as u8;
        }
    }
}

/// Separable box blur with the given pixel `radius` (a fast gaussian approx).
/// Averages each channel — including alpha — over the `2*radius+1` window,
/// clamping at edges. No-op for radius 0. PR #8 blur path.
///
/// Uses per-line prefix sums so each pass is O(w*h) regardless of `radius`
/// (the naive window sum was O(w*h*radius) — costly for the default blur-6 text
/// shadow at 1080p across every export frame). Output is bit-identical to the
/// naive average: the window sum is an exact integer and the divide/round match.
pub fn apply_blur(img: &mut RgbaImage, radius: usize) {
    if radius == 0 || img.width == 0 || img.height == 0 {
        return;
    }
    let (w, h) = (img.width, img.height);
    let mut prefix = vec![0u64; w.max(h) + 1];
    // Horizontal pass into a temp buffer.
    let mut tmp = vec![0u8; img.pixels.len()];
    for y in 0..h {
        let row = y * w;
        for c in 0..4 {
            prefix[0] = 0;
            for x in 0..w {
                prefix[x + 1] = prefix[x] + img.pixels[(row + x) * 4 + c] as u64;
            }
            for x in 0..w {
                let lo = x.saturating_sub(radius);
                let hi = (x + radius).min(w - 1);
                let sum = prefix[hi + 1] - prefix[lo];
                let count = (hi - lo + 1) as f64;
                tmp[(row + x) * 4 + c] = (sum as f64 / count).round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    // Vertical pass back into the image.
    for x in 0..w {
        for c in 0..4 {
            prefix[0] = 0;
            for y in 0..h {
                prefix[y + 1] = prefix[y] + tmp[(y * w + x) * 4 + c] as u64;
            }
            for y in 0..h {
                let lo = y.saturating_sub(radius);
                let hi = (y + radius).min(h - 1);
                let sum = prefix[hi + 1] - prefix[lo];
                let count = (hi - lo + 1) as f64;
                img.pixels[(y * w + x) * 4 + c] = (sum as f64 / count).round().clamp(0.0, 255.0) as u8;
            }
        }
    }
}

/// Darken pixels toward the corners (radial vignette). `amount` is `0..=1`: 0 is
/// a no-op, 1 fully darkens the corners. Falloff is quadratic in the normalized
/// distance from centre. Alpha is untouched. PR #8 vignette path.
pub fn apply_vignette(img: &mut RgbaImage, amount: f64) {
    let amount = amount.clamp(0.0, 1.0);
    if amount == 0.0 || img.width == 0 || img.height == 0 {
        return;
    }
    let (w, h) = (img.width as f64, img.height as f64);
    let (cx, cy) = (w / 2.0, h / 2.0);
    let max_d2 = cx * cx + cy * cy;
    for y in 0..img.height {
        for x in 0..img.width {
            let dx = x as f64 + 0.5 - cx;
            let dy = y as f64 + 0.5 - cy;
            let d2 = (dx * dx + dy * dy) / max_d2; // 0..1
            let factor = 1.0 - amount * d2;
            let i = (y * img.width + x) * 4;
            for c in 0..3 {
                img.pixels[i + c] =
                    (img.pixels[i + c] as f64 * factor).round().clamp(0.0, 255.0) as u8;
            }
        }
    }
}

/// Rotate `src` by `degrees` (clockwise) about the pixel point `(px, py)`,
/// keeping the same dimensions (content outside the frame is clipped). Bilinear
/// inverse sampling; a no-op for 0°. Used for text-layer rotation.
pub fn rotate_around(src: &RgbaImage, degrees: f64, px: f64, py: f64) -> RgbaImage {
    if degrees == 0.0 || src.width == 0 || src.height == 0 {
        return src.clone();
    }
    let rad = degrees * std::f64::consts::PI / 180.0;
    let (sin, cos) = (rad.sin(), rad.cos());
    let mut out = RgbaImage::new(src.width, src.height);
    for y in 0..src.height {
        for x in 0..src.width {
            let dx = x as f64 + 0.5 - px;
            let dy = y as f64 + 0.5 - py;
            // Inverse rotation to find the source position.
            let sx = px + dx * cos + dy * sin - 0.5;
            let sy = py - dx * sin + dy * cos - 0.5;
            if sx < -0.5 || sx > src.width as f64 - 0.5 || sy < -0.5 || sy > src.height as f64 - 0.5
            {
                continue;
            }
            let [r, g, b, a] = sample_bilinear(src, sx, sy);
            let i = (y * out.width + x) * 4;
            if a > 0.0 {
                out.pixels[i] = (r / a * 255.0).round().clamp(0.0, 255.0) as u8;
                out.pixels[i + 1] = (g / a * 255.0).round().clamp(0.0, 255.0) as u8;
                out.pixels[i + 2] = (b / a * 255.0).round().clamp(0.0, 255.0) as u8;
                out.pixels[i + 3] = (a * 255.0).round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    out
}

/// Mirror an image horizontally and/or vertically (clip flip). Returns a clone
/// unchanged when neither flip is set.
pub fn flip_image(img: &RgbaImage, horizontal: bool, vertical: bool) -> RgbaImage {
    if !horizontal && !vertical {
        return img.clone();
    }
    let mut out = RgbaImage::new(img.width, img.height);
    for y in 0..img.height {
        for x in 0..img.width {
            let sx = if horizontal { img.width - 1 - x } else { x };
            let sy = if vertical { img.height - 1 - y } else { y };
            let si = (sy * img.width + sx) * 4;
            let di = (y * img.width + x) * 4;
            out.pixels[di..di + 4].copy_from_slice(&img.pixels[si..si + 4]);
        }
    }
    out
}

/// Apply the compositor-supported colour adjustments (exposure, contrast,
/// saturation, brightness) from a clip's resolved effect states, in that order.
/// Each is a single-parameter effect; unrecognised or grading effects are left
/// for a dedicated pass. PR #8 colour-adjustment path.
pub fn apply_color_adjustments(img: &mut RgbaImage, effects: &[crate::effects::EffectState]) {
    // The effect stack is ORDERED (`effects: Vec<Effect>`), so apply each recognized
    // adjustment per pixel IN LIST ORDER, accumulating in f64 with a single final
    // clamp — no mid-pipeline u8 round-trip (which used to throw away out-of-[0,1]
    // intermediates, e.g. an exposure-blown highlight flattened by a later brightness
    // cut). Order and multiplicity are preserved: reordering the stack changes the
    // result, and stacking two of the same adjustment compounds.
    enum Op {
        Exposure(f64),
        Contrast(f64),
        Saturation(f64),
        Brightness(f64),
    }
    let ops: Vec<Op> = effects
        .iter()
        .filter(|e| e.enabled)
        .filter_map(|e| {
            let amount = e.params.values().next().copied();
            match e.effect_type.as_str() {
                "color.exposure" => Some(Op::Exposure(2f64.powf(amount.unwrap_or(0.0)))),
                "color.contrast" => Some(Op::Contrast(amount.unwrap_or(1.0))),
                "color.saturation" => Some(Op::Saturation(amount.unwrap_or(1.0))),
                "color.brightness" => Some(Op::Brightness(amount.unwrap_or(0.0))),
                _ => None,
            }
        })
        .collect();
    if ops.is_empty() {
        return;
    }
    for px in img.pixels.chunks_exact_mut(4) {
        let mut rgb = [
            px[0] as f64 / 255.0,
            px[1] as f64 / 255.0,
            px[2] as f64 / 255.0,
        ];
        for op in &ops {
            match *op {
                Op::Exposure(m) => {
                    for c in &mut rgb {
                        *c *= m;
                    }
                }
                Op::Contrast(k) => {
                    for c in &mut rgb {
                        *c = (*c - 0.5) * k + 0.5;
                    }
                }
                Op::Saturation(s) => {
                    let luma = 0.299 * rgb[0] + 0.587 * rgb[1] + 0.114 * rgb[2];
                    for c in &mut rgb {
                        *c = luma + (*c - luma) * s;
                    }
                }
                Op::Brightness(b) => {
                    for c in &mut rgb {
                        *c += b;
                    }
                }
            }
        }
        for (i, c) in rgb.iter().enumerate() {
            px[i] = (c * 255.0).round().clamp(0.0, 255.0) as u8;
        }
    }
}

/// Make pixels within `tolerance` of the key colour transparent (green-screen
/// keying), then suppress key-colour spill on the pixels that survive. Distances
/// are Euclidean in normalized RGB; `tolerance` and `spill_suppression` are
/// `0..=1`. No-op when the key is disabled. PR #8 chroma-key path.
pub fn apply_chroma_key(img: &mut RgbaImage, key: &ChromaKey) {
    if !key.enabled {
        return;
    }
    let (kr, kg, kb) = (key.key_r, key.key_g, key.key_b);
    // Distance is in 0..√3; scale the 0..1 tolerance to that range.
    let cutoff = key.tolerance.clamp(0.0, 1.0) * 3.0f64.sqrt();
    let spill = key.spill_suppression.clamp(0.0, 1.0);
    for px in img.pixels.chunks_exact_mut(4) {
        let r = px[0] as f64 / 255.0;
        let g = px[1] as f64 / 255.0;
        let b = px[2] as f64 / 255.0;
        let d = ((r - kr).powi(2) + (g - kg).powi(2) + (b - kb).powi(2)).sqrt();
        if d <= cutoff {
            px[3] = 0;
        } else if spill > 0.0 {
            // Spill suppression: where the key's dominant channel exceeds the
            // average of the other two (key-colour bleed), pull it down toward
            // that average by `spill`. Independent of the keying distance.
            let (ci, cv) = dominant_channel(kr, kg, kb);
            if cv > 0.5 {
                let chans = [r, g, b];
                let others = (chans[0] + chans[1] + chans[2] - chans[ci]) / 2.0;
                if chans[ci] > others {
                    let reduced = chans[ci] + (others - chans[ci]) * spill;
                    px[ci] = (reduced * 255.0).round().clamp(0.0, 255.0) as u8;
                }
            }
        }
    }
}

/// Index (0=r,1=g,2=b) and value of the key's largest channel.
fn dominant_channel(kr: f64, kg: f64, kb: f64) -> (usize, f64) {
    if kg >= kr && kg >= kb {
        (1, kg)
    } else if kr >= kb {
        (0, kr)
    } else {
        (2, kb)
    }
}

fn rgba_to_u8(c: &Rgba) -> [u8; 4] {
    [
        (c.r * 255.0).round().clamp(0.0, 255.0) as u8,
        (c.g * 255.0).round().clamp(0.0, 255.0) as u8,
        (c.b * 255.0).round().clamp(0.0, 255.0) as u8,
        (c.a * 255.0).round().clamp(0.0, 255.0) as u8,
    ]
}

/// Rasterize a shape annotation into a `w`×`h` RGBA image (transparent outside
/// the shape). Rect and Oval/Circle render fill + a border stroke; Arrow and
/// Line are not yet composited (returned transparent). PR #46.
pub fn rasterize_shape(shape: &ShapeStyle, w: usize, h: usize) -> RgbaImage {
    let mut img = RgbaImage::new(w, h);
    if w == 0 || h == 0 {
        return img;
    }
    let is_ellipse = matches!(shape.kind, ShapeKind::Oval | ShapeKind::Circle);
    if !is_ellipse && shape.kind != ShapeKind::Rect {
        return img;
    }
    let fill = rgba_to_u8(&shape.fill.color);
    let stroke = rgba_to_u8(&shape.stroke.color);
    let sw = shape.stroke.width.max(0.0);
    let band_x = sw / w as f64;
    let band_y = sw / h as f64;
    for y in 0..h {
        for x in 0..w {
            let nx = (x as f64 + 0.5) / w as f64;
            let ny = (y as f64 + 0.5) / h as f64;
            let (inside, on_stroke) = if is_ellipse {
                let (cx, cy) = (w as f64 / 2.0, h as f64 / 2.0);
                // A Circle is a TRUE circle (min half-extent on both axes); an Oval
                // fills the box.
                let (rx, ry) = if shape.kind == ShapeKind::Circle {
                    let r = cx.min(cy);
                    (r, r)
                } else {
                    (cx, cy)
                };
                let dx = x as f64 + 0.5 - cx;
                let dy = y as f64 + 0.5 - cy;
                let outer = (dx / rx).powi(2) + (dy / ry).powi(2) <= 1.0;
                // Uniform sw-pixel stroke: the inner ellipse is shrunk by sw pixels
                // per axis (was a single normalized band → non-uniform on non-square).
                let (irx, iry) = ((rx - sw).max(0.0), (ry - sw).max(0.0));
                let inner = irx > 0.0
                    && iry > 0.0
                    && (dx / irx).powi(2) + (dy / iry).powi(2) <= 1.0;
                (outer, outer && !inner)
            } else {
                let on = nx < band_x || nx > 1.0 - band_x || ny < band_y || ny > 1.0 - band_y;
                (true, on)
            };
            if !inside {
                continue;
            }
            let color = if sw > 0.0 && on_stroke {
                stroke
            } else if shape.fill.enabled {
                fill
            } else {
                continue;
            };
            let i = (y * w + x) * 4;
            img.pixels[i..i + 4].copy_from_slice(&color);
        }
    }
    img
}

/// Visual clips on `timeline` that are on screen at `frame`, in render order
/// (bottom layer first). `tracks[0]` is the TOP visual layer (matching the model
/// convention the XMEML export reverses), so tracks are walked back-to-front.
/// Text overlays are still skipped (need a text rasterizer); shape annotations
/// are included. Media clips need a non-empty `media_ref`.
fn visible_clips(timeline: &Timeline, frame: i64) -> Vec<&Clip> {
    let mut out: Vec<&Clip> = Vec::new();
    // Last track = bottom layer (blitted first); tracks[0] = top (blitted last).
    for track in timeline.tracks.iter().rev() {
        if track.r#type == ClipType::Audio || track.hidden {
            continue;
        }
        for clip in &track.clips {
            let is_shape = clip.media_type == ClipType::Shape && clip.shape_style.is_some();
            let is_text = clip.media_type == ClipType::Text
                && clip.text_style.is_some()
                && clip
                    .text_content
                    .as_ref()
                    .is_some_and(|t| !t.trim().is_empty());
            if clip.media_type == ClipType::Text && !is_text {
                continue;
            }
            if !is_shape && !is_text && clip.media_ref.is_empty() {
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
    // Expand compound clips into their constituents so they render their nested
    // content (Issue #155). Zero-cost when the project has no compound clips.
    let flattened;
    let timeline: &Timeline = if timeline.compound_timelines.is_empty() {
        timeline
    } else {
        flattened = timeline_core::flatten_compound_clips(timeline);
        &flattened
    };

    let mut canvas = RgbaImage::new(width, height);
    let (cw, ch) = (width as f64, height as f64);

    for clip in visible_clips(timeline, frame) {
        // Keyframe tracks are clip-relative; resolve transform/crop/opacity at
        // this frame so animated clips render correctly.
        let rel = frame - clip.start_frame;
        let t = timeline_core::resolved_transform_at(clip, rel);
        let opacity = timeline_core::resolved_opacity_at(clip, rel)
            * timeline_core::fade_multiplier_at(clip, rel);
        let dw = t.width * cw;
        let dh = t.height * ch;
        let dx = t.center_x * cw - dw / 2.0;
        let dy = t.center_y * ch - dh / 2.0;

        // Shape annotations are rasterized procedurally; text is rasterized into a
        // full-canvas layer (already positioned); media clips are fetched/cropped.
        // `dst` is the blit target rect; `rotation` the blit rotation.
        let mut dst = (dx, dy, dw, dh);
        let mut rotation = t.rotation;
        let (src, src_region) = if clip.media_type == ClipType::Text {
            let Some(ts) = clip.text_style.as_ref() else {
                continue;
            };
            let text = clip.text_content.as_deref().unwrap_or("");
            let mut img = crate::text::render_text(text, ts, width, height, t.center_x, t.center_y);
            // The text layer is positioned on the full canvas; rotate it about the
            // clip's centre (blit stays identity/unrotated).
            if t.rotation != 0.0 {
                img = rotate_around(&img, t.rotation, t.center_x * cw, t.center_y * ch);
            }
            dst = (0.0, 0.0, cw, ch);
            rotation = 0.0;
            (img, (0.0, 0.0, 1.0, 1.0))
        } else if clip.media_type == ClipType::Shape {
            let Some(shape) = clip.shape_style.as_ref() else {
                continue;
            };
            let sw = (dw.round() as usize).clamp(1, 4096);
            let sh = (dh.round() as usize).clamp(1, 4096);
            (rasterize_shape(shape, sw, sh), (0.0, 0.0, 1.0, 1.0))
        } else {
            let Some(mut src) = fetch_source(clip) else {
                continue;
            };
            if t.flip_horizontal || t.flip_vertical {
                src = flip_image(&src, t.flip_horizontal, t.flip_vertical);
            }
            if let Some(key) = clip.chroma_key.as_ref() {
                apply_chroma_key(&mut src, key);
            }
            let fx = crate::effects::analyze_clip_effects(clip, rel);
            if fx.has_color_adjustments {
                apply_color_adjustments(&mut src, &fx.effects);
            }
            if fx.has_blur_or_vignette {
                for e in &fx.effects {
                    if !e.enabled {
                        continue;
                    }
                    let amount = e.params.values().next().copied().unwrap_or(0.0);
                    match e.effect_type.as_str() {
                        "blur.gaussian" | "blur" => {
                            apply_blur(&mut src, amount.max(0.0).round() as usize)
                        }
                        "vignette" => apply_vignette(&mut src, amount),
                        _ => {}
                    }
                }
            }
            let c = timeline_core::resolved_crop_at(clip, rel);
            let region = (
                c.left,
                c.top,
                (1.0 - c.left - c.right).max(0.0),
                (1.0 - c.top - c.bottom).max(0.0),
            );
            (src, region)
        };

        blit_scaled(
            &mut canvas,
            &src,
            src_region,
            dst,
            opacity,
            rotation,
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
    use core_model::{
        AnimPair, Clip, Crop, Interpolation, Keyframe, KeyframeTrack, Track, Transform,
    };

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
            text_animation: None,
            word_timings: None,
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
    fn blit_over_transparent_stores_straight_alpha_not_premultiplied() {
        // A 50% clip over the transparent canvas must store STRAIGHT colour
        // (white stays 255 with alpha 128), not premultiplied grey [128,128,128,128].
        let mut canvas = RgbaImage::new(1, 1); // transparent, straight-alpha buffer
        let white_half = RgbaImage::solid(1, 1, [255, 255, 255, 128]);
        let region = (0.0, 0.0, 1.0, 1.0);
        blit_scaled(&mut canvas, &white_half, region, (0.0, 0.0, 1.0, 1.0), 1.0, 0.0, BlendMode::Normal);
        let p = px(&canvas, 0, 0);
        assert_eq!(p, [255, 255, 255, 128], "straight colour over transparent");
        // Stacking a second identical layer stays pure white (never greys out).
        blit_scaled(&mut canvas, &white_half, region, (0.0, 0.0, 1.0, 1.0), 1.0, 0.0, BlendMode::Normal);
        let p2 = px(&canvas, 0, 0);
        assert_eq!((p2[0], p2[1], p2[2]), (255, 255, 255), "white-over-white stays white: {p2:?}");
        assert!(p2[3] > 180 && p2[3] < 200, "alpha ~0.75: {}", p2[3]);
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
    fn compose_renders_compound_clip_nested_content() {
        // A compound clip wrapping one full-frame clip must render the nested
        // clip, not an empty frame (Issue #155 flatten inside compose_frame).
        let inner = clip("inner", "vid", 0, 30);
        let nested = tl(vec![inner]);
        let mut compound = clip("compound", "n1", 0, 30);
        compound.compound_timeline_id = Some("n1".into());
        let mut timeline = tl(vec![compound]);
        timeline
            .compound_timelines
            .insert("n1".into(), Box::new(nested));

        let out = compose_frame(&timeline, &MediaManifest::default(), 5, 4, 4, |c| {
            // The compound clip's own ref ("n1") must never be fetched — only the
            // flattened constituent's ("vid").
            if c.media_ref == "vid" {
                Some(RgbaImage::solid(4, 4, [0, 0, 255, 255]))
            } else {
                None
            }
        });
        assert_eq!(px(&out, 2, 2), [0, 0, 255, 255], "nested clip rendered");
    }

    #[test]
    fn compose_first_track_is_the_top_layer() {
        // tracks[0] is the top visual layer; a full-frame clip there must cover a
        // full-frame clip on tracks[1].
        let top_clip = clip("top", "m1", 0, 30);
        let bottom_clip = clip("bot", "m2", 0, 30);
        let mk_track = |id: &str, clips: Vec<Clip>| Track {
            id: id.into(),
            r#type: ClipType::Video,
            muted: false,
            hidden: false,
            sync_locked: false,
            clips,
        };
        let timeline = Timeline {
            fps: 30,
            width: 4,
            height: 4,
            tracks: vec![mk_track("v0", vec![top_clip]), mk_track("v1", vec![bottom_clip])],
            settings_configured: true,
            selected_clip_ids: std::collections::HashSet::new(),
            transcription_language: None,
            compound_timelines: std::collections::HashMap::new(),
        };
        let out = compose_frame(&timeline, &MediaManifest::default(), 0, 4, 4, |c| {
            if c.id == "top" {
                Some(RgbaImage::solid(4, 4, [0, 0, 255, 255])) // blue on tracks[0]
            } else {
                Some(RgbaImage::solid(4, 4, [255, 0, 0, 255])) // red on tracks[1]
            }
        });
        assert_eq!(px(&out, 2, 2), [0, 0, 255, 255], "tracks[0] blue is on top");
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
    fn color_dodge_burn_w3c_degenerate_corners() {
        use BlendMode::*;
        // W3C orders the backdrop check first: a black backdrop stays black under
        // ColorDodge even with a full-white source (source-first order gave 1.0).
        assert_eq!(blend_channel(ColorDodge, 1.0, 0.0), 0.0);
        assert!((blend_channel(ColorDodge, 0.5, 0.4) - (0.4f64 / 0.5).min(1.0)).abs() < 1e-9);
        // A white backdrop stays white under ColorBurn even with a black source.
        assert_eq!(blend_channel(ColorBurn, 0.0, 1.0), 1.0);
        assert!(
            (blend_channel(ColorBurn, 0.5, 0.6) - (1.0 - ((1.0 - 0.6) / 0.5f64).min(1.0))).abs()
                < 1e-9
        );
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
    fn compose_position_keyframe_places_clip_by_top_left() {
        // End-to-end: a half-canvas clip whose position track holds TOP-LEFT at the
        // origin must render at the top-left corner (resolved centre 0.25,0.25 →
        // dst (0,0,2,2) on a 4x4 canvas), not offset by half its size.
        let mut c = clip("p", "m1", 0, 30);
        c.transform.width = 0.5;
        c.transform.height = 0.5;
        c.position_track = Some(KeyframeTrack {
            keyframes: vec![Keyframe {
                frame: 0,
                value: AnimPair { a: 0.0, b: 0.0 },
                interpolation_out: Interpolation::Hold,
            }],
        });
        let timeline = tl(vec![c]);
        let out = compose_frame(&timeline, &MediaManifest::default(), 0, 4, 4, |_| {
            Some(RgbaImage::solid(2, 2, [255, 0, 0, 255]))
        });
        // Clip covers pixels (0,0)-(1,1). The discriminating pixel is (1,1): under
        // the old centre-passthrough bug the clip sat at (-1,-1,2,2) and (1,1) was
        // empty.
        assert_eq!(px(&out, 0, 0), [255, 0, 0, 255], "top-left pixel red");
        assert_eq!(px(&out, 1, 1), [255, 0, 0, 255], "clip reaches (1,1)");
        assert_eq!(px(&out, 3, 3), [0, 0, 0, 0], "clip does not reach bottom-right");
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
    fn blend_mode_over_transparent_shows_source() {
        // A Multiply clip alone on a transparent canvas must show the clip, not
        // multiply-against-black (which would be all black).
        let mut top = clip("only", "m1", 0, 30);
        top.blend_mode = BlendMode::Multiply;
        top.transform = Transform::from_top_left(0.0, 0.0, 1.0, 1.0);
        let timeline = tl(vec![top]);
        let out = compose_frame(&timeline, &MediaManifest::default(), 0, 2, 2, |_| {
            Some(RgbaImage::solid(2, 2, [200, 100, 50, 255]))
        });
        assert_eq!(px(&out, 0, 0), [200, 100, 50, 255], "source shows over transparent");
    }

    fn solid_shape(kind: core_model::shape_style::ShapeKind, rgba: [f64; 4]) -> ShapeStyle {
        use core_model::shape_style::{Fill, Rgba, Stroke};
        ShapeStyle {
            kind,
            stroke: Stroke {
                color: Rgba::default(),
                width: 0.0, // no border → pure fill for the test
                dashed: false,
                arrowhead_style: None,
            },
            fill: Fill {
                enabled: true,
                color: Rgba {
                    r: rgba[0],
                    g: rgba[1],
                    b: rgba[2],
                    a: rgba[3],
                },
            },
            arrowhead: None,
            endpoints: None,
        }
    }

    #[test]
    fn rasterize_rect_is_fully_filled() {
        use core_model::shape_style::ShapeKind;
        let img = rasterize_shape(&solid_shape(ShapeKind::Rect, [1.0, 0.0, 0.0, 1.0]), 4, 4);
        for y in 0..4 {
            for x in 0..4 {
                assert_eq!(px(&img, x, y), [255, 0, 0, 255], "({x},{y})");
            }
        }
    }

    #[test]
    fn rasterize_oval_fills_center_not_corners() {
        use core_model::shape_style::ShapeKind;
        let img = rasterize_shape(&solid_shape(ShapeKind::Oval, [0.0, 0.0, 1.0, 1.0]), 8, 8);
        assert_eq!(px(&img, 4, 4), [0, 0, 255, 255], "center filled");
        assert_eq!(px(&img, 0, 0), [0, 0, 0, 0], "corner transparent");
    }

    #[test]
    fn rasterize_circle_is_a_true_circle_not_stretched() {
        use core_model::shape_style::ShapeKind;
        // In a wide 16×4 box a Circle has radius min(8,2)=2 centred at (8,2); the
        // far-left mid-row pixel is well outside it — an Oval fills the box width.
        let circle = rasterize_shape(&solid_shape(ShapeKind::Circle, [1.0, 0.0, 0.0, 1.0]), 16, 4);
        assert_eq!(px(&circle, 1, 2), [0, 0, 0, 0], "circle does not reach the box edge");
        assert_eq!(px(&circle, 8, 2), [255, 0, 0, 255], "circle centre filled");
        let oval = rasterize_shape(&solid_shape(ShapeKind::Oval, [1.0, 0.0, 0.0, 1.0]), 16, 4);
        assert_eq!(px(&oval, 1, 2)[3], 255, "oval fills the box width at mid-row");
    }

    #[test]
    fn ellipse_stroke_has_uniform_pixel_thickness() {
        use core_model::shape_style::{Fill, Rgba, ShapeKind, Stroke};
        // A wide 24×8 oval with a 2px stroke and no fill. Scanning the mid-row, the
        // two ring crossings should stay thin (~2px each) — the old max-band inner
        // ellipse ballooned the horizontal stroke to ~6px on non-square boxes.
        let shape = ShapeStyle {
            kind: ShapeKind::Oval,
            stroke: Stroke {
                color: Rgba {
                    r: 0.0,
                    g: 1.0,
                    b: 0.0,
                    a: 1.0,
                },
                width: 2.0,
                dashed: false,
                arrowhead_style: None,
            },
            fill: Fill {
                enabled: false,
                color: Rgba::default(),
            },
            arrowhead: None,
            endpoints: None,
        };
        let img = rasterize_shape(&shape, 24, 8);
        let stroke_run = (0..24).filter(|&x| px(&img, x, 4)[3] > 0).count();
        assert!(stroke_run >= 2, "mid-row must have some stroke, got {stroke_run}px");
        assert!(stroke_run <= 6, "mid-row stroke stays thin, got {stroke_run}px");
    }

    #[test]
    fn rotate_around_center_90_moves_a_point() {
        // A single white pixel to the right of centre rotates to below centre
        // under a 90° clockwise rotation about the centre.
        let mut img = RgbaImage::new(5, 5);
        let ci = (2 * 5 + 4) * 4; // (x=4, y=2) — right of centre (2,2)
        img.pixels[ci..ci + 4].copy_from_slice(&[255, 255, 255, 255]);
        let r = rotate_around(&img, 90.0, 2.5, 2.5);
        // Clockwise 90°: the right-of-centre pixel lands below centre (x≈2, y≈4).
        assert!(px(&r, 2, 4)[3] > 100, "pixel rotated to below centre");
        assert_eq!(px(&r, 4, 2)[3], 0, "original position now empty");
    }

    #[test]
    fn rotate_around_zero_is_unchanged() {
        let img = RgbaImage::solid(3, 3, [10, 20, 30, 255]);
        assert_eq!(rotate_around(&img, 0.0, 1.5, 1.5).pixels, img.pixels);
    }

    #[test]
    fn flip_horizontal_mirrors_columns() {
        let mut img = RgbaImage::new(2, 1);
        img.pixels[0..4].copy_from_slice(&[255, 0, 0, 255]); // red left
        img.pixels[4..8].copy_from_slice(&[0, 255, 0, 255]); // green right
        let f = flip_image(&img, true, false);
        assert_eq!(px(&f, 0, 0), [0, 255, 0, 255], "green now on the left");
        assert_eq!(px(&f, 1, 0), [255, 0, 0, 255], "red now on the right");
    }

    #[test]
    fn flip_none_is_unchanged() {
        let img = RgbaImage::solid(2, 2, [10, 20, 30, 255]);
        assert_eq!(flip_image(&img, false, false).pixels, img.pixels);
    }

    #[test]
    fn chroma_key_makes_key_colour_transparent() {
        let mut img = RgbaImage::new(2, 1);
        img.pixels[0..4].copy_from_slice(&[0, 255, 0, 255]); // green (keyed)
        img.pixels[4..8].copy_from_slice(&[255, 0, 0, 255]); // red (kept)
        let key = core_model::ChromaKey {
            enabled: true,
            key_r: 0.0,
            key_g: 1.0,
            key_b: 0.0,
            tolerance: 0.2,
            spill_suppression: 0.0,
        };
        apply_chroma_key(&mut img, &key);
        assert_eq!(px(&img, 0, 0)[3], 0, "green keyed out");
        assert_eq!(px(&img, 1, 0), [255, 0, 0, 255], "red untouched");
    }

    fn color_effect(kind: &str, value: f64) -> crate::effects::EffectState {
        let mut params = std::collections::HashMap::new();
        params.insert("amount".to_string(), value);
        crate::effects::EffectState {
            effect_type: kind.to_string(),
            enabled: true,
            params,
            grade_curve: None,
        }
    }

    #[test]
    fn blur_uniform_image_is_unchanged() {
        let mut img = RgbaImage::solid(5, 5, [100, 150, 200, 255]);
        apply_blur(&mut img, 1);
        assert_eq!(px(&img, 2, 2), [100, 150, 200, 255]);
    }

    #[test]
    fn blur_spreads_a_bright_pixel() {
        // A single white pixel on black; after blur its neighbours brighten and
        // its own value drops.
        let mut img = RgbaImage::new(5, 5);
        let ci = (2 * 5 + 2) * 4;
        img.pixels[ci..ci + 4].copy_from_slice(&[255, 255, 255, 255]);
        apply_blur(&mut img, 1);
        assert!(px(&img, 2, 2)[0] < 255, "center spread out");
        assert!(px(&img, 1, 2)[0] > 0, "neighbour picked up brightness");
    }

    #[test]
    fn vignette_darkens_corners_not_center() {
        let mut img = RgbaImage::solid(9, 9, [200, 200, 200, 255]);
        apply_vignette(&mut img, 0.8);
        assert_eq!(px(&img, 4, 4), [200, 200, 200, 255], "centre unchanged");
        assert!(px(&img, 0, 0)[0] < 200, "corner darkened");
        assert_eq!(px(&img, 0, 0)[3], 255, "alpha untouched");
    }

    #[test]
    fn vignette_zero_is_noop() {
        let mut img = RgbaImage::solid(4, 4, [123, 45, 67, 255]);
        let before = img.pixels.clone();
        apply_vignette(&mut img, 0.0);
        assert_eq!(img.pixels, before);
    }

    #[test]
    fn blur_radius_zero_is_noop() {
        let mut img = RgbaImage::new(3, 3);
        img.pixels[0..4].copy_from_slice(&[10, 20, 30, 255]);
        let before = img.pixels.clone();
        apply_blur(&mut img, 0);
        assert_eq!(img.pixels, before);
    }

    // Naive O(w*h*radius) box blur — the reference the prefix-sum path must match
    // bit-for-bit.
    fn blur_naive(img: &mut RgbaImage, radius: usize) {
        if radius == 0 || img.width == 0 || img.height == 0 {
            return;
        }
        let (w, h) = (img.width, img.height);
        let mut tmp = vec![0u8; img.pixels.len()];
        for y in 0..h {
            for x in 0..w {
                let lo = x.saturating_sub(radius);
                let hi = (x + radius).min(w - 1);
                for c in 0..4 {
                    let mut sum = 0.0;
                    for sx in lo..=hi {
                        sum += img.pixels[(y * w + sx) * 4 + c] as f64;
                    }
                    let count = (hi - lo + 1) as f64;
                    tmp[(y * w + x) * 4 + c] = (sum / count).round().clamp(0.0, 255.0) as u8;
                }
            }
        }
        for y in 0..h {
            for x in 0..w {
                let lo = y.saturating_sub(radius);
                let hi = (y + radius).min(h - 1);
                for c in 0..4 {
                    let mut sum = 0.0;
                    for sy in lo..=hi {
                        sum += tmp[(sy * w + x) * 4 + c] as f64;
                    }
                    let count = (hi - lo + 1) as f64;
                    img.pixels[(y * w + x) * 4 + c] =
                        (sum / count).round().clamp(0.0, 255.0) as u8;
                }
            }
        }
    }

    #[test]
    fn blur_matches_naive_reference() {
        // Deterministic pseudo-random image; assert bit-identity across radii and
        // a non-square shape (exercises the separable passes independently).
        for (w, h) in [(17usize, 11usize), (11, 17), (1, 20), (20, 1)] {
            let mut img = RgbaImage::new(w, h);
            let mut s: u32 = 0x9E3779B9;
            for p in img.pixels.iter_mut() {
                s = s.wrapping_mul(1664525).wrapping_add(1013904223);
                *p = (s >> 24) as u8;
            }
            for radius in [1usize, 2, 3, 5, 8, 30] {
                let mut a = img.clone();
                let mut b = img.clone();
                apply_blur(&mut a, radius);
                blur_naive(&mut b, radius);
                assert_eq!(
                    a.pixels, b.pixels,
                    "prefix-sum blur diverged from naive at {w}x{h} radius {radius}"
                );
            }
        }
    }

    #[test]
    fn brightness_adjustment_adds() {
        let mut img = RgbaImage::solid(1, 1, [0, 0, 0, 255]);
        apply_color_adjustments(&mut img, &[color_effect("color.brightness", 0.5)]);
        assert_eq!(px(&img, 0, 0), [128, 128, 128, 255]);
    }

    #[test]
    fn exposure_adjustment_scales() {
        let mut img = RgbaImage::solid(1, 1, [64, 64, 64, 255]); // ~0.25
        apply_color_adjustments(&mut img, &[color_effect("color.exposure", 1.0)]); // ×2
        assert_eq!(px(&img, 0, 0), [128, 128, 128, 255]);
    }

    #[test]
    fn contrast_adjustment_pushes_from_mid() {
        let mut img = RgbaImage::solid(1, 1, [64, 64, 64, 255]); // 0.25
        apply_color_adjustments(&mut img, &[color_effect("color.contrast", 2.0)]);
        // (0.25 - 0.5) * 2 + 0.5 = 0.0
        assert_eq!(px(&img, 0, 0), [0, 0, 0, 255]);
    }

    #[test]
    fn saturation_zero_greys_out() {
        let mut img = RgbaImage::solid(1, 1, [255, 0, 0, 255]);
        apply_color_adjustments(&mut img, &[color_effect("color.saturation", 0.0)]);
        let p = px(&img, 0, 0);
        // Fully desaturated red → its luma in every channel (~76).
        assert!(p[0] == p[1] && p[1] == p[2], "grey: {p:?}");
        assert!((p[0] as i32 - 76).abs() <= 1, "luma of red, got {}", p[0]);
    }

    #[test]
    fn color_adjust_accumulates_in_float_no_midpoint_clamp() {
        // Exposure over-drives 0.5 to ~2.0; a following brightness cut of -0.5 must
        // leave it clamped at white (1.5→1.0), NOT re-derived from a clamped-to-1.0
        // u8 intermediate (which would give 0.5 → 128).
        let mut img = RgbaImage::solid(1, 1, [128, 128, 128, 255]);
        apply_color_adjustments(
            &mut img,
            &[
                color_effect("color.exposure", 2.0),
                color_effect("color.brightness", -0.5),
            ],
        );
        assert_eq!(
            px(&img, 0, 0),
            [255, 255, 255, 255],
            "highlight survives the brightness cut"
        );
    }

    #[test]
    fn color_adjust_preserves_stack_order_and_multiplicity() {
        // The effect stack is ordered, so list order is observable and stacking the
        // same adjustment compounds (NOT collapsed to a canonical order / last-wins).
        let run = |effects: &[crate::effects::EffectState], v: u8| {
            let mut img = RgbaImage::solid(1, 1, [v, v, v, 255]);
            apply_color_adjustments(&mut img, effects);
            px(&img, 0, 0)[0]
        };
        let ev12 = (1.2f64).log2(); // exposure ×1.2
        let a = run(
            &[
                color_effect("color.contrast", 1.5),
                color_effect("color.exposure", ev12),
            ],
            100,
        );
        let b = run(
            &[
                color_effect("color.exposure", ev12),
                color_effect("color.contrast", 1.5),
            ],
            100,
        );
        assert_ne!(a, b, "effect order must change the result");
        // Two ×2 exposures compound to ×4: 32 → 128 (last-wins would give 64).
        let doubled = run(
            &[
                color_effect("color.exposure", 1.0),
                color_effect("color.exposure", 1.0),
            ],
            32,
        );
        assert!(
            (doubled as i32 - 128).abs() <= 1,
            "stacked exposures compound, got {doubled}"
        );
    }

    #[test]
    fn chroma_key_spill_suppression_reduces_green_bleed() {
        // A greenish pixel that is NOT close enough to be keyed out.
        let mut img = RgbaImage::solid(1, 1, [77, 204, 77, 255]); // ~[0.3, 0.8, 0.3]
        let key = core_model::ChromaKey {
            enabled: true,
            key_r: 0.0,
            key_g: 1.0,
            key_b: 0.0,
            tolerance: 0.2, // cutoff ~0.35; this pixel's distance ~0.47 → kept
            spill_suppression: 1.0,
        };
        apply_chroma_key(&mut img, &key);
        let p = px(&img, 0, 0);
        assert_eq!(p[3], 255, "not keyed out");
        assert!(p[1] < 204, "green spill reduced (was 204, now {})", p[1]);
        assert!(p[1] <= 78, "pulled toward the ~0.3 of the other channels");
    }

    #[test]
    fn chroma_key_disabled_is_noop() {
        let mut img = RgbaImage::solid(1, 1, [0, 255, 0, 255]);
        let key = core_model::ChromaKey {
            enabled: false,
            key_r: 0.0,
            key_g: 1.0,
            key_b: 0.0,
            tolerance: 0.5,
            spill_suppression: 0.0,
        };
        apply_chroma_key(&mut img, &key);
        assert_eq!(px(&img, 0, 0), [0, 255, 0, 255]);
    }

    #[test]
    fn compose_renders_text_clip() {
        use core_model::{TextAlignment, TextRgba, TextStyle};
        let mut c = clip("t1", "", 0, 10);
        c.media_type = ClipType::Text;
        c.text_content = Some("Hi".into());
        c.transform = Transform::from_top_left(0.0, 0.0, 1.0, 1.0);
        c.text_style = Some(TextStyle {
            font_name: "Poppins".into(),
            font_size: 40.0,
            font_scale: 1.0,
            color: TextRgba { r: 1.0, g: 0.0, b: 0.0, a: 1.0 },
            alignment: TextAlignment::Center,
            shadow: Default::default(),
            background: Default::default(),
            border: Default::default(),
            font_weight: 400.0,
            is_italic: false,
            variable_font_axes: None,
            letter_spacing: None,
            line_height: None,
        });
        let timeline = tl(vec![c]);
        // Text is procedural — fetch_source is never consulted. Use a tall canvas
        // so the 1080-reference font scaling yields visible glyphs.
        let out = compose_frame(&timeline, &MediaManifest::default(), 0, 400, 1080, |_| None);
        // Some pixels are painted with the (red) text colour.
        let lit = out
            .pixels
            .chunks_exact(4)
            .any(|p| p[3] > 150 && p[0] > p[1] && p[0] > p[2]);
        assert!(lit, "text glyphs composited onto the canvas");
    }

    #[test]
    fn compose_renders_shape_clip() {
        use core_model::shape_style::ShapeKind;
        let mut c = clip("s1", "", 0, 10);
        c.media_type = ClipType::Shape;
        c.shape_style = Some(solid_shape(ShapeKind::Rect, [0.0, 1.0, 0.0, 1.0]));
        c.transform = Transform::from_top_left(0.0, 0.0, 1.0, 1.0);
        let timeline = tl(vec![c]);
        // Shapes are procedural — fetch_source is never consulted for them.
        let out = compose_frame(&timeline, &MediaManifest::default(), 0, 4, 4, |_| None);
        assert_eq!(px(&out, 2, 2), [0, 255, 0, 255], "shape fill composited");
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

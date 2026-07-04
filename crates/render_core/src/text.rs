//! Text-overlay rasterization for the compositor (PR #46 / text clips).
//!
//! Renders a clip's `text_content` into an RGBA layer using a bundled font
//! (Poppins) via the pure-Rust `ab_glyph` rasterizer — the linked ffmpeg has no
//! text support and the compositor stays platform-free. Covers bundled font
//! families (by name), Regular/Bold weight, `\n` line breaks, L/C/R alignment,
//! letter spacing, line height, drop shadow, caption background, and outline —
//! with `font_size` scaled to Swift's 1080-tall reference canvas. Text rotation,
//! shadow blur, rounded background corners, and variable-font axes are follow-ups.

use crate::compositor::RgbaImage;
use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use core_model::{TextAlignment, TextStyle};

macro_rules! font {
    ($p:literal) => {
        include_bytes!(concat!("../../../Sources/PalmierPro/Resources/Fonts/", $p))
    };
}

static POPPINS_REGULAR: &[u8] = font!("Poppins/Poppins-Regular.ttf");
static POPPINS_BOLD: &[u8] = font!("Poppins/Poppins-Bold.ttf");
static ANTON: &[u8] = font!("Anton/Anton-Regular.ttf");
static BEBAS_NEUE: &[u8] = font!("BebasNeue/BebasNeue-Regular.ttf");
static PERMANENT_MARKER: &[u8] = font!("PermanentMarker/PermanentMarker-Regular.ttf");
static SHRIKHAND: &[u8] = font!("Shrikhand/Shrikhand-Regular.ttf");
static BASEMENT_GROTESQUE: &[u8] = font!("BasementGrotesque/BasementGrotesque-Black.ttf");

/// Pick the embedded font bytes for a `font_name` (case-insensitive substring
/// match against a bundled family), honouring `bold` for families that ship a
/// bold weight. Falls back to Poppins for unknown / system fonts (e.g. the
/// default "Helvetica-Bold", which is not bundled).
fn font_for(font_name: &str, bold: bool) -> &'static [u8] {
    let n = font_name.to_ascii_lowercase();
    let has = |needle: &str| n.replace([' ', '-', '_'], "").contains(needle);
    if has("anton") {
        ANTON
    } else if has("bebas") {
        BEBAS_NEUE
    } else if has("permanentmarker") || has("marker") {
        PERMANENT_MARKER
    } else if has("shrikhand") {
        SHRIKHAND
    } else if has("basementgrotesque") {
        BASEMENT_GROTESQUE
    } else if bold {
        POPPINS_BOLD
    } else {
        POPPINS_REGULAR
    }
}

/// Source-over composite `src` onto `dst` (same size), per pixel.
fn composite_over(dst: &mut RgbaImage, src: &RgbaImage) {
    if dst.width != src.width || dst.height != src.height {
        return;
    }
    for (d, s) in dst.pixels.chunks_exact_mut(4).zip(src.pixels.chunks_exact(4)) {
        let sa = s[3] as f32 / 255.0;
        if sa <= 0.0 {
            continue;
        }
        let da = d[3] as f32 / 255.0;
        let out_a = sa + da * (1.0 - sa);
        if out_a <= 0.0 {
            continue;
        }
        for k in 0..3 {
            let sc = s[k] as f32 / 255.0;
            let dc = d[k] as f32 / 255.0;
            let out = (sc * sa + dc * da * (1.0 - sa)) / out_a;
            d[k] = (out * 255.0).round().clamp(0.0, 255.0) as u8;
        }
        d[3] = (out_a * 255.0).round().clamp(0.0, 255.0) as u8;
    }
}

fn blend_over(img: &mut RgbaImage, x: usize, y: usize, color: [u8; 3], a: f32) {
    if a <= 0.0 {
        return;
    }
    let i = (y * img.width + x) * 4;
    let dst_a = img.pixels[i + 3] as f32 / 255.0;
    let out_a = a + dst_a * (1.0 - a);
    if out_a <= 0.0 {
        return;
    }
    for k in 0..3 {
        let src = color[k] as f32 / 255.0;
        let dst = img.pixels[i + k] as f32 / 255.0;
        let out = (src * a + dst * dst_a * (1.0 - a)) / out_a;
        img.pixels[i + k] = (out * 255.0).round().clamp(0.0, 255.0) as u8;
    }
    img.pixels[i + 3] = (out_a * 255.0).round().clamp(0.0, 255.0) as u8;
}

/// Render `text` into a `cw`×`ch` transparent RGBA layer, its block centered at
/// the normalized point `(cx, cy)` (0..1 of the canvas).
pub fn render_text(
    text: &str,
    style: &TextStyle,
    cw: usize,
    ch: usize,
    cx: f64,
    cy: f64,
) -> RgbaImage {
    let mut img = RgbaImage::new(cw, ch);
    if text.trim().is_empty() || cw == 0 || ch == 0 {
        return img;
    }
    let bytes = font_for(&style.font_name, style.font_weight >= 600.0);
    let Ok(font) = FontRef::try_from_slice(bytes) else {
        return img;
    };
    // Swift sizes text for a 1080-tall reference canvas and scales by
    // canvas_height / 1080 (TextLayerController). Match it so sizes are consistent
    // across export resolutions.
    let canvas_scale = ch as f32 / 1080.0;
    let px = ((style.font_size * style.font_scale) as f32 * canvas_scale).max(1.0);
    let scale = PxScale::from(px);
    let sf = font.as_scaled(scale);
    let letter = style.letter_spacing.unwrap_or(0.0) as f32 * canvas_scale;
    let line_h = px * style.line_height.unwrap_or(1.2).max(0.1) as f32;
    let color = [
        (style.color.r * 255.0).round().clamp(0.0, 255.0) as u8,
        (style.color.g * 255.0).round().clamp(0.0, 255.0) as u8,
        (style.color.b * 255.0).round().clamp(0.0, 255.0) as u8,
    ];
    let alpha = style.color.a.clamp(0.0, 1.0) as f32;

    let lines: Vec<&str> = text.split('\n').collect();
    let line_width = |line: &str| -> f32 {
        line.chars().map(|c| sf.h_advance(font.glyph_id(c)) + letter).sum()
    };
    let max_width = lines.iter().map(|l| line_width(l)).fold(0.0f32, f32::max);

    let center_x = (cx * cw as f64) as f32;
    let center_y = (cy * ch as f64) as f32;
    let block_top = center_y - lines.len() as f32 * line_h / 2.0;
    let block_bottom = block_top + lines.len() as f32 * line_h;
    let ascent = sf.ascent();

    // One glyph-drawing pass, offset by (dx, dy) in `color` — used for the drop
    // shadow and the main fill.
    let draw_glyphs = |img: &mut RgbaImage, dx: f32, dy: f32, color: [u8; 3], alpha: f32| {
        for (li, line) in lines.iter().enumerate() {
            let width = line_width(line);
            let start_x = match style.alignment {
                TextAlignment::Left => center_x - max_width / 2.0,
                TextAlignment::Center => center_x - width / 2.0,
                TextAlignment::Right => center_x + max_width / 2.0 - width,
            };
            let base_y = block_top + li as f32 * line_h + ascent;
            let mut pen_x = start_x;
            for c in line.chars() {
                let gid = font.glyph_id(c);
                let glyph =
                    gid.with_scale_and_position(scale, ab_glyph::point(pen_x + dx, base_y + dy));
                if let Some(outlined) = font.outline_glyph(glyph) {
                    let bounds = outlined.px_bounds();
                    outlined.draw(|gx, gy, coverage| {
                        let x = bounds.min.x as i32 + gx as i32;
                        let y = bounds.min.y as i32 + gy as i32;
                        if x >= 0 && (x as usize) < cw && y >= 0 && (y as usize) < ch {
                            blend_over(img, x as usize, y as usize, color, coverage * alpha);
                        }
                    });
                }
                pen_x += sf.h_advance(gid) + letter;
            }
        }
    };

    // Caption background pill behind the text block (Issue #18): padded rect with
    // optional rounded corners (radius scaled to the reference like the font).
    if style.background.enabled {
        let pad = style.background.padding.unwrap_or(0.0) as f32 * canvas_scale;
        let bg = [
            (style.background.color.r * 255.0).round().clamp(0.0, 255.0) as u8,
            (style.background.color.g * 255.0).round().clamp(0.0, 255.0) as u8,
            (style.background.color.b * 255.0).round().clamp(0.0, 255.0) as u8,
        ];
        let ba = style.background.color.a.clamp(0.0, 1.0) as f32;
        let fx0 = center_x - max_width / 2.0 - pad;
        let fx1 = center_x + max_width / 2.0 + pad;
        let fy0 = block_top - pad;
        let fy1 = block_bottom + pad;
        let x0 = fx0.floor().max(0.0) as usize;
        let x1 = (fx1.ceil().max(0.0) as usize).min(cw);
        let y0 = fy0.floor().max(0.0) as usize;
        let y1 = (fy1.ceil().max(0.0) as usize).min(ch);
        // Clamp the radius to half the smaller side.
        let radius = (style.background.corner_radius.unwrap_or(0.0) as f32 * canvas_scale)
            .min(((fx1 - fx0).min(fy1 - fy0)) / 2.0)
            .max(0.0);
        for y in y0..y1 {
            for x in x0..x1 {
                if radius > 0.5 {
                    let px = x as f32 + 0.5;
                    let py = y as f32 + 0.5;
                    // Distance outside the inset rounded region's corner circles.
                    let dx = (fx0 + radius - px).max(px - (fx1 - radius)).max(0.0);
                    let dy = (fy0 + radius - py).max(py - (fy1 - radius)).max(0.0);
                    if dx * dx + dy * dy > radius * radius {
                        continue;
                    }
                }
                blend_over(&mut img, x, y, bg, ba);
            }
        }
    }

    // Drop shadow behind the text (offset + optional blur).
    if style.shadow.enabled && style.shadow.color.a > 0.0 {
        let sc = [
            (style.shadow.color.r * 255.0).round().clamp(0.0, 255.0) as u8,
            (style.shadow.color.g * 255.0).round().clamp(0.0, 255.0) as u8,
            (style.shadow.color.b * 255.0).round().clamp(0.0, 255.0) as u8,
        ];
        let sa = style.shadow.color.a.clamp(0.0, 1.0) as f32;
        // Offset is a 1080-reference quantity like everything else — scale it so the
        // shadow displacement tracks the glyphs at any export resolution (Swift
        // TextLayerController scales offsetX/Y by canvas_height / 1080).
        let (ox, oy) = (
            style.shadow.offset_x as f32 * canvas_scale,
            style.shadow.offset_y as f32 * canvas_scale,
        );
        let blur = (style.shadow.blur as f32 * canvas_scale).round() as usize;
        if blur > 0 {
            // Render + blur the shadow on its own layer, then composite it under.
            let mut shadow = RgbaImage::new(cw, ch);
            draw_glyphs(&mut shadow, ox, oy, sc, sa);
            crate::compositor::apply_blur(&mut shadow, blur);
            composite_over(&mut img, &shadow);
        } else {
            draw_glyphs(&mut img, ox, oy, sc, sa);
        }
    }

    // Text outline/stroke: draw the glyphs in the border colour at 8 offsets so
    // the main fill sits inside an outline (approximate; a true outline is a
    // follow-up). `border.padding` is the stroke width.
    if style.border.enabled && style.border.color.a > 0.0 {
        let bc = [
            (style.border.color.r * 255.0).round().clamp(0.0, 255.0) as u8,
            (style.border.color.g * 255.0).round().clamp(0.0, 255.0) as u8,
            (style.border.color.b * 255.0).round().clamp(0.0, 255.0) as u8,
        ];
        let ba = style.border.color.a.clamp(0.0, 1.0) as f32;
        // Stroke width is a 1080-reference quantity — scale it so the outline stays
        // proportional to the glyphs at higher resolutions (Swift scales borderWidth
        // by canvas_height / 1080).
        let r = style.border.padding.unwrap_or(2.0).max(0.5) as f32 * canvas_scale;
        for (ox, oy) in [
            (-r, 0.0),
            (r, 0.0),
            (0.0, -r),
            (0.0, r),
            (-r, -r),
            (r, -r),
            (-r, r),
            (r, r),
        ] {
            draw_glyphs(&mut img, ox, oy, bc, ba);
        }
    }

    draw_glyphs(&mut img, 0.0, 0.0, color, alpha);
    img
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_model::{TextAlignment, TextRgba, TextStyle};

    fn style(size: f64, color: TextRgba, align: TextAlignment) -> TextStyle {
        TextStyle {
            font_name: "Poppins".into(),
            font_size: size,
            font_scale: 1.0,
            color,
            alignment: align,
            // Off by default so tests opt in; TextStyle's Default enables a shadow.
            shadow: core_model::TextShadow { enabled: false, ..Default::default() },
            background: Default::default(),
            border: Default::default(),
            font_weight: 400.0,
            is_italic: false,
            variable_font_axes: None,
            letter_spacing: None,
            line_height: None,
        }
    }

    fn any_opaque(img: &RgbaImage) -> usize {
        img.pixels.chunks_exact(4).filter(|p| p[3] > 0).count()
    }

    #[test]
    fn renders_visible_glyphs() {
        let red = TextRgba { r: 1.0, g: 0.0, b: 0.0, a: 1.0 };
        let img = render_text("Hi", &style(120.0, red, TextAlignment::Center), 400, 1080, 0.5, 0.5);
        let painted = any_opaque(&img);
        assert!(painted > 20, "glyphs painted some pixels, got {painted}");
        // A painted pixel carries the text colour (red-dominant).
        let lit = img.pixels.chunks_exact(4).find(|p| p[3] > 200).unwrap();
        assert!(lit[0] > lit[1] && lit[0] > lit[2], "red text");
    }

    #[test]
    fn font_for_maps_bundled_families_and_defaults() {
        assert!(std::ptr::eq(font_for("Anton", false), ANTON));
        assert!(std::ptr::eq(font_for("Bebas Neue", false), BEBAS_NEUE));
        assert!(std::ptr::eq(font_for("Permanent Marker", false), PERMANENT_MARKER));
        // Unknown / system font → Poppins (regular vs bold by weight flag).
        assert!(std::ptr::eq(font_for("Helvetica-Bold", true), POPPINS_BOLD));
        assert!(std::ptr::eq(font_for("Whatever", false), POPPINS_REGULAR));
    }

    #[test]
    fn empty_text_is_blank() {
        let c = TextRgba::default();
        let img = render_text("   ", &style(40.0, c, TextAlignment::Left), 100, 50, 0.5, 0.5);
        assert_eq!(any_opaque(&img), 0);
    }

    #[test]
    fn shadow_adds_painted_pixels() {
        use core_model::TextShadow;
        let w = TextRgba { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
        let mut s = style(120.0, w, TextAlignment::Center);
        let no_shadow = render_text("Hi", &s, 400, 1080, 0.5, 0.5);
        s.shadow = TextShadow {
            enabled: true,
            color: TextRgba { r: 0.0, g: 0.0, b: 0.0, a: 1.0 },
            offset_x: 6.0,
            offset_y: 6.0,
            blur: 0.0,
        };
        let with_shadow = render_text("Hi", &s, 400, 1080, 0.5, 0.5);
        assert!(
            any_opaque(&with_shadow) > any_opaque(&no_shadow),
            "shadow paints extra pixels"
        );

        // A blurred shadow still renders and differs from the sharp one.
        s.shadow.blur = 8.0;
        let blurred = render_text("Hi", &s, 400, 1080, 0.5, 0.5);
        assert!(any_opaque(&blurred) > 0, "blurred shadow renders");
        assert!(blurred.pixels != with_shadow.pixels, "blur changed the shadow");
    }

    #[test]
    fn background_fills_behind_text() {
        use core_model::TextFill;
        let black = TextRgba { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };
        let mut s = style(120.0, black, TextAlignment::Center);
        s.background = TextFill {
            enabled: true,
            color: TextRgba { r: 0.0, g: 0.5, b: 1.0, a: 1.0 },
            padding: Some(8.0),
            corner_radius: None,
        };
        let img = render_text("Hi", &s, 400, 1080, 0.5, 0.5);
        // A solid rectangle of the (blue) background is painted.
        let blue = img
            .pixels
            .chunks_exact(4)
            .filter(|p| p[3] > 200 && p[2] > p[0] && p[2] > p[1])
            .count();
        assert!(blue > 200, "background rect painted, got {blue}");

        // With a large corner radius, the extreme corners are clipped away.
        s.background.corner_radius = Some(40.0);
        let rounded = render_text("Hi", &s, 400, 1080, 0.5, 0.5);
        let rounded_blue = rounded
            .pixels
            .chunks_exact(4)
            .filter(|p| p[3] > 200 && p[2] > p[0] && p[2] > p[1])
            .count();
        assert!(rounded_blue < blue, "rounded corners remove some fill");
    }

    #[test]
    fn border_outlines_the_text() {
        use core_model::TextFill;
        // White text with a black outline: black pixels appear around the glyphs.
        let white = TextRgba { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
        let mut s = style(120.0, white, TextAlignment::Center);
        s.border = TextFill {
            enabled: true,
            color: TextRgba { r: 0.0, g: 0.0, b: 0.0, a: 1.0 },
            padding: Some(3.0),
            corner_radius: None,
        };
        let img = render_text("Hi", &s, 400, 1080, 0.5, 0.5);
        let dark = img
            .pixels
            .chunks_exact(4)
            .filter(|p| p[3] > 150 && p[0] < 60 && p[1] < 60 && p[2] < 60)
            .count();
        assert!(dark > 20, "outline paints dark pixels, got {dark}");
    }

    #[test]
    fn multiline_paints_more_than_single_line() {
        let w = TextRgba { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
        let one = render_text("AA", &style(120.0, w, TextAlignment::Center), 400, 1080, 0.5, 0.5);
        let two =
            render_text("AA\nAA", &style(120.0, w, TextAlignment::Center), 400, 1080, 0.5, 0.5);
        assert!(any_opaque(&two) > any_opaque(&one), "two lines paint more");
    }

    // Horizontal span (px) of the opaque region — grows ~2x when every geometric
    // quantity scales with a 2x canvas.
    fn opaque_x_span(img: &RgbaImage) -> i32 {
        let xs: Vec<usize> = (0..img.width)
            .filter(|&x| (0..img.height).any(|y| img.pixels[(y * img.width + x) * 4 + 3] > 0))
            .collect();
        match (xs.first(), xs.last()) {
            (Some(&lo), Some(&hi)) => (hi - lo) as i32,
            _ => 0,
        }
    }

    #[test]
    fn shadow_offset_scales_with_canvas_resolution() {
        use core_model::TextShadow;
        // A big rightward shadow offset dominates the horizontal span. At a 2x canvas
        // the offset must double with the glyphs, so the span ~doubles; if the offset
        // stayed fixed the span would grow far less.
        let w = TextRgba { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
        let mut s = style(40.0, w, TextAlignment::Center);
        s.shadow = TextShadow {
            enabled: true,
            color: TextRgba { r: 0.0, g: 0.0, b: 0.0, a: 1.0 },
            offset_x: 200.0,
            offset_y: 0.0,
            blur: 0.0,
        };
        let span1 = opaque_x_span(&render_text("I", &s, 1600, 1080, 0.5, 0.5));
        let span2 = opaque_x_span(&render_text("I", &s, 1600, 2160, 0.5, 0.5));
        assert!(span1 > 0 && span2 > 0, "both render");
        assert!(
            span2 as f64 >= 1.8 * span1 as f64,
            "shadow offset must scale with resolution: span1={span1}, span2={span2}"
        );
    }

    #[test]
    fn outline_width_scales_with_canvas_resolution() {
        use core_model::TextFill;
        // A thick outline extends the span by its stroke width per side. At a 2x
        // canvas the stroke must double, so the span ~doubles.
        let w = TextRgba { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
        let mut s = style(40.0, w, TextAlignment::Center);
        s.border = TextFill {
            enabled: true,
            color: TextRgba { r: 0.0, g: 0.0, b: 0.0, a: 1.0 },
            padding: Some(100.0),
            corner_radius: None,
        };
        let span1 = opaque_x_span(&render_text("I", &s, 1600, 1080, 0.5, 0.5));
        let span2 = opaque_x_span(&render_text("I", &s, 1600, 2160, 0.5, 0.5));
        assert!(span1 > 0 && span2 > 0, "both render");
        assert!(
            span2 as f64 >= 1.8 * span1 as f64,
            "outline width must scale with resolution: span1={span1}, span2={span2}"
        );
    }
}

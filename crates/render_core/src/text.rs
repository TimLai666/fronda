//! Text-overlay rasterization for the compositor (PR #46 / text clips).
//!
//! Renders a clip's `text_content` into an RGBA layer using a bundled font
//! (Poppins) via the pure-Rust `ab_glyph` rasterizer — the linked ffmpeg has no
//! text support and the compositor stays platform-free. v1 covers a single
//! embedded font family (weight → Regular/Bold), `font_size` as pixels, `\n`
//! line breaks, left/center/right alignment, letter spacing, and line height.
//! Per-family fonts, rotation, shadow/stroke, and exact Swift size calibration
//! are follow-ups.

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
    let px = (style.font_size * style.font_scale).max(1.0) as f32;
    let scale = PxScale::from(px);
    let sf = font.as_scaled(scale);
    let letter = style.letter_spacing.unwrap_or(0.0) as f32;
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

    // Caption background pill behind the text block (Issue #18). Corner rounding
    // is a follow-up; padding is honoured.
    if style.background.enabled {
        let pad = style.background.padding.unwrap_or(0.0) as f32;
        let bg = [
            (style.background.color.r * 255.0).round().clamp(0.0, 255.0) as u8,
            (style.background.color.g * 255.0).round().clamp(0.0, 255.0) as u8,
            (style.background.color.b * 255.0).round().clamp(0.0, 255.0) as u8,
        ];
        let ba = style.background.color.a.clamp(0.0, 1.0) as f32;
        let x0 = (center_x - max_width / 2.0 - pad).floor().max(0.0) as usize;
        let x1 = ((center_x + max_width / 2.0 + pad).ceil().max(0.0) as usize).min(cw);
        let y0 = (block_top - pad).floor().max(0.0) as usize;
        let y1 = ((block_bottom + pad).ceil().max(0.0) as usize).min(ch);
        for y in y0..y1 {
            for x in x0..x1 {
                blend_over(&mut img, x, y, bg, ba);
            }
        }
    }

    // Drop shadow behind the text (offset; blur is a follow-up).
    if style.shadow.enabled && style.shadow.color.a > 0.0 {
        let sc = [
            (style.shadow.color.r * 255.0).round().clamp(0.0, 255.0) as u8,
            (style.shadow.color.g * 255.0).round().clamp(0.0, 255.0) as u8,
            (style.shadow.color.b * 255.0).round().clamp(0.0, 255.0) as u8,
        ];
        let sa = style.shadow.color.a.clamp(0.0, 1.0) as f32;
        draw_glyphs(
            &mut img,
            style.shadow.offset_x as f32,
            style.shadow.offset_y as f32,
            sc,
            sa,
        );
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
            shadow: Default::default(),
            background: Default::default(),
            border: Default::default(),
            font_weight: 400.0,
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
        let img = render_text("Hi", &style(40.0, red, TextAlignment::Center), 200, 80, 0.5, 0.5);
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
        let mut s = style(40.0, w, TextAlignment::Center);
        let no_shadow = render_text("Hi", &s, 200, 120, 0.5, 0.5);
        s.shadow = TextShadow {
            enabled: true,
            color: TextRgba { r: 0.0, g: 0.0, b: 0.0, a: 1.0 },
            offset_x: 6.0,
            offset_y: 6.0,
            blur: 0.0,
        };
        let with_shadow = render_text("Hi", &s, 200, 120, 0.5, 0.5);
        assert!(
            any_opaque(&with_shadow) > any_opaque(&no_shadow),
            "shadow paints extra pixels"
        );
    }

    #[test]
    fn background_fills_behind_text() {
        use core_model::TextFill;
        let black = TextRgba { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };
        let mut s = style(30.0, black, TextAlignment::Center);
        s.background = TextFill {
            enabled: true,
            color: TextRgba { r: 0.0, g: 0.5, b: 1.0, a: 1.0 },
            padding: Some(8.0),
            corner_radius: None,
        };
        let img = render_text("Hi", &s, 200, 120, 0.5, 0.5);
        // A solid rectangle of the (blue) background is painted.
        let blue = img
            .pixels
            .chunks_exact(4)
            .filter(|p| p[3] > 200 && p[2] > p[0] && p[2] > p[1])
            .count();
        assert!(blue > 200, "background rect painted, got {blue}");
    }

    #[test]
    fn multiline_paints_more_than_single_line() {
        let w = TextRgba { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
        let one = render_text("AA", &style(30.0, w, TextAlignment::Center), 200, 200, 0.5, 0.5);
        let two = render_text("AA\nAA", &style(30.0, w, TextAlignment::Center), 200, 200, 0.5, 0.5);
        assert!(any_opaque(&two) > any_opaque(&one), "two lines paint more");
    }
}

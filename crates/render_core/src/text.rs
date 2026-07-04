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

static REGULAR: &[u8] =
    include_bytes!("../../../Sources/PalmierPro/Resources/Fonts/Poppins/Poppins-Regular.ttf");
static BOLD: &[u8] =
    include_bytes!("../../../Sources/PalmierPro/Resources/Fonts/Poppins/Poppins-Bold.ttf");

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
    let bytes = if style.font_weight >= 600.0 { BOLD } else { REGULAR };
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
    let ascent = sf.ascent();

    for (li, line) in lines.iter().enumerate() {
        let width = line_width(line);
        // Block is centered at center_x; alignment anchors lines within it.
        let start_x = match style.alignment {
            TextAlignment::Left => center_x - max_width / 2.0,
            TextAlignment::Center => center_x - width / 2.0,
            TextAlignment::Right => center_x + max_width / 2.0 - width,
        };
        let base_y = block_top + li as f32 * line_h + ascent;
        let mut pen_x = start_x;
        for c in line.chars() {
            let gid = font.glyph_id(c);
            let glyph = gid.with_scale_and_position(scale, ab_glyph::point(pen_x, base_y));
            if let Some(outlined) = font.outline_glyph(glyph) {
                let bounds = outlined.px_bounds();
                outlined.draw(|gx, gy, coverage| {
                    let x = bounds.min.x as i32 + gx as i32;
                    let y = bounds.min.y as i32 + gy as i32;
                    if x >= 0 && (x as usize) < cw && y >= 0 && (y as usize) < ch {
                        blend_over(&mut img, x as usize, y as usize, color, coverage * alpha);
                    }
                });
            }
            pen_x += sf.h_advance(gid) + letter;
        }
    }
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
    fn empty_text_is_blank() {
        let c = TextRgba::default();
        let img = render_text("   ", &style(40.0, c, TextAlignment::Left), 100, 50, 0.5, 0.5);
        assert_eq!(any_opaque(&img), 0);
    }

    #[test]
    fn multiline_paints_more_than_single_line() {
        let w = TextRgba { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
        let one = render_text("AA", &style(30.0, w, TextAlignment::Center), 200, 200, 0.5, 0.5);
        let two = render_text("AA\nAA", &style(30.0, w, TextAlignment::Center), 200, 200, 0.5, 0.5);
        assert!(any_opaque(&two) > any_opaque(&one), "two lines paint more");
    }
}

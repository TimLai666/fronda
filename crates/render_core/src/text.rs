//! Text-overlay rasterization for the compositor (PR #46 / text clips).
//!
//! Renders a clip's `text_content` into an RGBA layer using a bundled font
//! (Poppins) via the pure-Rust `ab_glyph` rasterizer — the linked ffmpeg has no
//! text support and the compositor stays platform-free. Covers bundled font
//! families (by name), Regular/Bold weight — plus the wght axis on bundled
//! variable families (Inter/Geist/…, Issue #65) — `\n` line breaks, L/C/R
//! alignment, letter spacing + #330 tracking, line height + #330 lineSpacing,
//! fontCase display casing, #336 underline/strikethrough/overline bars, drop
//! shadow, the #330 rich caption background (per-axis padding, corner radius,
//! offset, outline), and the glyph outline (width from #330 `border.width`),
//! with `font_size` scaled to Swift's 1080-tall reference canvas. Text
//! rotation is a follow-up.

use crate::compositor::RgbaImage;
use ab_glyph::{Font, FontRef, PxScale, ScaleFont, VariableFont};
use core_model::{TextAlignment, TextBackgroundStyle, TextStyle};

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
// Variable families (wght axis) — one file spans all weights (Issue #65).
static INTER: &[u8] = font!("Inter/Inter[opsz,wght].ttf");
static GEIST: &[u8] = font!("Geist/Geist[wght].ttf");
static GEIST_MONO: &[u8] = font!("GeistMono/GeistMono[wght].ttf");
static DM_SANS: &[u8] = font!("DMSans/DMSans[opsz,wght].ttf");
static CAVEAT: &[u8] = font!("Caveat/Caveat[wght].ttf");
static PLAYFAIR_DISPLAY: &[u8] = font!("PlayfairDisplay/PlayfairDisplay[wght].ttf");
static SPACE_GROTESK: &[u8] = font!("SpaceGrotesk/SpaceGrotesk[wght].ttf");

/// Pick the embedded font bytes for a `font_name` (case-insensitive substring
/// match against a bundled family). Variable families (Inter/Geist/… — a single
/// file spanning all weights) ignore `bold`; their weight comes from the wght
/// axis applied at render time. Static families honour `bold` via a Bold file.
/// Falls back to Poppins for unknown / system fonts (e.g. the default
/// "Helvetica-Bold", which is not bundled).
fn font_for(font_name: &str, bold: bool) -> &'static [u8] {
    let n = font_name.to_ascii_lowercase();
    let has = |needle: &str| n.replace([' ', '-', '_'], "").contains(needle);
    // Variable families first ("geistmono" before "geist"; both are substrings).
    if has("geistmono") {
        GEIST_MONO
    } else if has("geist") {
        GEIST
    } else if has("inter") {
        INTER
    } else if has("dmsans") {
        DM_SANS
    } else if has("caveat") {
        CAVEAT
    } else if has("playfairdisplay") {
        PLAYFAIR_DISPLAY
    } else if has("spacegrotesk") {
        SPACE_GROTESK
    } else if has("anton") {
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
    for (d, s) in dst
        .pixels
        .chunks_exact_mut(4)
        .zip(src.pixels.chunks_exact(4))
    {
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

/// Swift `TextStyle.displayText`: non-destructive fontCase display casing
/// (#330). Unknown rawValues (future Swift cases) render the original text.
pub fn display_text(style: &TextStyle, text: &str) -> String {
    match style.font_case.as_str() {
        "uppercase" => text.to_uppercase(),
        "lowercase" => text.to_lowercase(),
        _ => text.to_string(),
    }
}

/// Swift `TextStyle.Outline.width` decode-fallback default (core_model
/// `default_border_width`): what `border_width` holds when a file never
/// carried the #330 key.
const BORDER_WIDTH_DEFAULT: f64 = 4.0;

/// Glyph-outline stroke width in reference-canvas points (Swift
/// `border.width`, #330). Pre-#330 Rust files stored it in `border.padding`;
/// that legacy value wins only while `border_width` still holds the decode
/// fallback, so old projects render unchanged.
pub fn effective_border_width(style: &TextStyle) -> f64 {
    match style.border.padding {
        Some(p) if style.border_width == BORDER_WIDTH_DEFAULT => p.max(0.0),
        _ => style.border_width.max(0.0),
    }
}

/// Horizontal decoration bar (underline/strikethrough/overline, #336),
/// centered on `yc` and clamped to the canvas.
#[allow(clippy::too_many_arguments)]
fn draw_bar(
    img: &mut RgbaImage,
    x0: f32,
    x1: f32,
    yc: f32,
    thickness: f32,
    color: [u8; 3],
    alpha: f32,
    cw: usize,
    ch: usize,
) {
    let xs = x0.round().max(0.0) as usize;
    let xe = (x1.round().max(0.0) as usize).min(cw);
    let ys = (yc - thickness / 2.0).round().max(0.0) as usize;
    let rows = thickness.round().max(1.0) as usize;
    for y in ys..(ys + rows).min(ch) {
        for x in xs..xe {
            blend_over(img, x, y, color, alpha);
        }
    }
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
    // #330 fontCase applies at display time; the stored content is untouched.
    let text = display_text(style, text);
    if text.trim().is_empty() || cw == 0 || ch == 0 {
        return img;
    }
    let bytes = font_for(&style.font_name, style.font_weight >= 600.0);
    let Ok(mut font) = FontRef::try_from_slice(bytes) else {
        return img;
    };
    // Variable fonts: drive weight by the wght axis. On static faces this is a
    // no-op (the Regular/Bold file chosen by font_for still applies). Issue #65.
    font.set_variation(b"wght", style.font_weight as f32);
    // Swift sizes text for a 1080-tall reference canvas and scales by
    // canvas_height / 1080 (TextLayerController). Match it so sizes are consistent
    // across export resolutions.
    let canvas_scale = ch as f32 / 1080.0;
    let px = ((style.font_size * style.font_scale) as f32 * canvas_scale).max(1.0);
    let scale = PxScale::from(px);
    let sf = font.as_scaled(scale);
    // #330 tracking and the Rust-native letter_spacing are both per-character
    // advance in reference-canvas points (Swift's kern = tracking · size /
    // (fontSize·fontScale) reduces to tracking · canvas_scale). Swift has no
    // letter_spacing field, so the two add; real styles carry only one.
    let letter = (style.letter_spacing.unwrap_or(0.0) + style.tracking) as f32 * canvas_scale;
    // #330 lineSpacing adds canvas points between lines on top of the native
    // line-height multiplier (Swift paragraphStyle.lineSpacing semantics).
    let line_h = (px * style.line_height.unwrap_or(1.2).max(0.1) as f32
        + style.line_spacing as f32 * canvas_scale)
        .max(1.0);
    let color = [
        (style.color.r * 255.0).round().clamp(0.0, 255.0) as u8,
        (style.color.g * 255.0).round().clamp(0.0, 255.0) as u8,
        (style.color.b * 255.0).round().clamp(0.0, 255.0) as u8,
    ];
    let alpha = style.color.a.clamp(0.0, 1.0) as f32;

    let lines: Vec<&str> = text.split('\n').collect();
    let line_width = |line: &str| -> f32 {
        line.chars()
            .map(|c| sf.h_advance(font.glyph_id(c)) + letter)
            .sum()
    };
    let max_width = lines.iter().map(|l| line_width(l)).fold(0.0f32, f32::max);

    let center_x = (cx * cw as f64) as f32;
    let center_y = (cy * ch as f64) as f32;
    let block_top = center_y - lines.len() as f32 * line_h / 2.0;
    let block_bottom = block_top + lines.len() as f32 * line_h;
    let ascent = sf.ascent();

    // Per-line geometry shared by the glyph passes and the decoration bars.
    let line_geoms: Vec<(f32, f32, f32)> = lines
        .iter()
        .enumerate()
        .map(|(li, line)| {
            let width = line_width(line);
            let start_x = match style.alignment {
                TextAlignment::Left => center_x - max_width / 2.0,
                TextAlignment::Center => center_x - width / 2.0,
                TextAlignment::Right => center_x + max_width / 2.0 - width,
            };
            let base_y = block_top + li as f32 * line_h + ascent;
            (start_x, base_y, width)
        })
        .collect();
    let has_bars = style.is_underlined || style.is_struck_through || style.is_overlined;
    // ≈ CTFontGetUnderlineThickness at this size, floored at 1px like Swift's
    // max(1, …). ab_glyph exposes no underline metrics, so bar geometry is the
    // documented approximation from the design.
    let bar_thickness = (px / 18.0).max(1.0);

    // One glyph-drawing pass, offset by (dx, dy) in `color` — used for the drop
    // shadow and the main fill. `with_bars` adds the #336 decoration bars
    // (shadow + fill passes; the glyph-outline pass strokes glyphs only).
    let draw_glyphs = |img: &mut RgbaImage, dx: f32, dy: f32, color: [u8; 3], alpha: f32, with_bars: bool| {
        for (line, &(start_x, base_y, width)) in lines.iter().zip(&line_geoms) {
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
            if with_bars && has_bars && width > 0.0 {
                let (x0, x1) = (start_x + dx, start_x + width + dx);
                // Underline below the baseline, strikethrough at ~half x-height,
                // overline capping the ascent (Swift: ascent − thickness/2).
                if style.is_underlined {
                    let yc = base_y + dy + 0.12 * px;
                    draw_bar(img, x0, x1, yc, bar_thickness, color, alpha, cw, ch);
                }
                if style.is_struck_through {
                    let yc = base_y + dy - 0.25 * px;
                    draw_bar(img, x0, x1, yc, bar_thickness, color, alpha, cw, ch);
                }
                if style.is_overlined {
                    let yc = base_y + dy - ascent + bar_thickness / 2.0;
                    draw_bar(img, x0, x1, yc, bar_thickness, color, alpha, cw, ch);
                }
            }
        }
    };

    // Caption background pill behind the text block: the #330 rich layout
    // (per-axis padding, corner radius, offset, outline). Pre-#330 Rust files
    // carried a single `padding`/`corner_radius` on the legacy TextFill —
    // honoured only while the rich fields are untouched, so old projects
    // render unchanged.
    if style.background.enabled {
        let bs = &style.background_style;
        let legacy = *bs == TextBackgroundStyle::default()
            && (style.background.padding.is_some() || style.background.corner_radius.is_some());
        let (pad_x, pad_y, radius_pts, off_x, off_y, outline_width) = if legacy {
            let pad = style.background.padding.unwrap_or(0.0).max(0.0);
            let radius = style.background.corner_radius.unwrap_or(0.0);
            (pad, pad, radius, 0.0, 0.0, 0.0)
        } else {
            (
                bs.padding_x.max(0.0),
                bs.padding_y.max(0.0),
                bs.corner_radius,
                bs.offset_x,
                bs.offset_y,
                bs.outline_width.max(0.0),
            )
        };
        let bg = [
            (style.background.color.r * 255.0).round().clamp(0.0, 255.0) as u8,
            (style.background.color.g * 255.0).round().clamp(0.0, 255.0) as u8,
            (style.background.color.b * 255.0).round().clamp(0.0, 255.0) as u8,
        ];
        let ba = style.background.color.a.clamp(0.0, 1.0) as f32;
        let oc = [
            (bs.outline_color.r * 255.0).round().clamp(0.0, 255.0) as u8,
            (bs.outline_color.g * 255.0).round().clamp(0.0, 255.0) as u8,
            (bs.outline_color.b * 255.0).round().clamp(0.0, 255.0) as u8,
        ];
        let oa = bs.outline_color.a.clamp(0.0, 1.0) as f32;
        // Every quantity is in reference-canvas points; positive offsetY moves
        // the box down (Swift offsets by -offsetY in CG's y-up space).
        let (pad_x, pad_y) = (pad_x as f32 * canvas_scale, pad_y as f32 * canvas_scale);
        let (off_x, off_y) = (off_x as f32 * canvas_scale, off_y as f32 * canvas_scale);
        let ow = outline_width as f32 * canvas_scale;
        let fx0 = center_x - max_width / 2.0 - pad_x + off_x;
        let fx1 = center_x + max_width / 2.0 + pad_x + off_x;
        let fy0 = block_top - pad_y + off_y;
        let fy1 = block_bottom + pad_y + off_y;
        // Clamp the radius to half the smaller side (Swift drawBox).
        let radius = (radius_pts as f32 * canvas_scale)
            .min(((fx1 - fx0).min(fy1 - fy0)) / 2.0)
            .max(0.0);
        let x0 = (fx0 - ow / 2.0 - 1.0).floor().max(0.0) as usize;
        let x1 = ((fx1 + ow / 2.0 + 1.0).ceil().max(0.0) as usize).min(cw);
        let y0 = (fy0 - ow / 2.0 - 1.0).floor().max(0.0) as usize;
        let y1 = ((fy1 + ow / 2.0 + 1.0).ceil().max(0.0) as usize).min(ch);
        let (rect_cx, rect_cy) = ((fx0 + fx1) / 2.0, (fy0 + fy1) / 2.0);
        let (half_w, half_h) = ((fx1 - fx0) / 2.0, (fy1 - fy0) / 2.0);
        let stroke = ow > 0.0 && oa > 0.0;
        for y in y0..y1 {
            for x in x0..x1 {
                let pcx = x as f32 + 0.5;
                let pcy = y as f32 + 0.5;
                // Signed distance to the rounded rect (negative inside).
                let qx = (pcx - rect_cx).abs() - (half_w - radius);
                let qy = (pcy - rect_cy).abs() - (half_h - radius);
                let d = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt()
                    + qx.max(qy).min(0.0)
                    - radius;
                if ba > 0.0 && d <= 0.0 {
                    blend_over(&mut img, x, y, bg, ba);
                }
                // Centered stroke on the edge, drawn over the fill like Swift.
                if stroke && d.abs() <= ow / 2.0 {
                    blend_over(&mut img, x, y, oc, oa);
                }
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
            draw_glyphs(&mut shadow, ox, oy, sc, sa, true);
            crate::compositor::apply_blur(&mut shadow, blur);
            composite_over(&mut img, &shadow);
        } else {
            draw_glyphs(&mut img, ox, oy, sc, sa, true);
        }
    }

    // Text outline/stroke: draw the glyphs in the border colour at 8 offsets so
    // the main fill sits inside an outline (approximate; a true outline is a
    // follow-up). #330 `border.width` is the stroke width (see
    // effective_border_width for the pre-#330 fallback).
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
        let width_pts = effective_border_width(style);
        if width_pts > 0.0 {
            let r = (width_pts as f32 * canvas_scale).max(0.5);
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
                draw_glyphs(&mut img, ox, oy, bc, ba, false);
            }
        }
    }

    draw_glyphs(&mut img, 0.0, 0.0, color, alpha, true);
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
            shadow: core_model::TextShadow {
                enabled: false,
                ..Default::default()
            },
            background: Default::default(),
            border: Default::default(),
            font_weight: 400.0,
            is_italic: false,
            variable_font_axes: None,
            letter_spacing: None,
            line_height: None,
            ..Default::default()
        }
    }

    fn any_opaque(img: &RgbaImage) -> usize {
        img.pixels.chunks_exact(4).filter(|p| p[3] > 0).count()
    }

    #[test]
    fn renders_visible_glyphs() {
        let red = TextRgba {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        };
        let img = render_text(
            "Hi",
            &style(120.0, red, TextAlignment::Center),
            400,
            1080,
            0.5,
            0.5,
        );
        let painted = any_opaque(&img);
        assert!(painted > 20, "glyphs painted some pixels, got {painted}");
        // A painted pixel carries the text colour (red-dominant).
        let lit = img.pixels.chunks_exact(4).find(|p| p[3] > 200).unwrap();
        assert!(lit[0] > lit[1] && lit[0] > lit[2], "red text");
    }

    #[test]
    fn font_for_maps_variable_families() {
        // Bundled variable families are selectable (Swift BundledFonts registers
        // every bundled face). "geistmono" must win over "geist" (substring).
        assert!(std::ptr::eq(font_for("Inter", false), INTER));
        assert!(std::ptr::eq(font_for("Geist", false), GEIST));
        assert!(std::ptr::eq(font_for("Geist Mono", false), GEIST_MONO));
        assert!(std::ptr::eq(font_for("DM Sans", false), DM_SANS));
        assert!(std::ptr::eq(font_for("Caveat", false), CAVEAT));
        assert!(std::ptr::eq(
            font_for("Playfair Display", false),
            PLAYFAIR_DISPLAY
        ));
        assert!(std::ptr::eq(
            font_for("Space Grotesk", false),
            SPACE_GROTESK
        ));
        // Variable families ignore the bold flag (weight is an axis, not a file).
        assert!(std::ptr::eq(font_for("Inter", true), INTER));
    }

    #[test]
    fn variable_font_wght_axis_changes_stroke_weight() {
        // Inter is a variable font (wght axis). Both weights are below the 600
        // bold threshold, so font_for returns the SAME file — any rendering
        // difference must come from the applied wght axis, not a Regular/Bold
        // file swap. Issue #65.
        let white = TextRgba {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        };
        let mut light = style(300.0, white, TextAlignment::Center);
        light.font_name = "Inter".into();
        light.font_weight = 100.0;
        let mut heavy = light.clone();
        heavy.font_weight = 590.0;
        let l = any_opaque(&render_text("B", &light, 800, 1080, 0.5, 0.5));
        let h = any_opaque(&render_text("B", &heavy, 800, 1080, 0.5, 0.5));
        assert!(l > 0 && h > 0, "both render (light={l}, heavy={h})");
        assert!(
            h > l,
            "heavier wght paints thicker strokes: light={l}, heavy={h}"
        );
    }

    #[test]
    fn font_for_maps_bundled_families_and_defaults() {
        assert!(std::ptr::eq(font_for("Anton", false), ANTON));
        assert!(std::ptr::eq(font_for("Bebas Neue", false), BEBAS_NEUE));
        assert!(std::ptr::eq(
            font_for("Permanent Marker", false),
            PERMANENT_MARKER
        ));
        // Unknown / system font → Poppins (regular vs bold by weight flag).
        assert!(std::ptr::eq(font_for("Helvetica-Bold", true), POPPINS_BOLD));
        assert!(std::ptr::eq(font_for("Whatever", false), POPPINS_REGULAR));
    }

    #[test]
    fn empty_text_is_blank() {
        let c = TextRgba::default();
        let img = render_text(
            "   ",
            &style(40.0, c, TextAlignment::Left),
            100,
            50,
            0.5,
            0.5,
        );
        assert_eq!(any_opaque(&img), 0);
    }

    #[test]
    fn shadow_adds_painted_pixels() {
        use core_model::TextShadow;
        let w = TextRgba {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        };
        let mut s = style(120.0, w, TextAlignment::Center);
        let no_shadow = render_text("Hi", &s, 400, 1080, 0.5, 0.5);
        s.shadow = TextShadow {
            enabled: true,
            color: TextRgba {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
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
        assert!(
            blurred.pixels != with_shadow.pixels,
            "blur changed the shadow"
        );
    }

    #[test]
    fn background_fills_behind_text() {
        use core_model::TextFill;
        let black = TextRgba {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        };
        let mut s = style(120.0, black, TextAlignment::Center);
        s.background = TextFill {
            enabled: true,
            color: TextRgba {
                r: 0.0,
                g: 0.5,
                b: 1.0,
                a: 1.0,
            },
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
        let white = TextRgba {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        };
        let mut s = style(120.0, white, TextAlignment::Center);
        s.border = TextFill {
            enabled: true,
            color: TextRgba {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
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
        let w = TextRgba {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        };
        let one = render_text(
            "AA",
            &style(120.0, w, TextAlignment::Center),
            400,
            1080,
            0.5,
            0.5,
        );
        let two = render_text(
            "AA\nAA",
            &style(120.0, w, TextAlignment::Center),
            400,
            1080,
            0.5,
            0.5,
        );
        assert!(any_opaque(&two) > any_opaque(&one), "two lines paint more");
    }

    // ── #330/#336 rendering semantics (text-style-v0610-full) ───────────────

    fn white() -> TextRgba {
        TextRgba {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        }
    }

    fn opaque_bbox(img: &RgbaImage, pick: impl Fn(&[u8]) -> bool) -> Option<(usize, usize, usize, usize)> {
        let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0usize, 0usize);
        for y in 0..img.height {
            for x in 0..img.width {
                let p = &img.pixels[(y * img.width + x) * 4..][..4];
                if p[3] > 0 && pick(p) {
                    x0 = x0.min(x);
                    y0 = y0.min(y);
                    x1 = x1.max(x);
                    y1 = y1.max(y);
                }
            }
        }
        (x0 != usize::MAX).then_some((x0, y0, x1, y1))
    }

    #[test]
    fn font_case_transforms_rendered_text() {
        // #330 fontCase: uppercase renders the transformed text (identical to
        // rendering the pre-uppercased content); unknown rawValues fall back to
        // the original so future Swift cases don't corrupt renders.
        let mixed = style(120.0, white(), TextAlignment::Center);
        let reference_upper = render_text("HI", &mixed, 400, 1080, 0.5, 0.5);
        let reference_lower = render_text("hi", &mixed, 400, 1080, 0.5, 0.5);
        assert_ne!(reference_upper.pixels, reference_lower.pixels);

        let mut s = style(120.0, white(), TextAlignment::Center);
        s.font_case = "uppercase".into();
        assert!(
            render_text("hi", &s, 400, 1080, 0.5, 0.5).pixels == reference_upper.pixels,
            "uppercase fontCase renders the uppercased text"
        );
        s.font_case = "lowercase".into();
        assert!(
            render_text("HI", &s, 400, 1080, 0.5, 0.5).pixels == reference_lower.pixels,
            "lowercase fontCase renders the lowercased text"
        );
        s.font_case = "smallcaps-from-the-future".into();
        assert!(
            render_text("hi", &s, 400, 1080, 0.5, 0.5).pixels == reference_lower.pixels,
            "unknown fontCase renders the original text"
        );
    }

    // y-centroid of pixels present in `img` but not painted in `base`.
    fn added_pixel_y_centroid(base: &RgbaImage, img: &RgbaImage) -> f64 {
        let (mut sum, mut n) = (0f64, 0usize);
        for y in 0..img.height {
            for x in 0..img.width {
                let i = (y * img.width + x) * 4 + 3;
                if img.pixels[i] > 0 && base.pixels[i] == 0 {
                    sum += y as f64;
                    n += 1;
                }
            }
        }
        assert!(n > 100, "decoration painted enough new pixels, got {n}");
        sum / n as f64
    }

    #[test]
    fn line_decoration_bars_render_in_vertical_order() {
        // #336: overline caps the ascent, strikethrough crosses the glyphs,
        // underline sits below the baseline — pinned by the relative y order
        // of the pixels each decoration adds.
        let base_style = style(120.0, white(), TextAlignment::Center);
        let base = render_text("TEXT", &base_style, 400, 1080, 0.5, 0.5);
        let mut over = base_style.clone();
        over.is_overlined = true;
        let mut strike = base_style.clone();
        strike.is_struck_through = true;
        let mut under = base_style.clone();
        under.is_underlined = true;
        let y_over = added_pixel_y_centroid(&base, &render_text("TEXT", &over, 400, 1080, 0.5, 0.5));
        let y_strike =
            added_pixel_y_centroid(&base, &render_text("TEXT", &strike, 400, 1080, 0.5, 0.5));
        let y_under =
            added_pixel_y_centroid(&base, &render_text("TEXT", &under, 400, 1080, 0.5, 0.5));
        assert!(
            y_over < y_strike && y_strike < y_under,
            "overline above strikethrough above underline: {y_over} {y_strike} {y_under}"
        );
    }

    #[test]
    fn underline_bar_spans_word_gaps() {
        // A bar is a contiguous line across the whole rendered line, not a
        // per-glyph decoration: with "I I" the underline row must span the
        // space between the glyphs.
        let plain = style(120.0, white(), TextAlignment::Center);
        let mut deco = plain.clone();
        deco.is_underlined = true;
        let max_row = |img: &RgbaImage| -> usize {
            (0..img.height)
                .map(|y| {
                    (0..img.width)
                        .filter(|&x| img.pixels[(y * img.width + x) * 4 + 3] > 0)
                        .count()
                })
                .max()
                .unwrap_or(0)
        };
        let base = max_row(&render_text("I I", &plain, 400, 1080, 0.5, 0.5));
        let with_bar = max_row(&render_text("I I", &deco, 400, 1080, 0.5, 0.5));
        assert!(
            with_bar > base * 2,
            "bar row spans the gap: base widest row {base}, decorated {with_bar}"
        );
    }

    #[test]
    fn tracking_widens_rendered_text_additively_with_letter_spacing() {
        // #330 tracking is a per-character advance in reference-canvas points,
        // exactly like the Rust-native letter_spacing; the two coexist by
        // adding (Swift has no letter_spacing field).
        let plain = style(60.0, white(), TextAlignment::Center);
        let mut tracked = plain.clone();
        tracked.tracking = 12.0;
        let mut spaced = plain.clone();
        spaced.letter_spacing = Some(12.0);
        let base = opaque_x_span(&render_text("TRACK", &plain, 900, 1080, 0.5, 0.5));
        let tracked_span = opaque_x_span(&render_text("TRACK", &tracked, 900, 1080, 0.5, 0.5));
        assert!(
            tracked_span >= base + 40,
            "tracking widens the line: {base} -> {tracked_span}"
        );
        assert!(
            render_text("TRACK", &tracked, 900, 1080, 0.5, 0.5).pixels
                == render_text("TRACK", &spaced, 900, 1080, 0.5, 0.5).pixels,
            "tracking and letter_spacing are the same advance"
        );
        let mut both = tracked.clone();
        both.letter_spacing = Some(12.0);
        let both_span = opaque_x_span(&render_text("TRACK", &both, 900, 1080, 0.5, 0.5));
        assert!(
            both_span >= tracked_span + 40,
            "the two fields add: {tracked_span} -> {both_span}"
        );
    }

    #[test]
    fn line_spacing_expands_line_gap() {
        // #330 lineSpacing adds canvas points between lines on top of the
        // native line-height multiplier.
        let plain = style(80.0, white(), TextAlignment::Center);
        let mut spaced = plain.clone();
        spaced.line_spacing = 40.0;
        let span_y = |img: &RgbaImage| -> i32 {
            let ys: Vec<usize> = (0..img.height)
                .filter(|&y| (0..img.width).any(|x| img.pixels[(y * img.width + x) * 4 + 3] > 0))
                .collect();
            match (ys.first(), ys.last()) {
                (Some(&lo), Some(&hi)) => (hi - lo) as i32,
                _ => 0,
            }
        };
        let base = span_y(&render_text("A\nA", &plain, 400, 1080, 0.5, 0.5));
        let wide = span_y(&render_text("A\nA", &spaced, 400, 1080, 0.5, 0.5));
        assert!(
            wide >= base + 30,
            "lineSpacing 40 must widen the two-line block: {base} -> {wide}"
        );
    }

    #[test]
    fn background_style_axis_padding_offset_and_outline() {
        use core_model::TextBackgroundStyle;
        let red = TextRgba {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        };
        let mut s = style(80.0, TextRgba { a: 0.0, ..white() }, TextAlignment::Center);
        s.background = core_model::TextFill {
            enabled: true,
            color: red,
            padding: None,
            corner_radius: None,
        };
        s.background_style = TextBackgroundStyle {
            padding_x: 40.0,
            padding_y: 5.0,
            ..Default::default()
        };
        let is_red = |p: &[u8]| p[0] > 160 && p[1] < 80 && p[2] < 80;
        let wide = opaque_bbox(&render_text("Hi", &s, 400, 1080, 0.5, 0.5), is_red).unwrap();
        s.background_style.padding_x = 5.0;
        s.background_style.padding_y = 40.0;
        let tall = opaque_bbox(&render_text("Hi", &s, 400, 1080, 0.5, 0.5), is_red).unwrap();
        assert!(
            wide.2 - wide.0 > tall.2 - tall.0,
            "paddingX widens: {wide:?} vs {tall:?}"
        );
        assert!(
            wide.3 - wide.1 < tall.3 - tall.1,
            "paddingY heightens: {wide:?} vs {tall:?}"
        );

        // offset shifts the box (positive y = down, like Swift's -offsetY in CG).
        s.background_style = TextBackgroundStyle {
            offset_x: 40.0,
            offset_y: 30.0,
            ..Default::default()
        };
        let shifted = opaque_bbox(&render_text("Hi", &s, 400, 1080, 0.5, 0.5), is_red).unwrap();
        s.background_style = TextBackgroundStyle::default();
        let centered = opaque_bbox(&render_text("Hi", &s, 400, 1080, 0.5, 0.5), is_red).unwrap();
        assert!(
            shifted.0 >= centered.0 + 30 && shifted.1 >= centered.1 + 20,
            "offset moves the box right+down: {centered:?} -> {shifted:?}"
        );

        // Outline strokes the box edge in its own colour.
        s.background_style = TextBackgroundStyle {
            padding_x: 10.0,
            padding_y: 10.0,
            outline_color: TextRgba {
                r: 0.0,
                g: 0.0,
                b: 1.0,
                a: 1.0,
            },
            outline_width: 8.0,
            ..Default::default()
        };
        let img = render_text("Hi", &s, 400, 1080, 0.5, 0.5);
        let blue = img
            .pixels
            .chunks_exact(4)
            .filter(|p| p[3] > 100 && p[2] > 160 && p[0] < 80 && p[1] < 80)
            .count();
        assert!(blue > 100, "background outline painted, got {blue}");
    }

    #[test]
    fn background_legacy_padding_falls_back_only_when_style_default() {
        use core_model::TextBackgroundStyle;
        let red = TextRgba {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        };
        let mut legacy = style(80.0, TextRgba { a: 0.0, ..white() }, TextAlignment::Center);
        legacy.background = core_model::TextFill {
            enabled: true,
            color: red,
            padding: Some(20.0),
            corner_radius: Some(6.0),
        };
        let mut rich = legacy.clone();
        rich.background.padding = None;
        rich.background.corner_radius = None;
        rich.background_style = TextBackgroundStyle {
            padding_x: 20.0,
            padding_y: 20.0,
            corner_radius: 6.0,
            ..Default::default()
        };
        assert!(
            render_text("Hi", &legacy, 400, 1080, 0.5, 0.5).pixels
                == render_text("Hi", &rich, 400, 1080, 0.5, 0.5).pixels,
            "pre-#330 padding renders exactly like the rich per-axis form"
        );

        // A non-default rich layout wins over the legacy fields.
        let is_red = |p: &[u8]| p[0] > 160 && p[1] < 80 && p[2] < 80;
        let mut overridden = legacy.clone();
        overridden.background_style = TextBackgroundStyle {
            padding_x: 2.0,
            padding_y: 2.0,
            ..Default::default()
        };
        let small = opaque_bbox(
            &render_text("Hi", &overridden, 400, 1080, 0.5, 0.5),
            is_red,
        )
        .unwrap();
        let big = opaque_bbox(&render_text("Hi", &legacy, 400, 1080, 0.5, 0.5), is_red).unwrap();
        assert!(
            small.2 - small.0 < big.2 - big.0,
            "rich fields beat legacy padding: {small:?} vs {big:?}"
        );
    }

    #[test]
    fn border_width_drives_glyph_outline_with_legacy_fallback() {
        use core_model::TextFill;
        let dark_count = |img: &RgbaImage| {
            img.pixels
                .chunks_exact(4)
                .filter(|p| p[3] > 150 && p[0] < 60 && p[1] < 60 && p[2] < 60)
                .count()
        };
        let mut s = style(120.0, white(), TextAlignment::Center);
        s.border = TextFill {
            enabled: true,
            color: TextRgba {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            padding: None,
            corner_radius: None,
        };
        s.border_width = 4.0;
        let thin = dark_count(&render_text("Hi", &s, 400, 1080, 0.5, 0.5));
        s.border_width = 14.0;
        let thick = dark_count(&render_text("Hi", &s, 400, 1080, 0.5, 0.5));
        assert!(thin > 20, "default border_width strokes, got {thin}");
        assert!(thick > thin, "wider border_width strokes more: {thin} -> {thick}");

        // Pre-#330 Rust files kept the stroke width in border.padding; it wins
        // only while border_width still holds the Swift decode fallback (4).
        let mut legacy = s.clone();
        legacy.border_width = 4.0;
        legacy.border.padding = Some(3.0);
        let mut explicit = s.clone();
        explicit.border_width = 3.0;
        explicit.border.padding = None;
        assert!(
            render_text("Hi", &legacy, 400, 1080, 0.5, 0.5).pixels
                == render_text("Hi", &explicit, 400, 1080, 0.5, 0.5).pixels,
            "legacy padding 3 renders exactly like border_width 3"
        );
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
        let w = TextRgba {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        };
        let mut s = style(40.0, w, TextAlignment::Center);
        s.shadow = TextShadow {
            enabled: true,
            color: TextRgba {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
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
        let w = TextRgba {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        };
        let mut s = style(40.0, w, TextAlignment::Center);
        s.border = TextFill {
            enabled: true,
            color: TextRgba {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
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

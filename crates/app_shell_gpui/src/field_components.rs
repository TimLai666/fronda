//! Shared inspector field components (Swift: ColorField, FontPickerField).
//!
//! ColorField: color swatch + hex text entry (cross-platform stand-in for the
//! macOS NSColorPanel flow). FontPickerField: bundled-family dropdown.
use crate::text_field::{TextField, TextFieldEvent};
use crate::theme::{Background, BorderColors, FontSize, IconSize, Opacity, Radius, Spacing, Text};
use core_model::TextRgba;
use gpui::{
    div, prelude::*, px, Context, Entity, EventEmitter, Hsla, InteractiveElement, IntoElement,
    ParentElement, Render, SharedString, Styled, Window,
};

/// Bundled font families shipped under `Sources/PalmierPro/Resources/Fonts/`
/// (Swift `BundledFonts.families`, sorted). The compositor (`render_core::text`)
/// renders Anton / Bebas Neue / Permanent Marker / Shrikhand / Basement
/// Grotesque natively and falls back to Poppins for the rest.
pub const FONT_FAMILIES: &[&str] = &[
    "Anton",
    "Basement Grotesque",
    "Bebas Neue",
    "Caveat",
    "DM Sans",
    "Geist",
    "Geist Mono",
    "Inter",
    "Permanent Marker",
    "Playfair Display",
    "Poppins",
    "Shrikhand",
    "Space Grotesk",
];

/// Parse user hex input (`#RGB`, `#RRGGBB`, `#RRGGBBAA`, leading `#` optional).
pub fn parse_hex_color(input: &str) -> Option<TextRgba> {
    let s = input.trim();
    if s.is_empty() {
        return None;
    }
    TextRgba::from_hex(s)
}

/// Canonical hex form: `#RRGGBB`, or `#RRGGBBAA` when alpha < 1.
pub fn color_to_hex(c: &TextRgba) -> String {
    let byte = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    if c.a < 1.0 {
        format!(
            "#{:02X}{:02X}{:02X}{:02X}",
            byte(c.r),
            byte(c.g),
            byte(c.b),
            byte(c.a)
        )
    } else {
        format!("#{:02X}{:02X}{:02X}", byte(c.r), byte(c.g), byte(c.b))
    }
}

/// RGB(A) → gpui Hsla for swatch display.
pub fn rgba_to_hsla(c: &TextRgba) -> Hsla {
    let (r, g, b) = (
        c.r.clamp(0.0, 1.0) as f32,
        c.g.clamp(0.0, 1.0) as f32,
        c.b.clamp(0.0, 1.0) as f32,
    );
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    let d = max - min;
    let (h, s) = if d.abs() < f32::EPSILON {
        (0.0, 0.0)
    } else {
        let s = if l > 0.5 {
            d / (2.0 - max - min)
        } else {
            d / (max + min)
        };
        let h = if (max - r).abs() < f32::EPSILON {
            ((g - b) / d).rem_euclid(6.0)
        } else if (max - g).abs() < f32::EPSILON {
            (b - r) / d + 2.0
        } else {
            (r - g) / d + 4.0
        };
        (h / 6.0, s)
    };
    Hsla {
        h,
        s,
        l,
        a: c.a.clamp(0.0, 1.0) as f32,
    }
}

// ── ColorField ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum ColorFieldEvent {
    /// User committed a valid new color.
    Changed(TextRgba),
}

/// Swatch + hex entry. Commit with Enter; invalid input reverts to the current color.
pub struct ColorField {
    color: TextRgba,
    enabled: bool,
    hex_field: Entity<TextField>,
}

impl EventEmitter<ColorFieldEvent> for ColorField {}

impl ColorField {
    pub fn new(cx: &mut Context<Self>, initial: TextRgba) -> Self {
        let hex_field = cx.new(|cx| TextField::new(cx, "#FFFFFF"));
        hex_field.update(cx, |f, cx| f.set_text(color_to_hex(&initial), cx));
        cx.subscribe(&hex_field, |this: &mut Self, field, event, cx| {
            if matches!(event, TextFieldEvent::Submitted) {
                let text = field.read(cx).text().to_string();
                match parse_hex_color(&text) {
                    Some(rgba) if this.enabled => {
                        this.color = rgba;
                        field.update(cx, |f, cx| f.set_text(color_to_hex(&rgba), cx));
                        cx.emit(ColorFieldEvent::Changed(rgba));
                    }
                    _ => {
                        let hex = color_to_hex(&this.color);
                        field.update(cx, |f, cx| f.set_text(hex, cx));
                    }
                }
                cx.notify();
            }
        })
        .detach();
        Self {
            color: initial,
            enabled: true,
            hex_field,
        }
    }

    pub fn color(&self) -> TextRgba {
        self.color
    }

    /// Sync from the model (does not emit).
    pub fn set_color(&mut self, color: TextRgba, cx: &mut Context<Self>) {
        if self.color != color {
            self.color = color;
            let hex = color_to_hex(&color);
            self.hex_field.update(cx, |f, cx| f.set_text(hex, cx));
            cx.notify();
        }
    }

    pub fn set_enabled(&mut self, enabled: bool, cx: &mut Context<Self>) {
        if self.enabled != enabled {
            self.enabled = enabled;
            cx.notify();
        }
    }
}

impl Render for ColorField {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let swatch = rgba_to_hsla(&self.color);
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(Spacing::XS))
            .opacity(if self.enabled {
                Opacity::OPAQUE
            } else {
                Opacity::MEDIUM
            })
            .child(
                div()
                    .w(px(IconSize::MD_LG))
                    .h(px(IconSize::XS))
                    .rounded(px(Radius::XS))
                    .bg(swatch)
                    .border_1()
                    .border_color(Hsla {
                        h: 0.0,
                        s: 0.0,
                        l: 1.0,
                        a: Opacity::MEDIUM,
                    }),
            )
            .child(
                div()
                    .w(px(64.0))
                    .text_size(px(FontSize::XS))
                    .child(self.hex_field.clone()),
            )
    }
}

// ── FontPickerField ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FontPickerEvent {
    /// User picked a family from the list.
    Picked(String),
}

/// Current-family button + bundled-family dropdown (Swift FontPickerField's
/// "Featured" section; system fonts are platform-specific and deferred).
pub struct FontPickerField {
    current: String,
    open: bool,
}

impl EventEmitter<FontPickerEvent> for FontPickerField {}

impl FontPickerField {
    pub fn new(_cx: &mut Context<Self>, current: impl Into<String>) -> Self {
        Self {
            current: current.into(),
            open: false,
        }
    }

    pub fn current(&self) -> &str {
        &self.current
    }

    /// Sync from the model (does not emit).
    pub fn set_current(&mut self, name: impl Into<String>, cx: &mut Context<Self>) {
        let name = name.into();
        if self.current != name {
            self.current = name;
            cx.notify();
        }
    }
}

impl Render for FontPickerField {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let open = self.open;
        let current = self.current.clone();

        let mut dropdown = div()
            .id("font-picker-dropdown")
            .absolute()
            .top(px(IconSize::MD))
            .right_0()
            .w(px(160.0))
            .max_h(px(260.0))
            .overflow_y_scroll()
            .bg(Background::RAISED)
            .border_1()
            .border_color(BorderColors::SUBTLE)
            .rounded(px(Radius::SM))
            .flex()
            .flex_col()
            .py(px(Spacing::XS));
        for (i, family) in FONT_FAMILIES.iter().enumerate() {
            let is_current = *family == current;
            let name = family.to_string();
            dropdown = dropdown.child(
                div()
                    .id(SharedString::from(format!("font-opt-{i}")))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(Spacing::SM))
                    .px(px(Spacing::MD))
                    .py(px(Spacing::XS))
                    .cursor_pointer()
                    .on_click(cx.listener(move |this: &mut Self, _, _, cx| {
                        this.current = name.clone();
                        this.open = false;
                        cx.emit(FontPickerEvent::Picked(name.clone()));
                        cx.notify();
                    }))
                    .child(
                        div()
                            .text_size(px(FontSize::XS))
                            .text_color(if is_current {
                                Text::PRIMARY
                            } else {
                                Hsla {
                                    h: 0.0,
                                    s: 0.0,
                                    l: 1.0,
                                    a: 0.0,
                                }
                            })
                            .child("✓"),
                    )
                    .child(
                        div()
                            .text_size(px(FontSize::SM))
                            .text_color(if is_current {
                                Text::PRIMARY
                            } else {
                                Text::SECONDARY
                            })
                            .child(*family),
                    ),
            );
        }

        div()
            .relative()
            .child(
                div()
                    .id("font-picker-btn")
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(Spacing::XS))
                    .px(px(Spacing::SM_MD))
                    .py(px(Spacing::XS))
                    .rounded(px(Radius::SM))
                    .bg(Hsla {
                        h: 0.0,
                        s: 0.0,
                        l: 1.0,
                        a: Opacity::HINT,
                    })
                    .cursor_pointer()
                    .on_click(cx.listener(|this: &mut Self, _, _, cx| {
                        this.open = !this.open;
                        cx.notify();
                    }))
                    .child(
                        div()
                            .max_w(px(120.0))
                            .overflow_hidden()
                            .text_size(px(FontSize::SM))
                            .text_color(Text::PRIMARY)
                            .child(current.clone()),
                    )
                    .child(
                        div()
                            .text_size(px(FontSize::XXS))
                            .text_color(Text::TERTIARY)
                            .child("▾"),
                    ),
            )
            .when(open, |el| el.child(dropdown))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hex_color_accepts_all_swift_forms() {
        let c = parse_hex_color("#FFFFFF").unwrap();
        assert!((c.r - 1.0).abs() < 1e-9 && (c.a - 1.0).abs() < 1e-9);
        let c = parse_hex_color("000000").unwrap();
        assert!(c.r.abs() < 1e-9 && (c.a - 1.0).abs() < 1e-9);
        let c = parse_hex_color("#F00").unwrap();
        assert!((c.r - 1.0).abs() < 1e-9 && c.g.abs() < 1e-9);
        let c = parse_hex_color("#FF000080").unwrap();
        assert!((c.a - 128.0 / 255.0).abs() < 1e-9);
        let c = parse_hex_color("  #00FF00  ").unwrap();
        assert!((c.g - 1.0).abs() < 1e-9);
    }

    #[test]
    fn parse_hex_color_rejects_invalid() {
        assert!(parse_hex_color("").is_none());
        assert!(parse_hex_color("   ").is_none());
        assert!(parse_hex_color("#GGGGGG").is_none());
        assert!(parse_hex_color("#FFFF").is_none());
        assert!(parse_hex_color("not a color").is_none());
    }

    #[test]
    fn color_to_hex_round_trips() {
        for hex in ["#FF0000", "#00FF00", "#0000FF", "#12345678", "#FFFFFF"] {
            let c = parse_hex_color(hex).unwrap();
            assert_eq!(color_to_hex(&c), hex.to_string());
        }
    }

    #[test]
    fn color_to_hex_omits_alpha_when_opaque() {
        let c = TextRgba {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        };
        assert_eq!(color_to_hex(&c), "#FFFFFF");
        let translucent = TextRgba { a: 0.5, ..c };
        assert_eq!(color_to_hex(&translucent), "#FFFFFF80");
    }

    #[test]
    fn rgba_to_hsla_known_colors() {
        let white = rgba_to_hsla(&TextRgba::default());
        assert!((white.l - 1.0).abs() < 1e-6 && white.s.abs() < 1e-6);

        let black = rgba_to_hsla(&parse_hex_color("#000000").unwrap());
        assert!(black.l.abs() < 1e-6);

        let red = rgba_to_hsla(&parse_hex_color("#FF0000").unwrap());
        assert!(red.h.abs() < 1e-6, "red hue = 0, got {}", red.h);
        assert!((red.s - 1.0).abs() < 1e-6 && (red.l - 0.5).abs() < 1e-6);

        let green = rgba_to_hsla(&parse_hex_color("#00FF00").unwrap());
        assert!((green.h - 1.0 / 3.0).abs() < 1e-6, "green hue = 1/3");

        let blue = rgba_to_hsla(&parse_hex_color("#0000FF").unwrap());
        assert!((blue.h - 2.0 / 3.0).abs() < 1e-6, "blue hue = 2/3");

        let gray = rgba_to_hsla(&parse_hex_color("#808080").unwrap());
        assert!(gray.s.abs() < 1e-6 && (gray.l - 128.0 / 255.0).abs() < 1e-3);
    }

    #[test]
    fn font_families_sorted_and_cover_renderer_supported() {
        assert!(!FONT_FAMILIES.is_empty());
        let mut sorted = FONT_FAMILIES.to_vec();
        sorted.sort();
        assert_eq!(sorted, FONT_FAMILIES, "Swift BundledFonts sorts families");
        for supported in [
            "Anton",
            "Bebas Neue",
            "Permanent Marker",
            "Shrikhand",
            "Basement Grotesque",
            "Poppins",
        ] {
            assert!(
                FONT_FAMILIES.contains(&supported),
                "{supported} missing from FONT_FAMILIES"
            );
        }
    }
}

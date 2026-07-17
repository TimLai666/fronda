//! Design system constants matching Swift `AppTheme.swift`.
//!
//! All UI styling MUST use these constants. Never hardcode numeric values.
//! Values are converted from the Swift AppTheme (NSColor → Hsla, CGFloat → f32/f64).

use gpui::Hsla;

/// Background colors (matching Swift `AppTheme.Background`).
pub struct Background;
impl Background {
    pub const BASE: Hsla = Hsla {
        h: 0.0,
        s: 0.0,
        l: 0.04,
        a: 1.0,
    };
    pub const SURFACE: Hsla = Hsla {
        h: 0.0,
        s: 0.0,
        l: 0.086,
        a: 1.0,
    };
    pub const RAISED: Hsla = Hsla {
        h: 0.0,
        s: 0.0,
        l: 0.118,
        a: 1.0,
    };
    pub const PROMINENT: Hsla = Hsla {
        h: 0.0,
        s: 0.0,
        l: 0.173,
        a: 1.0,
    };
    pub const PLACEHOLDER: Hsla = Self::RAISED;
}

/// Border colors (matching Swift `AppTheme.Border`).
pub struct BorderColors;
impl BorderColors {
    /// white at 16%
    pub const PRIMARY: Hsla = Hsla {
        h: 0.0,
        s: 0.0,
        l: 1.0,
        a: 0.16,
    };
    /// white at 12%
    pub const SUBTLE: Hsla = Hsla {
        h: 0.0,
        s: 0.0,
        l: 1.0,
        a: 0.12,
    };
    /// white at 44%
    pub const DIVIDER: Hsla = Hsla {
        h: 0.0,
        s: 0.0,
        l: 1.0,
        a: 0.44,
    };
    /// Timeline clip outline — black (Swift `Border.timelineClip`, #281).
    pub const TIMELINE_CLIP: Hsla = Hsla {
        h: 0.0,
        s: 0.0,
        l: 0.0,
        a: 1.0,
    };
}

/// Border widths (matching Swift `AppTheme.BorderWidth`).
pub struct BorderWidth;
impl BorderWidth {
    pub const HAIRLINE: f32 = 0.5;
    pub const THIN: f32 = 1.0;
    pub const MEDIUM: f32 = 1.5;
    pub const THICK: f32 = 2.0;
}

/// Text colors (matching Swift `AppTheme.Text`).
pub struct Text;
impl Text {
    pub const PRIMARY: Hsla = Hsla {
        h: 0.0,
        s: 0.0,
        l: 1.0,
        a: 1.0,
    };
    pub const SECONDARY: Hsla = Hsla {
        h: 0.0,
        s: 0.0,
        l: 1.0,
        a: 0.80,
    };
    pub const TERTIARY: Hsla = Hsla {
        h: 0.0,
        s: 0.0,
        l: 1.0,
        a: 0.62,
    };
    pub const MUTED: Hsla = Hsla {
        h: 0.0,
        s: 0.0,
        l: 1.0,
        a: 0.34,
    };
}

/// Opacity values (matching Swift `AppTheme.Opacity`).
pub struct Opacity;
impl Opacity {
    pub const OPAQUE: f32 = 1.0;
    pub const SUBTLE: f32 = 0.04;
    pub const HINT: f32 = 0.06;
    pub const FAINT: f32 = 0.08;
    pub const SOFT: f32 = 0.10;
    pub const MUTED: f32 = 0.15;
    pub const MODERATE: f32 = 0.25;
    pub const MEDIUM: f32 = 0.35;
    pub const STRONG: f32 = 0.55;
    pub const HIGH: f32 = 0.70;
    pub const PROMINENT: f32 = 0.80;
}

/// Exact sRGB hex → Hsla conversion (standard RGB→HSL; h normalized 0..1).
///
/// Palette constants derive from the same hex source of truth as
/// `app_contract::ui_constants::TrackColors` (THM-007) — never eyeball
/// approximate HSL values.
const fn hsla_from_hex(hex: u32) -> Hsla {
    let r = ((hex >> 16) & 0xFF) as f32 / 255.0;
    let g = ((hex >> 8) & 0xFF) as f32 / 255.0;
    let b = (hex & 0xFF) as f32 / 255.0;
    let max = if r >= g && r >= b {
        r
    } else if g >= b {
        g
    } else {
        b
    };
    let min = if r <= g && r <= b {
        r
    } else if g <= b {
        g
    } else {
        b
    };
    let l = (max + min) / 2.0;
    let d = max - min;
    if d == 0.0 {
        return Hsla {
            h: 0.0,
            s: 0.0,
            l,
            a: 1.0,
        };
    }
    let two_l_minus_one = 2.0 * l - 1.0;
    let abs_two_l_minus_one = if two_l_minus_one < 0.0 {
        -two_l_minus_one
    } else {
        two_l_minus_one
    };
    let s = d / (1.0 - abs_two_l_minus_one);
    let h6 = if max == r {
        let v = (g - b) / d;
        if v < 0.0 {
            v + 6.0
        } else {
            v
        }
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    Hsla {
        h: h6 / 6.0,
        s,
        l,
        a: 1.0,
    }
}

/// Track type colors (matching Swift `AppTheme.TrackColor`, upstream #281 palette).
pub struct TrackColor;
impl TrackColor {
    pub const VIDEO: Hsla = hsla_from_hex(0x1D5878);
    pub const AUDIO: Hsla = hsla_from_hex(0x2E7765);
    pub const IMAGE: Hsla = hsla_from_hex(0x715486);
    pub const TEXT: Hsla = Self::IMAGE;
    pub const LOTTIE: Hsla = hsla_from_hex(0xA07822);
    pub const SEQUENCE: Hsla = hsla_from_hex(0xB9B29A);
}

/// Corner radii (matching Swift `AppTheme.Radius`).
pub struct Radius;
impl Radius {
    pub const XS: f32 = 3.0;
    pub const XS_SM: f32 = 4.0;
    pub const SM: f32 = 6.0;
    pub const MD: f32 = 10.0;
    pub const MD_LG: f32 = 12.0;
    pub const LG: f32 = 14.0;
    pub const XL: f32 = 20.0;
}

/// Spacing values (matching Swift `AppTheme.Spacing`).
pub struct Spacing;
impl Spacing {
    pub const XXS: f32 = 2.0;
    pub const XS: f32 = 4.0;
    pub const SM: f32 = 6.0;
    pub const SM_MD: f32 = 8.0;
    pub const MD: f32 = 10.0;
    pub const MD_LG: f32 = 12.0;
    pub const LG: f32 = 14.0;
    pub const LG_XL: f32 = 16.0;
    pub const XL: f32 = 20.0;
    pub const XL_XXL: f32 = 24.0;
    pub const XXL: f32 = 28.0;
}

/// Font sizes (matching Swift `AppTheme.FontSize`).
pub struct FontSize;
impl FontSize {
    pub const MICRO: f32 = 8.0;
    pub const XXS: f32 = 9.0;
    pub const XS: f32 = 10.0;
    pub const SM: f32 = 11.0;
    pub const SM_MD: f32 = 12.0;
    pub const MD: f32 = 13.0;
    pub const MD_LG: f32 = 14.0;
    pub const LG: f32 = 15.0;
    pub const XL: f32 = 18.0;
    pub const TITLE_1: f32 = 22.0;
    pub const TITLE_2: f32 = 28.0;
    pub const DISPLAY: f32 = 36.0;
}

/// Icon frame sizes (matching Swift `AppTheme.IconSize`).
pub struct IconSize;
impl IconSize {
    pub const XXS: f32 = 12.0;
    pub const XS: f32 = 14.0;
    pub const SM: f32 = 18.0;
    pub const SM_MD: f32 = 20.0;
    pub const MD: f32 = 22.0;
    pub const MD_LG: f32 = 24.0;
    pub const LG: f32 = 26.0;
    pub const LG_XL: f32 = 28.0;
    pub const XL: f32 = 30.0;
}

/// Animation durations (matching Swift `AppTheme.Anim`).
pub struct Anim;
impl Anim {
    pub const HOVER: f32 = 0.15;
    pub const TRANSITION: f32 = 0.2;
}

/// Media panel constants (matching Swift `AppTheme.MediaPanel`).
pub struct MediaPanel;
impl MediaPanel {
    pub const TAB_RAIL_WIDTH: f32 = IconSize::LG + Spacing::SM * 2.0;
    pub const CONTEXT_ROW_HEIGHT: f32 = IconSize::MD;
}

/// Accent colors (matching Swift `AppTheme.Accent`).
pub struct Accent;
impl Accent {
    pub const TIMECODE: Hsla = Hsla {
        h: 34.0 / 360.0,
        s: 0.88,
        l: 0.57,
        a: 1.0,
    };
    pub const PRIMARY: Hsla = Hsla {
        h: 36.0 / 360.0,
        s: 0.52,
        l: 0.93,
        a: 1.0,
    };
    pub const SPOTLIGHT: Hsla = Hsla {
        h: 0.0,
        s: 1.0,
        l: 0.63,
        a: 1.0,
    };
}

/// Drop-target highlight (Swift MediaTab `dropHighlight`: accent 0.6 border +
/// subtle accent fill; gpui has no dashed borders so the border is solid).
pub struct DropZone;
impl DropZone {
    pub const BORDER: Hsla = Hsla {
        h: 36.0 / 360.0,
        s: 0.52,
        l: 0.93,
        a: 0.6,
    };
    pub const FILL: Hsla = Hsla {
        h: 36.0 / 360.0,
        s: 0.52,
        l: 0.93,
        a: Opacity::SUBTLE,
    };
}

/// Layout constants (matching Swift `Layout` enum).
pub struct Layout;
impl Layout {
    pub const MEDIA_PANEL_DEFAULT: f32 = 500.0;
    pub const MEDIA_PANEL_MIN: f32 = crate::pane_resize::MEDIA_MIN;
    pub const INSPECTOR_DEFAULT: f32 = 260.0;
    pub const INSPECTOR_MIN: f32 = crate::pane_resize::INSPECTOR_MIN;
    pub const AGENT_PANEL_MIN: f32 = crate::pane_resize::AGENT_MIN;
    pub const AGENT_PANEL_MAX: f32 = crate::pane_resize::AGENT_MAX;
    pub const CHAT_COLUMN_MAX: f32 = 640.0;
    pub const PANEL_HEADER_HEIGHT: f32 = 28.0;
    pub const TOOLBAR_HEIGHT: f32 = 38.0;
    pub const PANEL_GAP: f32 = 5.0;
    pub const TIMELINE_MIN_HEIGHT: f32 = crate::pane_resize::TIMELINE_MIN;
    pub const TIMELINE_MAX_HEIGHT: f32 = crate::pane_resize::TIMELINE_MAX;
    pub const TRACK_HEIGHT: f32 = 50.0;
    pub const RULER_HEIGHT: f32 = 24.0;
    pub const TRACK_HEADER_WIDTH: f32 = 100.0;
    pub const DROP_ZONE_HEIGHT: f32 = 60.0;
    pub const INSERT_THRESHOLD: f32 = 10.0;
    pub const DRAG_THRESHOLD: f32 = 3.0;
    pub const PREVIEW_MIN_WIDTH: f32 = crate::pane_resize::PREVIEW_MIN_W;
    pub const PREVIEW_MIN_HEIGHT: f32 = crate::pane_resize::PREVIEW_MIN_H;
    pub const MEDIA_PANEL_DEFAULT_DEFAULT: f32 = 250.0;
}

/// Shadow constants (matching Swift `AppTheme.Shadow`).
pub struct Shadow;
impl Shadow {
    pub const SM_BLUR: f32 = 1.0;
    pub const SM_Y: f32 = 0.5;
    pub const SM_COLOR_OPACITY: f32 = 0.3;
    pub const MD_BLUR: f32 = 4.0;
    pub const MD_Y: f32 = 2.0;
    pub const MD_COLOR_OPACITY: f32 = 0.3;
    pub const LG_BLUR: f32 = 24.0;
    pub const LG_Y: f32 = 8.0;
    pub const LG_COLOR_OPACITY: f32 = 0.25;
}

/// Status / semantic colors.
pub struct Status;
impl Status {
    /// Error red — h≈0°, s=0.60, l=0.45 (Swift: #E54F4F).
    pub const ERROR: Hsla = Hsla {
        h: 0.0,
        s: 0.60,
        l: 0.45,
        a: 1.0,
    };
}

/// Component size constants (matching Swift `ComponentSize`).
pub struct ComponentSize;
impl ComponentSize {
    pub const PROJECT_CARD_WIDTH: f32 = 150.0;
    pub const PROJECT_CARD_HEIGHT: f32 = 120.0;
    pub const CAPTION_PREVIEW_MAX_HEIGHT: f32 = 150.0;
    pub const TOOL_IMAGE_PREVIEW_MAX_HEIGHT: f32 = 50.0;
    pub const UPDATE_OVERLAY_WIDTH: f32 = 640.0;
    /// Minimum clip width for the black timeline outline (#281).
    pub const TIMELINE_CLIP_BORDER_MIN_WIDTH: f32 = 8.0;
}

/// Window size constants (matching Swift window defaults).
pub struct WindowSize;
impl WindowSize {
    pub const HOME_DEFAULT_W: f32 = 1200.0;
    pub const HOME_DEFAULT_H: f32 = 800.0;
    pub const HOME_MIN_W: f32 = 760.0;
    pub const HOME_MIN_H: f32 = 480.0;
    pub const PROJECT_DEFAULT_W: f32 = 1600.0;
    pub const PROJECT_DEFAULT_H: f32 = 1000.0;
    pub const PROJECT_MIN_W: f32 = 960.0;
    pub const PROJECT_MIN_H: f32 = 600.0;
    pub const PROJECT_TITLEBAR_TRAILING_WIDTH: f32 = 280.0;
}

/// Letter-spacing constants (matching Swift `AppTheme.Tracking`).
///
/// Values are approximate point-per-em equivalents; gpui exposes letter spacing
/// via `Styled::letter_spacing(px(f32))` where positive = wider, negative = tighter.
pub struct Tracking;
impl Tracking {
    /// Tight — used in welcome title (Swift: AppTheme.Tracking.tight ≈ -0.5pt).
    pub const TIGHT: f32 = -0.5;
    /// Wide — used in section headers (Swift: AppTheme.Tracking.wide ≈ 1.5pt).
    pub const WIDE: f32 = 1.5;
    /// Normal — no letter spacing adjustment (default).
    pub const NORMAL: f32 = 0.0;
}

/// Generation panel size constants (matching Swift `GenerationPanel`).
pub struct GenerationPanel;
impl GenerationPanel {
    pub const MEDIA_AREA_MIN_HEIGHT: f32 = 120.0;
    pub const LOADING_HEIGHT: f32 = 180.0;
    pub const PROMPT_MIN_HEIGHT: f32 = 40.0;
    pub const REFERENCE_TILE_WIDTH: f32 = 80.0;
    pub const REFERENCE_TILE_HEIGHT: f32 = 56.0;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn background_colors_distinct() {
        assert!(Background::BASE != Background::SURFACE);
        assert!(Background::SURFACE != Background::RAISED);
        assert!(Background::RAISED != Background::PROMINENT);
    }

    #[test]
    fn border_colors_alpha_order() {
        // PRIMARY (0.16) > SUBTLE (0.12)
        assert!(BorderColors::PRIMARY.a > BorderColors::SUBTLE.a);
    }

    #[test]
    fn text_colors_decrease_alpha() {
        assert!(Text::PRIMARY.a > Text::SECONDARY.a);
        assert!(Text::SECONDARY.a > Text::TERTIARY.a);
        assert!(Text::TERTIARY.a > Text::MUTED.a);
    }

    #[test]
    fn spacing_monotonic() {
        let vals = [
            Spacing::XXS,
            Spacing::XS,
            Spacing::SM,
            Spacing::SM_MD,
            Spacing::MD,
            Spacing::MD_LG,
            Spacing::LG,
            Spacing::LG_XL,
            Spacing::XL,
            Spacing::XL_XXL,
            Spacing::XXL,
        ];
        for i in 1..vals.len() {
            assert!(
                vals[i] > vals[i - 1],
                "spacing[{}] must be > [{}]",
                i,
                i - 1
            );
        }
    }

    #[test]
    fn font_sizes_monotonic() {
        let vals = [
            FontSize::MICRO,
            FontSize::XXS,
            FontSize::XS,
            FontSize::SM,
            FontSize::SM_MD,
            FontSize::MD,
            FontSize::MD_LG,
            FontSize::LG,
            FontSize::XL,
            FontSize::TITLE_1,
            FontSize::TITLE_2,
            FontSize::DISPLAY,
        ];
        for i in 1..vals.len() {
            assert!(vals[i] > vals[i - 1], "font[{}] must be > [{}]", i, i - 1);
        }
    }

    #[test]
    fn icon_sizes_monotonic() {
        let vals = [
            IconSize::XXS,
            IconSize::XS,
            IconSize::SM,
            IconSize::SM_MD,
            IconSize::MD,
            IconSize::MD_LG,
            IconSize::LG,
            IconSize::LG_XL,
            IconSize::XL,
        ];
        for i in 1..vals.len() {
            assert!(vals[i] > vals[i - 1], "icon[{}] must be > [{}]", i, i - 1);
        }
    }

    #[test]
    fn track_colors_defined() {
        // Just verify they compile and are opaque
        assert!((TrackColor::VIDEO.a - 1.0).abs() < 1e-6);
        assert!((TrackColor::AUDIO.a - 1.0).abs() < 1e-6);
        assert!((TrackColor::IMAGE.a - 1.0).abs() < 1e-6);
        assert!((TrackColor::TEXT.a - 1.0).abs() < 1e-6);
        assert!((TrackColor::LOTTIE.a - 1.0).abs() < 1e-6);
        assert!((TrackColor::SEQUENCE.a - 1.0).abs() < 1e-6);
    }

    fn assert_hsla(c: Hsla, h: f32, s: f32, l: f32) {
        assert!((c.h - h).abs() < 1e-6, "h {} != {}", c.h, h);
        assert!((c.s - s).abs() < 1e-6, "s {} != {}", c.s, s);
        assert!((c.l - l).abs() < 1e-6, "l {} != {}", c.l, l);
        assert!((c.a - 1.0).abs() < 1e-6);
    }

    #[test]
    fn hsla_from_hex_known_values() {
        assert_hsla(hsla_from_hex(0xFFFFFF), 0.0, 0.0, 1.0);
        assert_hsla(hsla_from_hex(0x000000), 0.0, 0.0, 0.0);
        assert_hsla(hsla_from_hex(0xFF0000), 0.0, 1.0, 0.5);
        assert_hsla(hsla_from_hex(0x00FF00), 1.0 / 3.0, 1.0, 0.5);
        assert_hsla(hsla_from_hex(0x0000FF), 2.0 / 3.0, 1.0, 0.5);
    }

    // THM-007: #281 palette, HSL independently derived from the hex values.
    #[test]
    fn track_colors_match_281_hex_palette() {
        // #1D5878
        assert_hsla(TrackColor::VIDEO, 0.558608059, 0.610738255, 0.292156863);
        // #2E7765
        assert_hsla(TrackColor::AUDIO, 0.458904110, 0.442424242, 0.323529412);
        // #715486
        assert_hsla(TrackColor::IMAGE, 0.763333333, 0.229357798, 0.427450980);
        // #715486 (text aliases image)
        assert_hsla(TrackColor::TEXT, 0.763333333, 0.229357798, 0.427450980);
        // #A07822
        assert_hsla(TrackColor::LOTTIE, 0.113756614, 0.649484536, 0.380392157);
        // #B9B29A
        assert_hsla(TrackColor::SEQUENCE, 0.129032258, 0.181286550, 0.664705882);
    }

    // #281 tokens: Border.timelineClip (black), Opacity.high, min border width.
    #[test]
    fn timeline_clip_tokens_281() {
        assert_hsla(BorderColors::TIMELINE_CLIP, 0.0, 0.0, 0.0);
        assert!((Opacity::HIGH - 0.70).abs() < 1e-6);
        assert!(Opacity::STRONG < Opacity::HIGH && Opacity::HIGH < Opacity::PROMINENT);
        assert_eq!(ComponentSize::TIMELINE_CLIP_BORDER_MIN_WIDTH, 8.0);
    }

    #[test]
    fn media_panel_tab_rail_width_formula() {
        let expected = IconSize::LG + Spacing::SM * 2.0;
        assert!((MediaPanel::TAB_RAIL_WIDTH - expected).abs() < 1e-6);
    }

    #[test]
    fn layout_constants_positive() {
        assert!(Layout::MEDIA_PANEL_DEFAULT > 0.0);
        assert!(Layout::INSPECTOR_DEFAULT > 0.0);
        assert!(Layout::TOOLBAR_HEIGHT > 0.0);
        assert!(Layout::PANEL_GAP > 0.0);
    }
}

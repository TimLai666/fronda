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
    pub const PROMINENT: f32 = 0.80;
}

/// Track type colors (matching Swift `AppTheme.TrackColor`).
pub struct TrackColor;
impl TrackColor {
    pub const VIDEO: Hsla = Hsla {
        h: 196.0 / 360.0,
        s: 1.0,
        l: 0.38,
        a: 1.0,
    };
    pub const AUDIO: Hsla = Hsla {
        h: 100.0 / 360.0,
        s: 0.66,
        l: 0.40,
        a: 1.0,
    };
    pub const IMAGE: Hsla = Hsla {
        h: 288.0 / 360.0,
        s: 0.65,
        l: 0.50,
        a: 1.0,
    };
    pub const TEXT: Hsla = Self::IMAGE;
    pub const LOTTIE: Hsla = Hsla {
        h: 42.0 / 360.0,
        s: 1.0,
        l: 0.44,
        a: 1.0,
    };
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

/// Layout constants (matching Swift `Layout` enum).
pub struct Layout;
impl Layout {
    pub const MEDIA_PANEL_DEFAULT: f32 = 500.0;
    pub const MEDIA_PANEL_MIN: f32 = 280.0;
    pub const INSPECTOR_DEFAULT: f32 = 260.0;
    pub const INSPECTOR_MIN: f32 = 150.0;
    pub const AGENT_PANEL_MIN: f32 = 240.0;
    pub const AGENT_PANEL_MAX: f32 = 640.0;
    pub const CHAT_COLUMN_MAX: f32 = 640.0;
    pub const PANEL_HEADER_HEIGHT: f32 = 28.0;
    pub const TOOLBAR_HEIGHT: f32 = 38.0;
    pub const PANEL_GAP: f32 = 5.0;
    pub const TIMELINE_MIN_HEIGHT: f32 = 100.0;
    pub const TIMELINE_MAX_HEIGHT: f32 = 700.0;
    pub const TRACK_HEIGHT: f32 = 50.0;
    pub const RULER_HEIGHT: f32 = 24.0;
    pub const TRACK_HEADER_WIDTH: f32 = 100.0;
    pub const DROP_ZONE_HEIGHT: f32 = 60.0;
    pub const INSERT_THRESHOLD: f32 = 10.0;
    pub const DRAG_THRESHOLD: f32 = 3.0;
    pub const PREVIEW_MIN_WIDTH: f32 = 400.0;
    pub const PREVIEW_MIN_HEIGHT: f32 = 320.0;
    pub const MEDIA_PANEL_DEFAULT_DEFAULT: f32 = 250.0;
}

/// Shadow constants (matching Swift `AppTheme.Shadow`).
pub struct Shadow;
impl Shadow {
    pub const SM_BLUR: f32 = 1.0;
    pub const MD_BLUR: f32 = 4.0;
    pub const LG_BLUR: f32 = 24.0;
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

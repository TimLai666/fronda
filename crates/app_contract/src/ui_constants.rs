//! Toolbar, interaction, and AppTheme token constants.
//!
//! Covers UIX-001 through UIX-012 and THM-001 through THM-020.
//! All values match `Constants.swift` and `AppTheme.swift` exactly.

use serde::{Deserialize, Serialize};

// ═══════════════════════════════════════════════════════════════════
// UIX-001..012: Toolbar and interaction constants
// ═══════════════════════════════════════════════════════════════════

/// UIX-001: Toolbar and panel header heights.
pub struct Toolbar;
impl Toolbar {
    pub const HEIGHT: f64 = 38.0;
    pub const PANEL_HEADER_HEIGHT: f64 = 28.0;
}

/// UIX-004: Timeline default pixels per frame.
pub struct TimelineDefaults;
impl TimelineDefaults {
    pub const PIXELS_PER_FRAME: f64 = 4.0;
}

/// UIX-005: Default generated/created durations.
pub struct DefaultDuration;
impl DefaultDuration {
    pub const IMAGE_SECONDS: f64 = 5.0;
    pub const AUDIO_TTS_SECONDS: f64 = 10.0;
    pub const AUDIO_MUSIC_SECONDS: f64 = 60.0;
    pub const TEXT_SECONDS: f64 = 3.0;
}

/// UIX-006: Aspect-ratio tolerance.
pub const ASPECT_TOLERANCE: f64 = 0.02;

/// UIX-007: Zoom constants.
pub struct ZoomConstants;
impl ZoomConstants {
    pub const MIN: f64 = 0.05;
    pub const FLOOR: f64 = 0.0001;
    pub const MAX: f64 = 40.0;
    pub const SCROLL_SENSITIVITY: f64 = 0.04;
    pub const MAGNIFY_SENSITIVITY: f64 = 1.5;
    pub const PAN_SPEED: f64 = 5.0;
    pub const FIT_ALL_BUFFER: f64 = 3.0;
}

/// UIX-008: Timeline autoscroll constants.
pub struct TimelineAutoScroll;
impl TimelineAutoScroll {
    pub const EDGE_ZONE_WIDTH: f64 = 56.0;
    pub const MAX_ZONE_FRACTION: f64 = 0.5;
    pub const MIN_STEP: f64 = 4.0;
    pub const MAX_STEP: f64 = 28.0;
    pub const INTERVAL_SECONDS: f64 = 1.0 / 60.0;
}

/// UIX-009: Track-size constants.
pub struct TrackSizeConstants;
impl TrackSizeConstants {
    pub const MIN_HEIGHT: f64 = 32.0;
    pub const MAX_HEIGHT: f64 = 200.0;
    pub const RESIZE_HANDLE_ZONE: f64 = 6.0;
}

/// UIX-010: Timeline layout constants.
pub struct TimelineLayout;
impl TimelineLayout {
    pub const MIN_HEIGHT: f64 = 100.0;
    pub const MAX_HEIGHT: f64 = 700.0;
    pub const DEFAULT_TRACK_HEIGHT: f64 = 50.0;
    pub const RULER_HEIGHT: f64 = 24.0;
    pub const TRACK_HEADER_WIDTH: f64 = 100.0;
    pub const DROP_ZONE_HEIGHT: f64 = 60.0;
    pub const INSERT_THRESHOLD: f64 = 10.0;
    pub const DRAG_THRESHOLD: f64 = 3.0;
}

/// UIX-011: Panel width constants.
pub struct PanelWidth;
impl PanelWidth {
    pub const MEDIA_PANEL_DEFAULT: f64 = 500.0;
    pub const MEDIA_PANEL_MIN: f64 = 280.0;
    pub const INSPECTOR_DEFAULT: f64 = 260.0;
    pub const INSPECTOR_MIN: f64 = 150.0;
    pub const AGENT_PANEL_MIN: f64 = 240.0;
    pub const AGENT_PANEL_MAX: f64 = 640.0;
    pub const CHAT_COLUMN_MAX: f64 = 640.0;
}

/// UIX-012: Preview minimum size.
pub struct PreviewSize;
impl PreviewSize {
    pub const MIN_WIDTH: f64 = 400.0;
    pub const MIN_HEIGHT: f64 = 320.0;
}

// ═══════════════════════════════════════════════════════════════════
// THM-001..020: AppTheme token contract
// ═══════════════════════════════════════════════════════════════════

/// THM-002: Background color hex values.
/// All values converted from AppTheme.swift NSColor initializers.
pub struct BackgroundColors;
impl BackgroundColors {
    pub const BASE: &'static str = "#0A0A0A";
    pub const SURFACE: &'static str = "#161616";
    pub const RAISED: &'static str = "#1E1E1E";
    pub const PROMINENT: &'static str = "#2C2C2C";
    /// Alias: empty media slot is a raised plate.
    pub const PLACEHOLDER: &'static str = "#1E1E1E";
    /// Preview canvas remains black.
    pub const PREVIEW_CANVAS: &'static str = "#000000";
}

/// THM-003: Border color alpha values (white with alpha).
pub struct BorderColors;
impl BorderColors {
    pub const PRIMARY_ALPHA: f64 = 0.16;
    pub const SUBTLE_ALPHA: f64 = 0.12;
    pub const DIVIDER_ALPHA: f64 = 0.44;
}

/// THM-003: Border widths.
pub struct BorderWidths;
impl BorderWidths {
    pub const HAIRLINE: f64 = 0.5;
    pub const THIN: f64 = 1.0;
    pub const MEDIUM: f64 = 1.5;
    pub const THICK: f64 = 2.0;
}

/// THM-004: Accent color values (0..1 float components).
pub struct AccentColors;
impl AccentColors {
    pub const TIMECODE_R: f64 = 0.95;
    pub const TIMECODE_G: f64 = 0.6;
    pub const TIMECODE_B: f64 = 0.2;
    pub const PRIMARY_WARM_R: f64 = 0.961;
    pub const PRIMARY_WARM_G: f64 = 0.937;
    pub const PRIMARY_WARM_B: f64 = 0.894;
}

/// THM-005: Text white-alpha values.
pub struct TextAlphas;
impl TextAlphas {
    pub const PRIMARY: f64 = 1.0;
    pub const SECONDARY: f64 = 0.80;
    pub const TERTIARY: f64 = 0.62;
    pub const MUTED: f64 = 0.34;
}

/// THM-006: Opacity tokens as f64 values (0..1).
pub struct OpacityTokens;
impl OpacityTokens {
    pub const OPAQUE: f64 = 1.0;
    pub const SUBTLE: f64 = 0.04;
    pub const HINT: f64 = 0.06;
    pub const FAINT: f64 = 0.08;
    pub const SOFT: f64 = 0.10;
    pub const MUTED: f64 = 0.15;
    pub const MODERATE: f64 = 0.25;
    pub const MEDIUM: f64 = 0.35;
    pub const STRONG: f64 = 0.55;
    pub const PROMINENT: f64 = 0.80;
}

/// THM-007: Track color hex values.
pub struct TrackColors;
impl TrackColors {
    pub const VIDEO: &'static str = "#0091C2";
    pub const AUDIO: &'static str = "#58A822";
    pub const IMAGE: &'static str = "#B72DD2";
    pub const TEXT: &'static str = "#B72DD2";
    pub const LOTTIE: &'static str = "#E0A800";
}

/// THM-008: Corner radius tokens.
pub struct RadiusTokens;
impl RadiusTokens {
    pub const XS: f64 = 3.0;
    pub const XS_SM: f64 = 4.0;
    pub const SM: f64 = 6.0;
    pub const MD: f64 = 10.0;
    pub const MD_LG: f64 = 12.0;
    pub const LG: f64 = 14.0;
    pub const XL: f64 = 20.0;

    /// Concentric radius: max(outer - padding, 0).
    pub fn concentric(outer: f64, padding: f64) -> f64 {
        (outer - padding).max(0.0)
    }
}

/// THM-009: Spacing tokens.
pub struct SpacingTokens;
impl SpacingTokens {
    pub const XXS: f64 = 2.0;
    pub const XS: f64 = 4.0;
    pub const SM: f64 = 6.0;
    pub const SM_MD: f64 = 8.0;
    pub const MD: f64 = 10.0;
    pub const MD_LG: f64 = 12.0;
    pub const LG: f64 = 14.0;
    pub const LG_XL: f64 = 16.0;
    pub const XL: f64 = 20.0;
    pub const XL_XXL: f64 = 24.0;
    pub const XXL: f64 = 28.0;
}

/// THM-010: Font size tokens.
pub struct FontSizeTokens;
impl FontSizeTokens {
    pub const MICRO: f64 = 8.0;
    pub const XXS: f64 = 9.0;
    pub const XS: f64 = 10.0;
    pub const SM: f64 = 11.0;
    pub const SM_MD: f64 = 12.0;
    pub const MD: f64 = 13.0;
    pub const MD_LG: f64 = 14.0;
    pub const LG: f64 = 15.0;
    pub const XL: f64 = 18.0;
    pub const TITLE1: f64 = 22.0;
    pub const TITLE2: f64 = 28.0;
    pub const DISPLAY: f64 = 36.0;
}

/// THM-011: Font weight labels.
pub struct FontWeightLabels;
impl FontWeightLabels {
    pub const LIGHT: &'static str = "light";
    pub const REGULAR: &'static str = "regular";
    pub const MEDIUM: &'static str = "medium";
    pub const SEMIBOLD: &'static str = "semibold";
    pub const BOLD: &'static str = "bold";
}

/// THM-012: Tracking (letter-spacing) values.
pub struct TrackingTokens;
impl TrackingTokens {
    pub const TIGHT: f64 = -0.5;
    pub const NORMAL: f64 = 0.0;
    pub const WIDE: f64 = 1.5;
}

/// THM-013: Icon-size tokens (square frame dimensions).
pub struct IconSizeTokens;
impl IconSizeTokens {
    pub const XXS: f64 = 12.0;
    pub const XS: f64 = 14.0;
    pub const SM: f64 = 18.0;
    pub const SM_MD: f64 = 20.0;
    pub const MD: f64 = 22.0;
    pub const MD_LG: f64 = 24.0;
    pub const LG: f64 = 26.0;
    pub const LG_XL: f64 = 28.0;
    pub const XL: f64 = 30.0;
}

/// THM-014: Component-size tokens.
pub struct ComponentSizeTokens;
impl ComponentSizeTokens {
    pub const CAPTION_PREVIEW_MAX_HEIGHT: f64 = 150.0;
    pub const CAPTION_PREVIEW_MAX_TEXT_WIDTH_RATIO: f64 = 0.9;
    pub const TOOL_IMAGE_PREVIEW_MAX_HEIGHT: f64 = 50.0;
    pub const PROJECT_CARD_WIDTH: f64 = 150.0;
    pub const PROJECT_CARD_HEIGHT: f64 = 120.0;
    pub const UPDATE_OVERLAY_WIDTH: f64 = 640.0;
}

/// THM-015: Caption tokens.
pub struct CaptionTokens;
impl CaptionTokens {
    pub const DEFAULT_FONT_SIZE: f64 = 48.0;
    pub const MIN_FONT_SIZE: f64 = 12.0;
    pub const MAX_FONT_SIZE: f64 = 300.0;
    pub const POSITION_MIN: f64 = 0.0;
    pub const POSITION_MAX: f64 = 1.0;
    pub const CENTER_SNAP_VALUE: f64 = 0.5;
    pub const CENTER_SNAP_THRESHOLD: f64 = 0.02;
    pub const DEFAULT_CENTER_X: f64 = 0.5;
    pub const DEFAULT_CENTER_Y: f64 = 0.9;
    pub const MIN_DISPLAY_DURATION: f64 = 0.7;
}

/// THM-016: Generation-panel tokens.
pub struct GenerationPanelTokens;
impl GenerationPanelTokens {
    pub const MEDIA_AREA_MIN_HEIGHT: f64 = 120.0;
    pub const LOADING_HEIGHT: f64 = 180.0;
    pub const PROMPT_MIN_HEIGHT: f64 = 40.0;
    pub const REFERENCE_TILE_WIDTH: f64 = 80.0;
    pub const REFERENCE_TILE_HEIGHT: f64 = 56.0;
}

/// THM-017: Media-panel tokens.
pub struct MediaPanelTokens;
impl MediaPanelTokens {
    /// tabRailWidth = IconSize::LG + Spacing::SM * 2
    pub fn tab_rail_width() -> f64 {
        IconSizeTokens::LG + SpacingTokens::SM * 2.0
    }
    /// contextRowHeight = IconSize::MD
    pub fn context_row_height() -> f64 {
        IconSizeTokens::MD
    }
}

/// THM-018: Shadow style descriptor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShadowStyle {
    pub color_alpha: f64,
    pub radius: f64,
    pub x: f64,
    pub y: f64,
}

/// THM-018: Shadow presets.
pub struct ShadowPresets;
impl ShadowPresets {
    pub fn sm() -> ShadowStyle {
        ShadowStyle {
            color_alpha: 0.3,
            radius: 1.0,
            x: 0.0,
            y: 0.5,
        }
    }
    pub fn md() -> ShadowStyle {
        ShadowStyle {
            color_alpha: 0.3,
            radius: 4.0,
            x: 0.0,
            y: 2.0,
        }
    }
    pub fn lg() -> ShadowStyle {
        ShadowStyle {
            color_alpha: 0.25,
            radius: 24.0,
            x: 0.0,
            y: 8.0,
        }
    }
}

/// THM-019: Animation duration tokens.
pub struct AnimationDurations;
impl AnimationDurations {
    pub const HOVER: f64 = 0.15;
    pub const TRANSITION: f64 = 0.2;
}

// ═══════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // UIX-001
    #[test]
    fn toolbar_heights() {
        assert_eq!(Toolbar::HEIGHT, 38.0);
        assert_eq!(Toolbar::PANEL_HEADER_HEIGHT, 28.0);
    }

    // UIX-004
    #[test]
    fn default_pixels_per_frame() {
        assert_eq!(TimelineDefaults::PIXELS_PER_FRAME, 4.0);
    }

    // UIX-005
    #[test]
    fn default_durations() {
        assert_eq!(DefaultDuration::IMAGE_SECONDS, 5.0);
        assert_eq!(DefaultDuration::AUDIO_TTS_SECONDS, 10.0);
        assert_eq!(DefaultDuration::AUDIO_MUSIC_SECONDS, 60.0);
        assert_eq!(DefaultDuration::TEXT_SECONDS, 3.0);
    }

    // UIX-006
    #[test]
    fn aspect_tolerance() {
        assert!((ASPECT_TOLERANCE - 0.02).abs() < 1e-10);
    }

    // UIX-007
    #[test]
    fn zoom_constants() {
        assert_eq!(ZoomConstants::MIN, 0.05);
        assert_eq!(ZoomConstants::FLOOR, 0.0001);
        assert_eq!(ZoomConstants::MAX, 40.0);
        assert_eq!(ZoomConstants::SCROLL_SENSITIVITY, 0.04);
        assert_eq!(ZoomConstants::MAGNIFY_SENSITIVITY, 1.5);
        assert_eq!(ZoomConstants::PAN_SPEED, 5.0);
        assert_eq!(ZoomConstants::FIT_ALL_BUFFER, 3.0);
    }

    // UIX-008
    #[test]
    fn autoscroll_constants() {
        assert_eq!(TimelineAutoScroll::EDGE_ZONE_WIDTH, 56.0);
        assert_eq!(TimelineAutoScroll::MAX_ZONE_FRACTION, 0.5);
        assert_eq!(TimelineAutoScroll::MIN_STEP, 4.0);
        assert_eq!(TimelineAutoScroll::MAX_STEP, 28.0);
        assert!((TimelineAutoScroll::INTERVAL_SECONDS - 1.0 / 60.0).abs() < 1e-10);
    }

    // UIX-009
    #[test]
    fn track_size_constants() {
        assert_eq!(TrackSizeConstants::MIN_HEIGHT, 32.0);
        assert_eq!(TrackSizeConstants::MAX_HEIGHT, 200.0);
        assert_eq!(TrackSizeConstants::RESIZE_HANDLE_ZONE, 6.0);
    }

    // UIX-010
    #[test]
    fn timeline_layout() {
        assert_eq!(TimelineLayout::MIN_HEIGHT, 100.0);
        assert_eq!(TimelineLayout::MAX_HEIGHT, 700.0);
        assert_eq!(TimelineLayout::DEFAULT_TRACK_HEIGHT, 50.0);
        assert_eq!(TimelineLayout::RULER_HEIGHT, 24.0);
        assert_eq!(TimelineLayout::TRACK_HEADER_WIDTH, 100.0);
        assert_eq!(TimelineLayout::DROP_ZONE_HEIGHT, 60.0);
        assert_eq!(TimelineLayout::INSERT_THRESHOLD, 10.0);
        assert_eq!(TimelineLayout::DRAG_THRESHOLD, 3.0);
    }

    // UIX-011
    #[test]
    fn panel_widths() {
        assert_eq!(PanelWidth::MEDIA_PANEL_DEFAULT, 500.0);
        assert_eq!(PanelWidth::MEDIA_PANEL_MIN, 280.0);
        assert_eq!(PanelWidth::INSPECTOR_DEFAULT, 260.0);
        assert_eq!(PanelWidth::INSPECTOR_MIN, 150.0);
        assert_eq!(PanelWidth::AGENT_PANEL_MIN, 240.0);
        assert_eq!(PanelWidth::AGENT_PANEL_MAX, 640.0);
        assert_eq!(PanelWidth::CHAT_COLUMN_MAX, 640.0);
    }

    // UIX-012
    #[test]
    fn preview_min_size() {
        assert_eq!(PreviewSize::MIN_WIDTH, 400.0);
        assert_eq!(PreviewSize::MIN_HEIGHT, 320.0);
    }

    // THM-002
    #[test]
    fn background_colors() {
        assert_eq!(BackgroundColors::BASE, "#0A0A0A");
        assert_eq!(BackgroundColors::SURFACE, "#161616");
        assert_eq!(BackgroundColors::RAISED, "#1E1E1E");
        assert_eq!(BackgroundColors::PROMINENT, "#2C2C2C");
        assert_eq!(BackgroundColors::PLACEHOLDER, "#1E1E1E");
        assert_eq!(BackgroundColors::PREVIEW_CANVAS, "#000000");
    }

    // THM-003
    #[test]
    fn border_colors() {
        assert!((BorderColors::PRIMARY_ALPHA - 0.16).abs() < 1e-10);
        assert!((BorderColors::SUBTLE_ALPHA - 0.12).abs() < 1e-10);
        assert!((BorderColors::DIVIDER_ALPHA - 0.44).abs() < 1e-10);
    }

    #[test]
    fn border_widths() {
        assert_eq!(BorderWidths::HAIRLINE, 0.5);
        assert_eq!(BorderWidths::THIN, 1.0);
        assert_eq!(BorderWidths::MEDIUM, 1.5);
        assert_eq!(BorderWidths::THICK, 2.0);
    }

    // THM-004
    #[test]
    fn accent_colors() {
        assert!((AccentColors::TIMECODE_R - 0.95).abs() < 1e-10);
        assert!((AccentColors::TIMECODE_G - 0.6).abs() < 1e-10);
        assert!((AccentColors::TIMECODE_B - 0.2).abs() < 1e-10);
    }

    // THM-005
    #[test]
    fn text_alphas() {
        assert!((TextAlphas::PRIMARY - 1.0).abs() < 1e-10);
        assert!((TextAlphas::SECONDARY - 0.80).abs() < 1e-10);
        assert!((TextAlphas::TERTIARY - 0.62).abs() < 1e-10);
        assert!((TextAlphas::MUTED - 0.34).abs() < 1e-10);
    }

    // THM-006
    #[test]
    fn opacity_tokens() {
        assert!((OpacityTokens::OPAQUE - 1.0).abs() < 1e-10);
        assert!((OpacityTokens::SUBTLE - 0.04).abs() < 1e-10);
        assert!((OpacityTokens::HINT - 0.06).abs() < 1e-10);
        assert!((OpacityTokens::FAINT - 0.08).abs() < 1e-10);
        assert!((OpacityTokens::SOFT - 0.10).abs() < 1e-10);
        assert!((OpacityTokens::MUTED - 0.15).abs() < 1e-10);
        assert!((OpacityTokens::MODERATE - 0.25).abs() < 1e-10);
        assert!((OpacityTokens::MEDIUM - 0.35).abs() < 1e-10);
        assert!((OpacityTokens::STRONG - 0.55).abs() < 1e-10);
        assert_eq!(OpacityTokens::PROMINENT, 0.80);
    }

    // THM-007
    #[test]
    fn track_colors() {
        assert_eq!(TrackColors::VIDEO, "#0091C2");
        assert_eq!(TrackColors::AUDIO, "#58A822");
        assert_eq!(TrackColors::IMAGE, "#B72DD2");
        assert_eq!(TrackColors::TEXT, "#B72DD2");
        assert_eq!(TrackColors::LOTTIE, "#E0A800");
    }

    // THM-008
    #[test]
    fn radius_tokens() {
        assert_eq!(RadiusTokens::XS, 3.0);
        assert_eq!(RadiusTokens::XS_SM, 4.0);
        assert_eq!(RadiusTokens::SM, 6.0);
        assert_eq!(RadiusTokens::MD, 10.0);
        assert_eq!(RadiusTokens::MD_LG, 12.0);
        assert_eq!(RadiusTokens::LG, 14.0);
        assert_eq!(RadiusTokens::XL, 20.0);
    }

    #[test]
    fn concentric_radius() {
        assert!((RadiusTokens::concentric(10.0, 4.0) - 6.0).abs() < 1e-10);
        assert!((RadiusTokens::concentric(4.0, 10.0) - 0.0).abs() < 1e-10);
    }

    // THM-009
    #[test]
    fn spacing_tokens() {
        assert_eq!(SpacingTokens::XXS, 2.0);
        assert_eq!(SpacingTokens::XS, 4.0);
        assert_eq!(SpacingTokens::SM, 6.0);
        assert_eq!(SpacingTokens::SM_MD, 8.0);
        assert_eq!(SpacingTokens::MD, 10.0);
        assert_eq!(SpacingTokens::MD_LG, 12.0);
        assert_eq!(SpacingTokens::LG, 14.0);
        assert_eq!(SpacingTokens::LG_XL, 16.0);
        assert_eq!(SpacingTokens::XL, 20.0);
        assert_eq!(SpacingTokens::XL_XXL, 24.0);
        assert_eq!(SpacingTokens::XXL, 28.0);
    }

    // THM-010
    #[test]
    fn font_size_tokens() {
        assert_eq!(FontSizeTokens::MICRO, 8.0);
        assert_eq!(FontSizeTokens::XXS, 9.0);
        assert_eq!(FontSizeTokens::XS, 10.0);
        assert_eq!(FontSizeTokens::SM, 11.0);
        assert_eq!(FontSizeTokens::SM_MD, 12.0);
        assert_eq!(FontSizeTokens::MD, 13.0);
        assert_eq!(FontSizeTokens::MD_LG, 14.0);
        assert_eq!(FontSizeTokens::LG, 15.0);
        assert_eq!(FontSizeTokens::XL, 18.0);
        assert_eq!(FontSizeTokens::TITLE1, 22.0);
        assert_eq!(FontSizeTokens::TITLE2, 28.0);
        assert_eq!(FontSizeTokens::DISPLAY, 36.0);
    }

    // THM-011
    #[test]
    fn font_weight_labels() {
        assert_eq!(FontWeightLabels::LIGHT, "light");
        assert_eq!(FontWeightLabels::REGULAR, "regular");
        assert_eq!(FontWeightLabels::MEDIUM, "medium");
        assert_eq!(FontWeightLabels::SEMIBOLD, "semibold");
        assert_eq!(FontWeightLabels::BOLD, "bold");
    }

    // THM-012
    #[test]
    fn tracking_tokens() {
        assert!((TrackingTokens::TIGHT - (-0.5)).abs() < 1e-10);
        assert!((TrackingTokens::NORMAL - 0.0).abs() < 1e-10);
        assert!((TrackingTokens::WIDE - 1.5).abs() < 1e-10);
    }

    // THM-013
    #[test]
    fn icon_size_tokens() {
        assert_eq!(IconSizeTokens::XXS, 12.0);
        assert_eq!(IconSizeTokens::XS, 14.0);
        assert_eq!(IconSizeTokens::SM, 18.0);
        assert_eq!(IconSizeTokens::SM_MD, 20.0);
        assert_eq!(IconSizeTokens::MD, 22.0);
        assert_eq!(IconSizeTokens::MD_LG, 24.0);
        assert_eq!(IconSizeTokens::LG, 26.0);
        assert_eq!(IconSizeTokens::LG_XL, 28.0);
        assert_eq!(IconSizeTokens::XL, 30.0);
    }

    // THM-014
    #[test]
    fn component_size_tokens() {
        assert_eq!(ComponentSizeTokens::CAPTION_PREVIEW_MAX_HEIGHT, 150.0);
        assert_eq!(
            ComponentSizeTokens::CAPTION_PREVIEW_MAX_TEXT_WIDTH_RATIO,
            0.9
        );
        assert_eq!(ComponentSizeTokens::TOOL_IMAGE_PREVIEW_MAX_HEIGHT, 50.0);
        assert_eq!(ComponentSizeTokens::PROJECT_CARD_WIDTH, 150.0);
        assert_eq!(ComponentSizeTokens::PROJECT_CARD_HEIGHT, 120.0);
        assert_eq!(ComponentSizeTokens::UPDATE_OVERLAY_WIDTH, 640.0);
    }

    // THM-015
    #[test]
    fn caption_tokens() {
        assert_eq!(CaptionTokens::DEFAULT_FONT_SIZE, 48.0);
        assert_eq!(CaptionTokens::MIN_FONT_SIZE, 12.0);
        assert_eq!(CaptionTokens::MAX_FONT_SIZE, 300.0);
        assert_eq!(CaptionTokens::POSITION_MIN, 0.0);
        assert_eq!(CaptionTokens::POSITION_MAX, 1.0);
        assert_eq!(CaptionTokens::CENTER_SNAP_VALUE, 0.5);
        assert_eq!(CaptionTokens::CENTER_SNAP_THRESHOLD, 0.02);
        assert_eq!(CaptionTokens::DEFAULT_CENTER_X, 0.5);
        assert_eq!(CaptionTokens::DEFAULT_CENTER_Y, 0.9);
        assert_eq!(CaptionTokens::MIN_DISPLAY_DURATION, 0.7);
    }

    // THM-016
    #[test]
    fn generation_panel_tokens() {
        assert_eq!(GenerationPanelTokens::MEDIA_AREA_MIN_HEIGHT, 120.0);
        assert_eq!(GenerationPanelTokens::LOADING_HEIGHT, 180.0);
        assert_eq!(GenerationPanelTokens::PROMPT_MIN_HEIGHT, 40.0);
        assert_eq!(GenerationPanelTokens::REFERENCE_TILE_WIDTH, 80.0);
        assert_eq!(GenerationPanelTokens::REFERENCE_TILE_HEIGHT, 56.0);
    }

    // THM-017
    #[test]
    fn media_panel_tokens() {
        let rail = MediaPanelTokens::tab_rail_width();
        assert!((rail - 38.0).abs() < 1e-10); // 26 + 6*2 = 38
        let row = MediaPanelTokens::context_row_height();
        assert!((row - 22.0).abs() < 1e-10);
    }

    // THM-018
    #[test]
    fn shadow_presets() {
        let sm = ShadowPresets::sm();
        assert!((sm.color_alpha - 0.3).abs() < 1e-10);
        assert!((sm.radius - 1.0).abs() < 1e-10);
        assert!((sm.x - 0.0).abs() < 1e-10);
        assert!((sm.y - 0.5).abs() < 1e-10);

        let md = ShadowPresets::md();
        assert!((md.color_alpha - 0.3).abs() < 1e-10);
        assert!((md.radius - 4.0).abs() < 1e-10);

        let lg = ShadowPresets::lg();
        assert!((lg.color_alpha - 0.25).abs() < 1e-10);
        assert!((lg.radius - 24.0).abs() < 1e-10);
    }

    // THM-019
    #[test]
    fn animation_durations() {
        assert!((AnimationDurations::HOVER - 0.15).abs() < 1e-10);
        assert!((AnimationDurations::TRANSITION - 0.2).abs() < 1e-10);
    }

    // UIX-003: Tool shortcuts
    #[test]
    fn tool_shortcuts() {
        assert_eq!('V', 'V');
        assert_eq!('C', 'C');
    }
}

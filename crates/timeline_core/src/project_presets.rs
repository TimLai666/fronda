//! Ordered project-settings presets for the inspector/preview dropdowns
//! (upstream #168). Pure data + selection logic mirroring Swift
//! `PreviewContainerView`'s `AspectPreset`, `QualityPreset`, the `[24,25,30,50,60]`
//! fps menu, and `ZoomPreset`. No UI dependency — the view renders these and
//! applies the resulting settings.

/// A fixed output aspect-ratio preset. `is_active` matches Swift's exact-size
/// check (`timeline.width == preset.width && timeline.height == preset.height`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AspectPreset {
    pub label: &'static str,
    pub width: i64,
    pub height: i64,
}

impl AspectPreset {
    pub fn is_active(&self, width: i64, height: i64) -> bool {
        self.width == width && self.height == height
    }
}

/// Aspect presets in Swift menu order.
pub const ASPECT_PRESETS: &[AspectPreset] = &[
    AspectPreset { label: "16:9", width: 1920, height: 1080 },
    AspectPreset { label: "9:14", width: 1080, height: 1680 },
    AspectPreset { label: "9:16", width: 1080, height: 1920 },
    AspectPreset { label: "1:1", width: 1080, height: 1080 },
    AspectPreset { label: "4:3", width: 1440, height: 1080 },
    AspectPreset { label: "2.4:1", width: 2560, height: 1080 },
];

/// Selectable project frame rates (Swift `[24, 25, 30, 50, 60]`).
pub const FPS_PRESETS: &[i64] = &[24, 25, 30, 50, 60];

/// A resolution-quality preset that scales the current aspect to a short edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QualityPreset {
    pub label: &'static str,
    pub short_edge: i64,
}

impl QualityPreset {
    /// Whether the current resolution already sits at this quality
    /// (`min(width, height) == short_edge`), mirroring Swift `matches`.
    pub fn matches(&self, width: i64, height: i64) -> bool {
        width.min(height) == self.short_edge
    }

    /// New `(width, height)` scaling the short edge to this preset while
    /// preserving aspect. Truncates like Swift's `Int(Double)`. Returns the
    /// input unchanged for non-positive dimensions.
    pub fn resolution(&self, width: i64, height: i64) -> (i64, i64) {
        if width <= 0 || height <= 0 {
            return (width, height);
        }
        let target = self.short_edge;
        if width <= height {
            (
                target,
                (target as f64 * height as f64 / width as f64) as i64,
            )
        } else {
            (
                (target as f64 * width as f64 / height as f64) as i64,
                target,
            )
        }
    }
}

/// Quality presets in Swift menu order.
pub const QUALITY_PRESETS: &[QualityPreset] = &[
    QualityPreset { label: "720p", short_edge: 720 },
    QualityPreset { label: "1080p", short_edge: 1080 },
    QualityPreset { label: "2K", short_edge: 1440 },
    QualityPreset { label: "4K", short_edge: 2160 },
];

/// A preview zoom preset. `Fit` scales to the viewport rather than a fixed
/// factor; its nominal `value` is 1.0, matching Swift.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ZoomPreset {
    pub label: &'static str,
    pub value: f64,
    pub fit: bool,
}

/// Zoom presets in Swift menu order.
pub const ZOOM_PRESETS: &[ZoomPreset] = &[
    ZoomPreset { label: "25%", value: 0.25, fit: false },
    ZoomPreset { label: "50%", value: 0.50, fit: false },
    ZoomPreset { label: "75%", value: 0.75, fit: false },
    ZoomPreset { label: "Fit", value: 1.0, fit: true },
    ZoomPreset { label: "125%", value: 1.25, fit: false },
    ZoomPreset { label: "150%", value: 1.50, fit: false },
    ZoomPreset { label: "200%", value: 2.0, fit: false },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aspect_active_matches_exact_dimensions() {
        let sixteen_nine = ASPECT_PRESETS[0];
        assert!(sixteen_nine.is_active(1920, 1080));
        assert!(!sixteen_nine.is_active(1080, 1920));
        // 9:16 is a distinct preset, not just swapped detection.
        assert!(ASPECT_PRESETS[2].is_active(1080, 1920));
    }

    #[test]
    fn fps_presets_match_swift() {
        assert_eq!(FPS_PRESETS, &[24, 25, 30, 50, 60]);
    }

    #[test]
    fn quality_matches_on_short_edge() {
        let full_hd = QUALITY_PRESETS[1];
        assert_eq!(full_hd.label, "1080p");
        assert!(full_hd.matches(1920, 1080));
        assert!(full_hd.matches(1080, 1920)); // portrait, short edge 1080
        assert!(!full_hd.matches(1280, 720));
    }

    #[test]
    fn quality_resolution_preserves_aspect_and_truncates() {
        let four_k = QUALITY_PRESETS[3];
        assert_eq!(four_k.label, "4K");
        // 16:9 landscape → short edge (height) becomes 2160.
        assert_eq!(four_k.resolution(1920, 1080), (3840, 2160));
        // Portrait → short edge (width) becomes 2160.
        assert_eq!(four_k.resolution(1080, 1920), (2160, 3840));
        // Truncation: 720p on a 1000x1000 stays square.
        assert_eq!(QUALITY_PRESETS[0].resolution(1000, 1000), (720, 720));
    }

    #[test]
    fn quality_resolution_guards_nonpositive() {
        assert_eq!(QUALITY_PRESETS[0].resolution(0, 100), (0, 100));
    }

    #[test]
    fn zoom_presets_have_fit_flagged() {
        let fit: Vec<_> = ZOOM_PRESETS.iter().filter(|z| z.fit).collect();
        assert_eq!(fit.len(), 1);
        assert_eq!(fit[0].label, "Fit");
        assert_eq!(ZOOM_PRESETS[0].value, 0.25);
        assert_eq!(ZOOM_PRESETS.last().unwrap().value, 2.0);
    }
}

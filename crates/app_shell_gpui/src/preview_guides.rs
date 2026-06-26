//! Viewer guide overlays for the preview canvas (Issue #167).
//!
//! Guides are view-only runtime state — never written to the project file,
//! reset on app restart. They draw safe-zone rectangles and format-reference
//! letterbox/pillarbox bars over the preview canvas without affecting export.

/// A single viewer guide type.
///
/// Safe-zone values follow SMPTE ST 2046-1 (2009) and ITU-R BT.1848-1 (2015).
/// Legacy RP 8 / RP 13 values (80% / 90%) are intentionally not used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ViewerGuide {
    // ── Safe-zone guides ────────────────────────────────────────────────────
    /// Action-safe area: 93% × 93% of frame (3.5% inset each side).
    ActionSafe,
    /// Title-safe area: 90% × 90% of frame (5% inset each side).
    TitleSafe,
    /// Crosshair at the geometric center of the frame.
    Center,

    // ── Format-reference guides ─────────────────────────────────────────────
    /// Anamorphic cinema: 2.39:1 (SMPTE post-1970).
    Scope,
    /// Flat theatrical / Netflix originals: 1.85:1.
    Wide,
    /// Instagram feed: 1:1.
    Square,
    /// Reels / Stories / TikTok: 9:16 portrait.
    Portrait,
}

impl ViewerGuide {
    /// Display label shown in the Guides menu.
    pub fn label(&self) -> &'static str {
        match self {
            ViewerGuide::ActionSafe => "Action Safe",
            ViewerGuide::TitleSafe => "Title Safe",
            ViewerGuide::Center => "Center",
            ViewerGuide::Scope => "Scope (2.39:1)",
            ViewerGuide::Wide => "Wide (1.85:1)",
            ViewerGuide::Square => "Square (1:1)",
            ViewerGuide::Portrait => "Portrait (9:16)",
        }
    }

    /// Whether this is a safe-zone overlay (rectangle + inset).
    pub fn is_safe_zone(&self) -> bool {
        matches!(self, ViewerGuide::ActionSafe | ViewerGuide::TitleSafe | ViewerGuide::Center)
    }

    /// Whether this is a format-reference overlay (letterbox/pillarbox bars).
    pub fn is_format_reference(&self) -> bool {
        matches!(
            self,
            ViewerGuide::Scope | ViewerGuide::Wide | ViewerGuide::Square | ViewerGuide::Portrait
        )
    }

    /// Aspect ratio for format-reference guides (width / height).
    /// Returns `None` for safe-zone guides.
    pub fn aspect_ratio(&self) -> Option<f64> {
        match self {
            ViewerGuide::Scope => Some(2.39),
            ViewerGuide::Wide => Some(1.85),
            ViewerGuide::Square => Some(1.0),
            ViewerGuide::Portrait => Some(9.0 / 16.0),
            _ => None,
        }
    }
}

/// Safe-zone inset as a fraction of the frame dimension (each side).
/// Values from SMPTE ST 2046-1 / ITU-R BT.1848-1.
pub struct SafeZoneInset {
    /// Fraction of width/height to inset from each edge.
    pub fraction: f64,
}

impl SafeZoneInset {
    /// Compute the inset rectangle normalized to [0,1] × [0,1].
    ///
    /// Returns `(left, top, right, bottom)` where each value is in [0,1].
    pub fn rect(&self) -> (f64, f64, f64, f64) {
        let f = self.fraction;
        (f, f, 1.0 - f, 1.0 - f)
    }
}

impl ViewerGuide {
    /// Return the safe-zone inset for this guide, or `None` if not applicable.
    pub fn safe_zone_inset(&self) -> Option<SafeZoneInset> {
        match self {
            ViewerGuide::ActionSafe => Some(SafeZoneInset { fraction: 0.035 }),
            ViewerGuide::TitleSafe => Some(SafeZoneInset { fraction: 0.05 }),
            _ => None,
        }
    }
}

/// Runtime state for viewer guides (never persisted).
#[derive(Debug, Clone, Default)]
pub struct ViewerGuideState {
    active: Vec<ViewerGuide>,
}

impl ViewerGuideState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Toggle a guide on/off.
    pub fn toggle(&mut self, guide: ViewerGuide) {
        if let Some(pos) = self.active.iter().position(|g| *g == guide) {
            self.active.remove(pos);
        } else {
            self.active.push(guide);
        }
    }

    /// Whether any guide is currently active (drives icon state).
    pub fn any_active(&self) -> bool {
        !self.active.is_empty()
    }

    /// Whether a specific guide is active.
    pub fn is_active(&self, guide: ViewerGuide) -> bool {
        self.active.contains(&guide)
    }

    /// Disable all guides.
    pub fn clear(&mut self) {
        self.active.clear();
    }

    /// Currently active guides (order: insertion order).
    pub fn active_guides(&self) -> &[ViewerGuide] {
        &self.active
    }

    /// Compute letterbox/pillarbox bar rectangles for a format-reference guide
    /// given the canvas dimensions.
    ///
    /// Returns `None` if the guide is not a format-reference guide or if the
    /// timeline already matches the guide's aspect ratio (within tolerance).
    ///
    /// Returns `Some((bar1, bar2))` where each bar is `(x, y, width, height)`
    /// in canvas pixels.
    pub fn format_bars(
        &self,
        guide: ViewerGuide,
        canvas_w: f64,
        canvas_h: f64,
    ) -> Option<((f64, f64, f64, f64), (f64, f64, f64, f64))> {
        let target_ratio = guide.aspect_ratio()?;
        let canvas_ratio = canvas_w / canvas_h;
        const TOLERANCE: f64 = 0.02; // UIX-006

        if (canvas_ratio - target_ratio).abs() < TOLERANCE {
            return None; // Timeline already matches — no bars needed
        }

        if target_ratio > canvas_ratio {
            // Pillarbox: canvas is taller relative to width — add top/bottom bars
            // Actually if target is wider than canvas, add left/right (letterbox)
            let content_h = canvas_w / target_ratio;
            let bar_h = (canvas_h - content_h) / 2.0;
            let top = (0.0, 0.0, canvas_w, bar_h);
            let bottom = (0.0, canvas_h - bar_h, canvas_w, bar_h);
            Some((top, bottom))
        } else {
            // Pillarbox: target is narrower — add left/right bars
            let content_w = canvas_h * target_ratio;
            let bar_w = (canvas_w - content_w) / 2.0;
            let left = (0.0, 0.0, bar_w, canvas_h);
            let right = (canvas_w - bar_w, 0.0, bar_w, canvas_h);
            Some((left, right))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guide_labels_non_empty() {
        for g in [
            ViewerGuide::ActionSafe,
            ViewerGuide::TitleSafe,
            ViewerGuide::Center,
            ViewerGuide::Scope,
            ViewerGuide::Wide,
            ViewerGuide::Square,
            ViewerGuide::Portrait,
        ] {
            assert!(!g.label().is_empty(), "{g:?} has no label");
        }
    }

    #[test]
    fn safe_zone_inset_action_safe() {
        let inset = ViewerGuide::ActionSafe.safe_zone_inset().unwrap();
        assert!((inset.fraction - 0.035).abs() < 1e-9);
        let (l, t, r, b) = inset.rect();
        assert!((l - 0.035).abs() < 1e-9);
        assert!((r - 0.965).abs() < 1e-9);
        assert!((t - 0.035).abs() < 1e-9);
        assert!((b - 0.965).abs() < 1e-9);
    }

    #[test]
    fn safe_zone_inset_title_safe() {
        let inset = ViewerGuide::TitleSafe.safe_zone_inset().unwrap();
        assert!((inset.fraction - 0.05).abs() < 1e-9);
    }

    #[test]
    fn center_guide_no_inset() {
        assert!(ViewerGuide::Center.safe_zone_inset().is_none());
    }

    #[test]
    fn aspect_ratios_defined() {
        assert!((ViewerGuide::Scope.aspect_ratio().unwrap() - 2.39).abs() < 1e-9);
        assert!((ViewerGuide::Wide.aspect_ratio().unwrap() - 1.85).abs() < 1e-9);
        assert!((ViewerGuide::Square.aspect_ratio().unwrap() - 1.0).abs() < 1e-9);
        let portrait = ViewerGuide::Portrait.aspect_ratio().unwrap();
        assert!((portrait - 9.0 / 16.0).abs() < 1e-9);
    }

    #[test]
    fn format_reference_guides_have_aspect_ratio() {
        for g in [ViewerGuide::Scope, ViewerGuide::Wide, ViewerGuide::Square, ViewerGuide::Portrait] {
            assert!(g.aspect_ratio().is_some(), "{g:?} missing aspect ratio");
            assert!(g.is_format_reference());
            assert!(!g.is_safe_zone());
        }
    }

    #[test]
    fn safe_zone_guides_no_aspect_ratio() {
        for g in [ViewerGuide::ActionSafe, ViewerGuide::TitleSafe, ViewerGuide::Center] {
            assert!(g.aspect_ratio().is_none(), "{g:?} should not have aspect ratio");
            assert!(g.is_safe_zone());
            assert!(!g.is_format_reference());
        }
    }

    #[test]
    fn viewer_guide_state_toggle() {
        let mut state = ViewerGuideState::new();
        assert!(!state.any_active());
        state.toggle(ViewerGuide::ActionSafe);
        assert!(state.any_active());
        assert!(state.is_active(ViewerGuide::ActionSafe));
        state.toggle(ViewerGuide::ActionSafe); // off
        assert!(!state.is_active(ViewerGuide::ActionSafe));
        assert!(!state.any_active());
    }

    #[test]
    fn viewer_guide_state_multiple() {
        let mut state = ViewerGuideState::new();
        state.toggle(ViewerGuide::ActionSafe);
        state.toggle(ViewerGuide::TitleSafe);
        assert_eq!(state.active_guides().len(), 2);
        state.clear();
        assert!(!state.any_active());
    }

    #[test]
    fn format_bars_16x9_canvas_scope_guide() {
        // 1920×1080 canvas (16:9 = 1.777), Scope = 2.39 → letterbox bars top/bottom
        let state = ViewerGuideState::new();
        let result = state.format_bars(ViewerGuide::Scope, 1920.0, 1080.0);
        assert!(result.is_some(), "Scope bars expected on 16:9 canvas");
        let (bar1, bar2) = result.unwrap();
        // Both bars should span full width
        assert!((bar1.2 - 1920.0).abs() < 1.0);
        assert!((bar2.2 - 1920.0).abs() < 1.0);
        // Bars should be identical height
        assert!((bar1.3 - bar2.3).abs() < 1.0);
    }

    #[test]
    fn format_bars_matching_ratio_returns_none() {
        let state = ViewerGuideState::new();
        // Canvas is 2:1, guide is Wide (1.85:1) — within tolerance? No, 2.0 vs 1.85 is > 0.02
        // Use square canvas and square guide — exact match
        let result = state.format_bars(ViewerGuide::Square, 1080.0, 1080.0);
        assert!(result.is_none(), "No bars for matching aspect ratio");
    }

    #[test]
    fn format_bars_portrait_on_landscape_canvas() {
        // 1920×1080 canvas (landscape), Portrait = 9:16 (0.5625) → pillarbox bars
        let state = ViewerGuideState::new();
        let result = state.format_bars(ViewerGuide::Portrait, 1920.0, 1080.0);
        assert!(result.is_some(), "Portrait bars expected on landscape canvas");
        let (bar1, bar2) = result.unwrap();
        // Bars should be left/right (full height, partial width)
        assert!((bar1.3 - 1080.0).abs() < 1.0);
        assert!((bar2.3 - 1080.0).abs() < 1.0);
    }
}

//! Solid-colour matte sizing (upstream #242): the aspect presets + pixel-dimension math for the
//! `create_matte` agent tool. Pure integer math; the PNG render + project file-write live in the
//! render/app layers. Mirrors Swift `MatteAspect` + `Matte.even`/`Matte.fit`.

/// Aspect preset for a generated matte. `Project` matches the current timeline; the rest are
/// fixed ratios fit against the timeline's short edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatteAspect {
    Project,
    SixteenNine,
    NineSixteen,
    OneOne,
    FourThree,
    NineFourteen,
    TwoPointFourOne,
}

impl MatteAspect {
    pub const ALL: [MatteAspect; 7] = [
        MatteAspect::Project,
        MatteAspect::SixteenNine,
        MatteAspect::NineSixteen,
        MatteAspect::OneOne,
        MatteAspect::FourThree,
        MatteAspect::NineFourteen,
        MatteAspect::TwoPointFourOne,
    ];

    /// The agent/UI wire string (Swift `rawValue`).
    pub fn raw_value(self) -> &'static str {
        match self {
            MatteAspect::Project => "Project",
            MatteAspect::SixteenNine => "16:9",
            MatteAspect::NineSixteen => "9:16",
            MatteAspect::OneOne => "1:1",
            MatteAspect::FourThree => "4:3",
            MatteAspect::NineFourteen => "9:14",
            MatteAspect::TwoPointFourOne => "2.4:1",
        }
    }

    fn ratio(self) -> Option<(i64, i64)> {
        match self {
            MatteAspect::Project => None,
            MatteAspect::SixteenNine => Some((16, 9)),
            MatteAspect::NineSixteen => Some((9, 16)),
            MatteAspect::OneOne => Some((1, 1)),
            MatteAspect::FourThree => Some((4, 3)),
            MatteAspect::NineFourteen => Some((9, 14)),
            MatteAspect::TwoPointFourOne => Some((24, 10)),
        }
    }

    /// Parse the wire string: `"project"` (any case) → `Project`, else an exact rawValue match.
    /// Mirrors Swift `MatteAspect.parse`.
    pub fn parse(raw: &str) -> Option<Self> {
        let raw = raw.trim();
        if raw.is_empty() {
            return None;
        }
        if raw.eq_ignore_ascii_case("project") {
            return Some(MatteAspect::Project);
        }
        Self::ALL.into_iter().find(|a| a.raw_value() == raw)
    }

    /// Pixel dimensions for this aspect given the timeline size: `Project` matches the timeline
    /// (rounded even); a fixed ratio is fit against the timeline's short edge. Mirrors Swift
    /// `MatteAspect.pixelSize`.
    pub fn pixel_size(self, timeline_w: i64, timeline_h: i64) -> (i64, i64) {
        match self.ratio() {
            None => even(timeline_w, timeline_h),
            Some((aw, ah)) => fit(timeline_w.min(timeline_h), aw, ah),
        }
    }
}

/// Round each dimension down to an even number, minimum 2 (encoders need even dimensions).
/// Mirrors Swift `Matte.even`.
pub fn even(w: i64, h: i64) -> (i64, i64) {
    (((w.max(2) / 2) * 2).max(2), ((h.max(2) / 2) * 2).max(2))
}

/// Fit an aspect ratio against a short edge → even pixel dimensions. Mirrors Swift `Matte.fit`.
pub fn fit(short_edge: i64, aspect_w: i64, aspect_h: i64) -> (i64, i64) {
    let e = short_edge.max(2);
    let aw = aspect_w as f64;
    let ah = aspect_h as f64;
    if aw >= ah {
        even((e as f64 * aw / ah).round() as i64, e)
    } else {
        even(e, (e as f64 * ah / aw).round() as i64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn even_rounds_down_to_even_min_2() {
        assert_eq!(even(1920, 1080), (1920, 1080));
        assert_eq!(even(3, 5), (2, 4));
        assert_eq!(even(1, 1), (2, 2));
        assert_eq!(even(0, -4), (2, 2));
    }

    #[test]
    fn project_matches_timeline() {
        assert_eq!(MatteAspect::Project.pixel_size(1920, 1080), (1920, 1080));
        assert_eq!(MatteAspect::Project.pixel_size(1921, 1081), (1920, 1080));
    }

    #[test]
    fn landscape_ratio_fits_short_edge() {
        // 16:9 in a 1920x1080 timeline: short edge 1080 → 1920x1080.
        assert_eq!(MatteAspect::SixteenNine.pixel_size(1920, 1080), (1920, 1080));
        // 16:9 in a portrait 1080x1920 timeline: short edge 1080 → still 1920x1080.
        assert_eq!(MatteAspect::SixteenNine.pixel_size(1080, 1920), (1920, 1080));
    }

    #[test]
    fn portrait_ratio_fits_short_edge() {
        // 9:16 in 1920x1080: short 1080 → 1080x1920.
        assert_eq!(MatteAspect::NineSixteen.pixel_size(1920, 1080), (1080, 1920));
    }

    #[test]
    fn square_and_others() {
        assert_eq!(MatteAspect::OneOne.pixel_size(1920, 1080), (1080, 1080));
        // 2.4:1 (24:10) landscape: short 1080 → 1080*24/10 = 2592 x 1080.
        assert_eq!(MatteAspect::TwoPointFourOne.pixel_size(1920, 1080), (2592, 1080));
    }

    #[test]
    fn parse_wire_strings() {
        assert_eq!(MatteAspect::parse("Project"), Some(MatteAspect::Project));
        assert_eq!(MatteAspect::parse("project"), Some(MatteAspect::Project));
        assert_eq!(MatteAspect::parse(" 16:9 "), Some(MatteAspect::SixteenNine));
        assert_eq!(MatteAspect::parse("2.4:1"), Some(MatteAspect::TwoPointFourOne));
        assert_eq!(MatteAspect::parse("nonsense"), None);
        assert_eq!(MatteAspect::parse(""), None);
    }
}

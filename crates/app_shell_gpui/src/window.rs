/// Window type identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowKind {
    Home,
    Project,
    Settings,
    Help,
    Feedback,
}

/// Window configuration.
#[derive(Debug, Clone)]
pub struct WindowConfig {
    pub kind: WindowKind,
    pub title: String,
    pub default_width: f64,
    pub default_height: f64,
    pub min_width: f64,
    pub min_height: f64,
}

impl WindowConfig {
    pub fn new(
        kind: WindowKind,
        title: &str,
        default_width: f64,
        default_height: f64,
        min_width: f64,
        min_height: f64,
    ) -> Self {
        Self {
            kind,
            title: title.to_string(),
            default_width,
            default_height,
            min_width,
            min_height,
        }
    }

    /// WIN-001: Home default 1200×800 (#319), min 760×480 (upstream #204).
    pub fn for_home() -> Self {
        Self::new(WindowKind::Home, "Fronda", 1200.0, 800.0, 760.0, 480.0)
    }

    /// WIN-002: Project default 1600×1000, min 960×600.
    pub fn for_project() -> Self {
        Self::new(
            WindowKind::Project,
            "Fronda — Project",
            1600.0,
            1000.0,
            960.0,
            600.0,
        )
    }

    /// WIN-004: Settings default 1200×800 (#319), min 860×640 (upstream #204).
    pub fn for_settings() -> Self {
        Self::new(
            WindowKind::Settings,
            "Settings",
            1200.0,
            800.0,
            860.0,
            640.0,
        )
    }

    /// WIN-006: Help window with tabs (Shortcuts, MCP).
    pub fn for_help() -> Self {
        Self::new(WindowKind::Help, "Help", 760.0, 540.0, 600.0, 400.0)
    }

    /// WIN-007: Feedback default 480×480, min 480×420.
    pub fn for_feedback() -> Self {
        Self::new(
            WindowKind::Feedback,
            "Send feedback",
            480.0,
            480.0,
            480.0,
            420.0,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn win_001_home_window_size() {
        let cfg = WindowConfig::for_home();
        assert_eq!(cfg.kind, WindowKind::Home);
        assert_eq!(cfg.default_width, 1200.0);
        assert_eq!(cfg.default_height, 800.0);
        assert_eq!(cfg.min_width, 760.0);
        assert_eq!(cfg.min_height, 480.0);
    }

    #[test]
    fn win_002_project_window_size() {
        let cfg = WindowConfig::for_project();
        assert_eq!(cfg.kind, WindowKind::Project);
        assert_eq!(cfg.default_width, 1600.0);
        assert_eq!(cfg.default_height, 1000.0);
        assert_eq!(cfg.min_width, 960.0);
        assert_eq!(cfg.min_height, 600.0);
    }

    #[test]
    fn win_004_settings_window_size() {
        let cfg = WindowConfig::for_settings();
        assert_eq!(cfg.kind, WindowKind::Settings);
        assert_eq!(cfg.default_width, 1200.0);
        assert_eq!(cfg.default_height, 800.0);
        assert_eq!(cfg.min_width, 860.0);
        assert_eq!(cfg.min_height, 640.0);
    }

    #[test]
    fn win_007_feedback_window_size() {
        let cfg = WindowConfig::for_feedback();
        assert_eq!(cfg.kind, WindowKind::Feedback);
        assert_eq!(cfg.default_width, 480.0);
        assert_eq!(cfg.default_height, 480.0);
        assert_eq!(cfg.min_width, 480.0);
        assert_eq!(cfg.min_height, 420.0);
    }
}

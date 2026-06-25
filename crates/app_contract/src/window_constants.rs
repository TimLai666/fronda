//! Window size and native-window contracts.
//!
//! Covers WIN-001 through WIN-007.

/// WIN-001: Home window size constraints.
pub struct HomeWindow;
impl HomeWindow {
    pub const DEFAULT_WIDTH: f64 = 1200.0;
    pub const DEFAULT_HEIGHT: f64 = 1200.0;
    pub const MIN_WIDTH: f64 = 760.0;
    pub const MIN_HEIGHT: f64 = 480.0;
}

/// WIN-002: Project window size constraints.
pub struct ProjectWindow;
impl ProjectWindow {
    pub const DEFAULT_WIDTH: f64 = 1600.0;
    pub const DEFAULT_HEIGHT: f64 = 1000.0;
    pub const MIN_WIDTH: f64 = 960.0;
    pub const MIN_HEIGHT: f64 = 600.0;
}

/// WIN-003: Project titlebar trailing reserved width.
pub const PROJECT_TITLEBAR_TRAILING_WIDTH: f64 = 280.0;

/// WIN-004: Settings window.
pub struct SettingsWindow;
impl SettingsWindow {
    pub const DEFAULT_WIDTH: f64 = 980.0;
    pub const DEFAULT_HEIGHT: f64 = 640.0;
    pub const MIN_WIDTH: f64 = 760.0;
    pub const MIN_HEIGHT: f64 = 480.0;
    pub const AUTOSAVE_NAME: &'static str = "PalmierProSettings-v2";
}

/// WIN-005: Settings window style flags.
pub struct SettingsWindowStyle;
impl SettingsWindowStyle {
    pub const DARK_APPEARANCE: bool = true;
    pub const TRANSLUCENT_BACKGROUND: bool = true;
    pub const HIDDEN_TITLE: bool = true;
    pub const TRANSPARENT_TITLEBAR: bool = true;
    pub const FULL_SIZE_CONTENT_VIEW: bool = true;
    pub const MOVABLE_BY_BACKGROUND: bool = true;
}

/// WIN-006: Help window.
pub struct HelpWindow;
impl HelpWindow {
    pub const TABS: &'static [&'static str] = &["Shortcuts", "MCP"];
    pub const SIDEBAR_WIDTH: f64 = 220.0;
}

/// WIN-007: Feedback window.
pub struct FeedbackWindow;
impl FeedbackWindow {
    pub const DEFAULT_WIDTH: f64 = 480.0;
    pub const DEFAULT_HEIGHT: f64 = 480.0;
    pub const MIN_WIDTH: f64 = 480.0;
    pub const MIN_HEIGHT: f64 = 420.0;
    pub const TITLE: &'static str = "Send feedback";
}

#[cfg(test)]
mod tests {
    use super::*;

    // WIN-001
    #[test]
    fn home_window_default() {
        assert_eq!(HomeWindow::DEFAULT_WIDTH, 1200.0);
        assert_eq!(HomeWindow::DEFAULT_HEIGHT, 1200.0);
    }

    #[test]
    fn home_window_minimum() {
        assert_eq!(HomeWindow::MIN_WIDTH, 760.0);
        assert_eq!(HomeWindow::MIN_HEIGHT, 480.0);
    }

    // WIN-002
    #[test]
    fn project_window_default() {
        assert_eq!(ProjectWindow::DEFAULT_WIDTH, 1600.0);
        assert_eq!(ProjectWindow::DEFAULT_HEIGHT, 1000.0);
    }

    #[test]
    fn project_window_minimum() {
        assert_eq!(ProjectWindow::MIN_WIDTH, 960.0);
        assert_eq!(ProjectWindow::MIN_HEIGHT, 600.0);
    }

    // WIN-003
    #[test]
    fn project_titlebar_trailing() {
        assert_eq!(PROJECT_TITLEBAR_TRAILING_WIDTH, 280.0);
    }

    // WIN-004
    #[test]
    fn settings_window_default() {
        assert_eq!(SettingsWindow::DEFAULT_WIDTH, 980.0);
        assert_eq!(SettingsWindow::DEFAULT_HEIGHT, 640.0);
    }

    #[test]
    fn settings_window_minimum() {
        assert_eq!(SettingsWindow::MIN_WIDTH, 760.0);
        assert_eq!(SettingsWindow::MIN_HEIGHT, 480.0);
    }

    #[test]
    fn settings_window_autosave() {
        assert_eq!(SettingsWindow::AUTOSAVE_NAME, "PalmierProSettings-v2");
    }

    // WIN-005
    #[test]
    fn settings_window_style() {
        assert!(SettingsWindowStyle::DARK_APPEARANCE);
        assert!(SettingsWindowStyle::TRANSLUCENT_BACKGROUND);
        assert!(SettingsWindowStyle::HIDDEN_TITLE);
        assert!(SettingsWindowStyle::TRANSPARENT_TITLEBAR);
        assert!(SettingsWindowStyle::FULL_SIZE_CONTENT_VIEW);
        assert!(SettingsWindowStyle::MOVABLE_BY_BACKGROUND);
    }

    // WIN-006
    #[test]
    fn help_window_tabs() {
        assert_eq!(HelpWindow::TABS, &["Shortcuts", "MCP"]);
    }

    #[test]
    fn help_window_sidebar() {
        assert_eq!(HelpWindow::SIDEBAR_WIDTH, 220.0);
    }

    // WIN-007
    #[test]
    fn feedback_window_default() {
        assert_eq!(FeedbackWindow::DEFAULT_WIDTH, 480.0);
        assert_eq!(FeedbackWindow::DEFAULT_HEIGHT, 480.0);
    }

    #[test]
    fn feedback_window_minimum() {
        assert_eq!(FeedbackWindow::MIN_WIDTH, 480.0);
        assert_eq!(FeedbackWindow::MIN_HEIGHT, 420.0);
    }

    #[test]
    fn feedback_window_title() {
        assert_eq!(FeedbackWindow::TITLE, "Send feedback");
    }
}

//! Feedback submission model.
//!
//! Covers FBK-001 through FBK-006.

use serde::Serialize;

/// FBK-001: Initial feedback state.
#[derive(Debug, Clone, PartialEq)]
pub struct FeedbackState {
    pub message: String,
    pub email: String,
    pub include_screenshot: bool,
    pub may_contact: bool,
    pub sending: bool,
    pub error: Option<String>,
    pub sent: bool,
}

impl Default for FeedbackState {
    /// FBK-001: Default initial state.
    fn default() -> Self {
        Self {
            message: String::new(),
            email: String::new(),
            include_screenshot: true,
            may_contact: true,
            sending: false,
            error: None,
            sent: false,
        }
    }
}

impl FeedbackState {
    /// FBK-002: Validate that the message is non-empty after trimming.
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.message.trim().is_empty() {
            return Err("Message is required");
        }
        Ok(())
    }

    /// FBK-002: Submit shortcut is Cmd+Return.
    pub const SUBMIT_SHORTCUT: &'static str = "Cmd+Return";
    /// FBK-003: Cancel keyboard action.
    pub const CANCEL_KEY: &'static str = "Escape";
    /// FBK-003: Success Done uses default keyboard action.
    pub const DONE_KEY: &'static str = "Return";
}

/// FBK-004/005: Screenshot capture strategy priority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScreenshotTarget {
    KeyWindow,
    MainWindow,
    FirstVisibleNonFeedback,
}

impl ScreenshotTarget {
    /// FBK-004: Priority-ordered list of screenshot targets.
    pub fn priority() -> [ScreenshotTarget; 3] {
        [
            ScreenshotTarget::KeyWindow,
            ScreenshotTarget::MainWindow,
            ScreenshotTarget::FirstVisibleNonFeedback,
        ]
    }
}

/// APP-005: Maximum width for feedback screenshots in pixels.
pub const FEEDBACK_SCREENSHOT_MAX_WIDTH: u32 = 1920;
/// APP-005: Maximum height for feedback screenshots in pixels.
pub const FEEDBACK_SCREENSHOT_MAX_HEIGHT: u32 = 1080;
/// APP-005: JPEG compression quality for feedback screenshots (0.0–1.0).
pub const FEEDBACK_SCREENSHOT_QUALITY: f64 = 0.85;

/// APP-005: Determine the downscale factor needed to fit within max dimensions.
///
/// Returns `1.0` if the image already fits, or the smallest uniform scale
/// factor that makes both dimensions fit.
pub fn screenshot_downscale_factor(original_width: u32, original_height: u32) -> f64 {
    if original_width == 0 || original_height == 0 {
        return 1.0;
    }
    let scale_w = FEEDBACK_SCREENSHOT_MAX_WIDTH as f64 / original_width as f64;
    let scale_h = FEEDBACK_SCREENSHOT_MAX_HEIGHT as f64 / original_height as f64;
    scale_w.min(scale_h).min(1.0)
}

/// APP-005: Compute the downscaled dimensions.
pub fn screenshot_downscaled_size(original_width: u32, original_height: u32) -> (u32, u32) {
    let factor = screenshot_downscale_factor(original_width, original_height);
    (
        (original_width as f64 * factor).round() as u32,
        (original_height as f64 * factor).round() as u32,
    )
}

/// FBK-006: Feedback submission payload.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FeedbackPayload {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    pub may_contact: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screenshot_base64: Option<String>,
    pub app_version: String,
    pub os_version: String,
}

impl FeedbackPayload {
    /// Build a payload from state, checking may_contact requires a non-empty email.
    pub fn from_state(state: &FeedbackState, app_version: &str, os_version: &str) -> Self {
        Self {
            message: state.message.trim().to_string(),
            email: if state.email.trim().is_empty() {
                None
            } else {
                Some(state.email.trim().to_string())
            },
            may_contact: state.may_contact && !state.email.trim().is_empty(),
            screenshot_base64: None, // caller fills this after capture when enabled
            app_version: app_version.to_string(),
            os_version: os_version.to_string(),
        }
    }

    /// Attach a base64-encoded screenshot after capture.
    pub fn with_screenshot(mut self, base64: String) -> Self {
        self.screenshot_base64 = Some(base64);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // FBK-001
    #[test]
    fn feedback_default_state() {
        let state = FeedbackState::default();
        assert!(state.message.is_empty());
        assert!(state.email.is_empty());
        assert!(state.include_screenshot);
        assert!(state.may_contact);
        assert!(!state.sending);
        assert_eq!(state.error, None);
        assert!(!state.sent);
    }

    // FBK-002
    #[test]
    fn validate_empty_message_fails() {
        let state = FeedbackState::default();
        assert!(state.validate().is_err());
    }

    #[test]
    fn validate_non_empty_message_succeeds() {
        let mut state = FeedbackState::default();
        state.message = "Test message".into();
        assert!(state.validate().is_ok());
    }

    #[test]
    fn validate_whitespace_only_fails() {
        let mut state = FeedbackState::default();
        state.message = "   \n  ".into();
        assert!(state.validate().is_err());
    }

    #[test]
    fn submit_shortcut() {
        assert_eq!(FeedbackState::SUBMIT_SHORTCUT, "Cmd+Return");
    }

    // FBK-003
    #[test]
    fn cancel_and_done_keys() {
        assert_eq!(FeedbackState::CANCEL_KEY, "Escape");
        assert_eq!(FeedbackState::DONE_KEY, "Return");
    }

    // FBK-004
    #[test]
    fn screenshot_target_priority() {
        let targets = ScreenshotTarget::priority();
        assert_eq!(targets[0], ScreenshotTarget::KeyWindow);
        assert_eq!(targets[1], ScreenshotTarget::MainWindow);
        assert_eq!(targets[2], ScreenshotTarget::FirstVisibleNonFeedback);
    }

    // FBK-006
    #[test]
    fn payload_from_state_with_email() {
        let mut state = FeedbackState::default();
        state.message = "Great app!".into();
        state.email = "user@example.com".into();
        state.may_contact = true;
        let payload = FeedbackPayload::from_state(&state, "0.3.5", "macOS 26.0");
        assert_eq!(payload.message, "Great app!");
        assert_eq!(payload.email, Some("user@example.com".into()));
        assert!(payload.may_contact);
        assert_eq!(payload.app_version, "0.3.5");
        assert_eq!(payload.os_version, "macOS 26.0");
    }

    #[test]
    fn payload_may_contact_false_when_email_empty() {
        let mut state = FeedbackState::default();
        state.message = "Feedback".into();
        state.may_contact = true;
        state.email = "".into();
        let payload = FeedbackPayload::from_state(&state, "1.0", "macOS 26.0");
        assert!(!payload.may_contact);
        assert_eq!(payload.email, None);
    }

    #[test]
    fn payload_with_screenshot() {
        let mut state = FeedbackState::default();
        state.message = "Test".into();
        let payload = FeedbackPayload::from_state(&state, "1.0", "macOS 26.0");
        let with_ss = payload.with_screenshot("iVBOR...".into());
        assert_eq!(with_ss.screenshot_base64, Some("iVBOR...".into()));
    }

    #[test]
    fn payload_serialization_round_trip() {
        let payload = FeedbackPayload {
            message: "Test".into(),
            email: Some("user@example.com".into()),
            may_contact: true,
            screenshot_base64: Some("iVBOR...".into()),
            app_version: "0.3.5".into(),
            os_version: "macOS 26.0".into(),
        };
        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("Test"));
        assert!(json.contains("user@example.com"));
    }

    #[test]
    fn payload_serialization_skips_optional_none() {
        let payload = FeedbackPayload {
            message: "No contact info".into(),
            email: None,
            may_contact: false,
            screenshot_base64: None,
            app_version: "0.3.5".into(),
            os_version: "macOS 26.0".into(),
        };
        let json = serde_json::to_string(&payload).unwrap();
        // email and screenshot_base64 should be absent from JSON when None
        assert!(!json.contains("email"));
        assert!(!json.contains("screenshot_base64"));
    }

    // ── APP-005: Screenshot sizing ──────────────────────────────

    #[test]
    fn app_005_screenshot_fits_within_max() {
        let factor = screenshot_downscale_factor(1280, 720);
        assert!((factor - 1.0).abs() < f64::EPSILON);
        let (w, h) = screenshot_downscaled_size(1280, 720);
        assert_eq!(w, 1280);
        assert_eq!(h, 720);
    }

    #[test]
    fn app_005_screenshot_needs_downscale_width() {
        let (w, h) = screenshot_downscaled_size(3840, 2160);
        assert!(w <= 1920);
        assert!(h <= 1080);
        // 3840 -> 1920 is 0.5x, 2160 -> 1080 is 0.5x
        assert_eq!(w, 1920);
        assert_eq!(h, 1080);
    }

    #[test]
    fn app_005_screenshot_needs_downscale_height() {
        let (w, h) = screenshot_downscaled_size(1200, 2400);
        assert!(w <= 1920);
        assert!(h <= 1080);
        assert_eq!(h, 1080);
    }

    #[test]
    fn app_005_screenshot_zero_dimensions() {
        assert!((screenshot_downscale_factor(0, 0) - 1.0).abs() < f64::EPSILON);
        assert!((screenshot_downscale_factor(100, 0) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn app_005_screenshot_constants_are_reasonable() {
        assert!(FEEDBACK_SCREENSHOT_MAX_WIDTH >= 1024);
        assert!(FEEDBACK_SCREENSHOT_MAX_HEIGHT >= 1024);
        assert!((0.0..=1.0).contains(&FEEDBACK_SCREENSHOT_QUALITY));
    }
}

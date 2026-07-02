//! Data types for the Feedback view — pure logic, no gpui dependency.
//!
//! Covers FBK-001 through FBK-006.

/// State for the feedback view.
#[derive(Debug, Clone, PartialEq)]
pub struct FeedbackViewModel {
    pub message: String,
    pub email: String,
    pub include_screenshot: bool,
    pub may_contact: bool,
    pub is_sending: bool,
    pub error: Option<String>,
    pub sent: bool,
}

impl Default for FeedbackViewModel {
    fn default() -> Self {
        Self {
            message: String::new(),
            email: String::new(),
            include_screenshot: true,
            may_contact: true,
            is_sending: false,
            error: None,
            sent: false,
        }
    }
}

impl FeedbackViewModel {
    /// Validate the form state.
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.message.trim().is_empty() {
            return Err("Message is required");
        }
        Ok(())
    }

    /// Build payload for submission.
    pub fn build_payload(
        &self,
        app_version: &str,
        os_version: &str,
    ) -> crate::feedback::FeedbackPayload {
        let state = crate::feedback::FeedbackState {
            message: self.message.clone(),
            email: self.email.clone(),
            include_screenshot: self.include_screenshot,
            may_contact: self.may_contact,
            sending: self.is_sending,
            error: self.error.clone(),
            sent: self.sent,
        };
        crate::feedback::FeedbackPayload::from_state(&state, app_version, os_version)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feedback_default_state() {
        let model = FeedbackViewModel::default();
        assert!(model.message.is_empty());
        assert!(model.email.is_empty());
        assert!(model.include_screenshot);
        assert!(model.may_contact);
        assert!(!model.is_sending);
        assert_eq!(model.error, None);
        assert!(!model.sent);
    }

    #[test]
    fn feedback_validation_empty_fails() {
        let model = FeedbackViewModel::default();
        assert!(model.validate().is_err());
    }

    #[test]
    fn feedback_validation_non_empty_succeeds() {
        let model = FeedbackViewModel {
            message: "Test feedback".into(),
            ..Default::default()
        };
        assert!(model.validate().is_ok());
    }

    #[test]
    fn feedback_build_payload() {
        let model = FeedbackViewModel {
            message: "Great app!".into(),
            email: "user@example.com".into(),
            ..Default::default()
        };
        let payload = model.build_payload("0.3.5", "macOS 26.0");
        assert_eq!(payload.message, "Great app!");
        assert_eq!(payload.email, Some("user@example.com".into()));
        assert!(payload.may_contact);
    }
}

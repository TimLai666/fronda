//! Update badge visibility (APP-008).

use serde::{Deserialize, Serialize};

/// APP-008: The current state of update availability.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum UpdateAvailability {
    /// No update check has been performed.
    Unknown,
    /// No update is available.
    UpToDate,
    /// A new version is available.
    Available(String),
    /// Update check failed.
    Failed(String),
}

/// APP-008: Whether the update badge is currently dismissed by the user.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UpdateBadgeState {
    /// Current update availability.
    pub availability: UpdateAvailability,
    /// Whether the user has dismissed the badge.
    pub dismissed: bool,
    /// The last checked version (to avoid re-showing after dismissal).
    pub last_checked_version: Option<String>,
}

impl Default for UpdateBadgeState {
    fn default() -> Self {
        Self {
            availability: UpdateAvailability::Unknown,
            dismissed: false,
            last_checked_version: None,
        }
    }
}

impl UpdateBadgeState {
    /// Set update availability.
    pub fn set_availability(&mut self, availability: UpdateAvailability) {
        // Re-show badge when a new version is found that differs from the dismissed one.
        if let UpdateAvailability::Available(ref version) = availability {
            if self.last_checked_version.as_deref() != Some(version.as_str()) || !self.dismissed {
                self.dismissed = false;
                self.last_checked_version = Some(version.clone());
            }
        }
        self.availability = availability;
    }

    /// Dismiss the badge for the current version.
    pub fn dismiss(&mut self) {
        self.dismissed = true;
        if let UpdateAvailability::Available(ref version) = self.availability {
            self.last_checked_version = Some(version.clone());
        }
    }

    /// Whether the badge should be visible.
    pub fn is_visible(&self) -> bool {
        !self.dismissed && matches!(self.availability, UpdateAvailability::Available(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_008_default_not_visible() {
        let state = UpdateBadgeState::default();
        assert!(!state.is_visible());
        assert_eq!(state.availability, UpdateAvailability::Unknown);
    }

    #[test]
    fn app_008_update_available_shows_badge() {
        let mut state = UpdateBadgeState::default();
        state.set_availability(UpdateAvailability::Available("0.4.0".into()));
        assert!(state.is_visible());
        assert_eq!(state.last_checked_version.as_deref(), Some("0.4.0"));
    }

    #[test]
    fn app_008_dismiss_hides_badge() {
        let mut state = UpdateBadgeState::default();
        state.set_availability(UpdateAvailability::Available("0.4.0".into()));
        assert!(state.is_visible());
        state.dismiss();
        assert!(!state.is_visible());
    }

    #[test]
    fn app_008_new_version_reshows_badge() {
        let mut state = UpdateBadgeState::default();
        state.set_availability(UpdateAvailability::Available("0.4.0".into()));
        state.dismiss();
        assert!(!state.is_visible());

        // Same version again — stays dismissed
        state.set_availability(UpdateAvailability::Available("0.4.0".into()));
        assert!(!state.is_visible());

        // New version — badge reappears
        state.set_availability(UpdateAvailability::Available("0.5.0".into()));
        assert!(state.is_visible());
    }

    #[test]
    fn app_008_failure_does_not_show_badge() {
        let mut state = UpdateBadgeState::default();
        state.set_availability(UpdateAvailability::Failed("Network error".into()));
        assert!(!state.is_visible());
    }

    #[test]
    fn app_008_up_to_date_no_badge() {
        let mut state = UpdateBadgeState::default();
        state.set_availability(UpdateAvailability::UpToDate);
        assert!(!state.is_visible());
    }

    #[test]
    fn app_008_serde_roundtrip() {
        let state = UpdateBadgeState {
            availability: UpdateAvailability::Available("0.5.0".into()),
            dismissed: true,
            last_checked_version: Some("0.5.0".into()),
        };
        let json = serde_json::to_string(&state).unwrap();
        let restored: UpdateBadgeState = serde_json::from_str(&json).unwrap();
        assert_eq!(state, restored);
    }
}

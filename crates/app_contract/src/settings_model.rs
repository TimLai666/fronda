//! Data types for the Settings view — pure logic, no gpui dependency.
//!
//! Covers SETUI-001 through SETUI-012.

use crate::settings_storage::{SettingsTab, SettingsVisibility};

/// Configuration for a settings window.
#[derive(Debug, Clone, PartialEq)]
pub struct SettingsWindowModel {
    pub active_tab: SettingsTab,
    pub settings_visibility: SettingsVisibility,
}

impl SettingsWindowModel {
    /// Create a new settings model with the specified active tab.
    pub fn new(active_tab: SettingsTab, backend_configured: bool) -> Self {
        let visibility = SettingsVisibility::for_backend_state(backend_configured);
        let tab = SettingsVisibility::fallback_tab(&active_tab, &visibility.visible_tabs);
        Self {
            active_tab: tab,
            settings_visibility: visibility,
        }
    }

    /// Switch to a tab, respecting visibility.
    pub fn switch_to(&mut self, tab: SettingsTab) {
        if self.settings_visibility.visible_tabs.contains(&tab) {
            self.active_tab = tab;
        }
    }

    /// Get the display title for the settings window.
    pub fn title(&self) -> &str {
        "Settings"
    }

    /// Check if a tab is currently visible.
    pub fn is_tab_visible(&self, tab: &SettingsTab) -> bool {
        self.settings_visibility.visible_tabs.contains(tab)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings_storage::SettingsTab;

    #[test]
    fn settings_model_created_with_visible_tab() {
        let model = SettingsWindowModel::new(SettingsTab::General, true);
        assert_eq!(model.active_tab, SettingsTab::General);
        assert_eq!(model.title(), "Settings");
    }

    #[test]
    fn settings_model_switches_tab() {
        let mut model = SettingsWindowModel::new(SettingsTab::General, true);
        model.switch_to(SettingsTab::Models);
        assert_eq!(model.active_tab, SettingsTab::Models);
    }

    #[test]
    fn settings_model_fallback_when_hidden() {
        let model = SettingsWindowModel::new(SettingsTab::Account, false);
        // Account tab hidden when backend not configured → fallback to first visible
        assert_ne!(model.active_tab, SettingsTab::Account);
    }

    #[test]
    fn settings_model_cannot_switch_to_hidden_tab() {
        let mut model = SettingsWindowModel::new(SettingsTab::General, false);
        model.switch_to(SettingsTab::Account);
        // Account should remain hidden, so switch should be rejected
        assert_ne!(model.active_tab, SettingsTab::Account);
    }
}

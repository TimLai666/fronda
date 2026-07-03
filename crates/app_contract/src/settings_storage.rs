//! Settings surface and persisted preferences.
//!
//! Covers SETUI-001 through SETUI-012.

use crate::telemetry::TELEMETRY_DEFAULT_ENABLED;

/// SETUI-001: Settings tab identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SettingsTab {
    Account,
    General,
    Models,
    Agent,
    Storage,
}

impl SettingsTab {
    pub const ALL: &'static [SettingsTab] = &[
        SettingsTab::Account,
        SettingsTab::General,
        SettingsTab::Models,
        SettingsTab::Agent,
        SettingsTab::Storage,
    ];

    /// Display name for each tab.
    pub fn label(&self) -> &'static str {
        match self {
            SettingsTab::Account => "Account",
            SettingsTab::General => "General",
            SettingsTab::Models => "Models",
            SettingsTab::Agent => "Agent",
            SettingsTab::Storage => "Storage",
        }
    }

    /// SF Symbol icon name for each tab (SETUI-001).
    pub fn icon(&self) -> &'static str {
        match self {
            SettingsTab::Account => "person.circle",
            SettingsTab::General => "gearshape",
            SettingsTab::Models => "square.stack.3d.up",
            SettingsTab::Agent => "paperplane",
            SettingsTab::Storage => "internaldrive",
        }
    }

    /// Whether this tab requires valid backend configuration (SETUI-002).
    pub fn requires_backend(&self) -> bool {
        matches!(self, SettingsTab::Account)
    }
}

/// SETUI-002: Account tab visibility.
#[derive(Debug, Clone, PartialEq)]
pub struct SettingsVisibility {
    pub visible_tabs: Vec<SettingsTab>,
}

impl SettingsVisibility {
    /// Build visible tabs based on backend configuration.
    pub fn for_backend_state(backend_configured: bool) -> Self {
        let all = SettingsTab::ALL.to_vec();
        let visible = if backend_configured {
            all
        } else {
            all.into_iter().filter(|t| !t.requires_backend()).collect()
        };
        Self {
            visible_tabs: visible,
        }
    }

    /// SETUI-003: Fallback tab when currently selected becomes hidden.
    pub fn fallback_tab(current: &SettingsTab, visible: &[SettingsTab]) -> SettingsTab {
        if visible.contains(current) {
            *current
        } else {
            visible.first().copied().unwrap_or(SettingsTab::General)
        }
    }
}

/// SETUI-004: General settings sections.
#[derive(Debug, Clone, PartialEq)]
pub enum GeneralSettingsSection {
    Notifications,
    Privacy,
}

/// SETUI-005: Model category for enabled-model toggles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModelCategory {
    Image,
    Video,
    Audio,
}

/// SETUI-006: Model catalog loading states.
#[derive(Debug, Clone, PartialEq)]
pub enum ModelCatalogState {
    Loading,
    Loaded { model_count: usize },
    FilteredEmpty { query: String },
    Error(String),
}

impl ModelCatalogState {
    pub fn display_message(&self) -> String {
        match self {
            ModelCatalogState::Loading => "Loading models…".into(),
            ModelCatalogState::Loaded { model_count } => {
                format!("{model_count} models available")
            }
            ModelCatalogState::FilteredEmpty { query } => {
                format!("No models match \"{query}\".")
            }
            ModelCatalogState::Error(msg) => msg.clone(),
        }
    }
}

/// MCP enabled preference key (SETUI-011).
pub const MCP_ENABLED_KEY: &str = "io.palmier.pro.mcp.enabled";

/// MCP default state (SETUI-011): enabled when absent.
pub const MCP_DEFAULT_ENABLED: bool = true;

/// Default MCP port.
pub const MCP_DEFAULT_PORT: u16 = 19789;

/// Agent model preference key (SETUI-012).
pub const AGENT_MODEL_KEY: &str = "agentModel";

/// Default agent model id (SETUI-012).
/// Upstream palmier-pro #243: Sonnet 4.6 → Sonnet 5.
pub const AGENT_DEFAULT_MODEL: &str = "sonnet5";

/// Storage preference key constants.
pub struct StoragePreferenceKeys;
impl StoragePreferenceKeys {
    pub const ON_DEVICE_MEDIA_SEARCH: &'static str = "io.palmier.pro.search.onDeviceMediaSearch";
    pub const SEARCH_INDEX_VERSION: &'static str = "io.palmier.pro.search.indexVersion";
}

/// Settings state container for all persisted preferences.
#[derive(Debug, Clone, PartialEq)]
pub struct SettingsState {
    pub notifications_enabled: bool,
    pub telemetry_enabled: bool,
    pub mcp_enabled: bool,
    pub agent_model: String,
    pub disabled_models: Vec<String>,
}

impl Default for SettingsState {
    fn default() -> Self {
        Self {
            notifications_enabled: true,
            telemetry_enabled: TELEMETRY_DEFAULT_ENABLED,
            mcp_enabled: MCP_DEFAULT_ENABLED,
            agent_model: AGENT_DEFAULT_MODEL.into(),
            disabled_models: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // SETUI-001
    #[test]
    fn settings_tab_count() {
        assert_eq!(SettingsTab::ALL.len(), 5);
    }

    #[test]
    fn settings_tab_labels_and_icons() {
        assert_eq!(SettingsTab::Account.label(), "Account");
        assert_eq!(SettingsTab::Account.icon(), "person.circle");
        assert_eq!(SettingsTab::General.icon(), "gearshape");
        assert_eq!(SettingsTab::Models.icon(), "square.stack.3d.up");
        assert_eq!(SettingsTab::Agent.icon(), "paperplane");
        assert_eq!(SettingsTab::Storage.icon(), "internaldrive");
    }

    // SETUI-002
    #[test]
    fn account_tab_hidden_when_backend_misconfigured() {
        let vis = SettingsVisibility::for_backend_state(false);
        assert!(!vis.visible_tabs.contains(&SettingsTab::Account));
        assert!(vis.visible_tabs.contains(&SettingsTab::General));
    }

    #[test]
    fn all_tabs_visible_when_backend_configured() {
        let vis = SettingsVisibility::for_backend_state(true);
        assert_eq!(vis.visible_tabs.len(), 5);
    }

    // SETUI-003
    #[test]
    fn fallback_tab_preserves_current_when_visible() {
        let visible = vec![SettingsTab::General, SettingsTab::Storage];
        let result = SettingsVisibility::fallback_tab(&SettingsTab::Storage, &visible);
        assert_eq!(result, SettingsTab::Storage);
    }

    #[test]
    fn fallback_tab_chooses_first_visible() {
        let visible = vec![SettingsTab::General, SettingsTab::Models];
        let result = SettingsVisibility::fallback_tab(&SettingsTab::Account, &visible);
        assert_eq!(result, SettingsTab::General);
    }

    // SETUI-006
    #[test]
    fn model_catalog_loading_message() {
        let state = ModelCatalogState::Loading;
        assert_eq!(state.display_message(), "Loading models…");
    }

    #[test]
    fn model_catalog_filtered_empty_message() {
        let state = ModelCatalogState::FilteredEmpty {
            query: "xyz".into(),
        };
        assert_eq!(state.display_message(), "No models match \"xyz\".");
    }

    // SETUI-011
    #[test]
    fn mcp_key_name_stable() {
        assert_eq!(MCP_ENABLED_KEY, "io.palmier.pro.mcp.enabled");
    }

    // SETUI-012
    #[test]
    fn agent_model_defaults() {
        assert_eq!(AGENT_MODEL_KEY, "agentModel");
        assert_eq!(AGENT_DEFAULT_MODEL, "sonnet5");
    }

    // Default settings
    #[test]
    fn settings_state_default() {
        let state = SettingsState::default();
        assert!(state.notifications_enabled);
        assert!(state.telemetry_enabled);
        assert!(state.mcp_enabled);
        assert_eq!(state.agent_model, "sonnet5");
        assert!(state.disabled_models.is_empty());
    }
}

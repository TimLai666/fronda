//! Data types for the Help view — pure logic, no gpui dependency.
//!
//! Covers HELP-001 through HELP-006.

/// Help tab identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HelpTab {
    Shortcuts,
    Mcp,
}

impl HelpTab {
    pub const ALL: &'static [HelpTab] = &[HelpTab::Shortcuts, HelpTab::Mcp];

    pub fn label(&self) -> &'static str {
        match self {
            HelpTab::Shortcuts => "Shortcuts",
            HelpTab::Mcp => "MCP",
        }
    }
}

/// Model for the Help view.
#[derive(Debug, Clone, PartialEq)]
pub struct HelpViewModel {
    pub active_tab: HelpTab,
    pub mcp_port: u16,
}

impl HelpViewModel {
    pub fn new(mcp_port: u16) -> Self {
        Self {
            active_tab: HelpTab::Shortcuts,
            mcp_port,
        }
    }

    pub fn switch_to(&mut self, tab: HelpTab) {
        self.active_tab = tab;
    }

    pub fn mcp_server_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.mcp_port)
    }

    pub fn mcp_endpoint(&self) -> String {
        format!("http://127.0.0.1:{}/mcp", self.mcp_port)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_tab_labels() {
        assert_eq!(HelpTab::Shortcuts.label(), "Shortcuts");
        assert_eq!(HelpTab::Mcp.label(), "MCP");
    }

    #[test]
    fn help_view_model_default_tab() {
        let model = HelpViewModel::new(19789);
        assert_eq!(model.active_tab, HelpTab::Shortcuts);
        assert_eq!(model.mcp_port, 19789);
    }

    #[test]
    fn help_view_switch_tab() {
        let mut model = HelpViewModel::new(19789);
        model.switch_to(HelpTab::Mcp);
        assert_eq!(model.active_tab, HelpTab::Mcp);
    }

    #[test]
    fn help_mcp_urls() {
        let model = HelpViewModel::new(19789);
        assert_eq!(model.mcp_server_url(), "http://127.0.0.1:19789");
        assert_eq!(model.mcp_endpoint(), "http://127.0.0.1:19789/mcp");
    }

    #[test]
    fn help_mcp_urls_custom_port() {
        let model = HelpViewModel::new(9000);
        assert_eq!(model.mcp_server_url(), "http://127.0.0.1:9000");
        assert_eq!(model.mcp_endpoint(), "http://127.0.0.1:9000/mcp");
    }
}

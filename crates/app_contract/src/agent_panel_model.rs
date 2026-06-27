//! Agent panel model — pure logic for the Agent panel state.
//!
//! Covers agent panel UI state (panel visibility, mention picker, etc.).

/// MCP server status.
#[derive(Debug, Clone, PartialEq)]
pub enum McpServerStatus {
    Starting,
    Running { port: u16 },
    Stopped,
    Failed(String),
}

/// Agent panel state.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentPanelModel {
    pub mcp_status: McpServerStatus,
    pub mention_query: String,
    pub show_mention_picker: bool,
    pub available_tools: Vec<String>,
}

impl Default for AgentPanelModel {
    fn default() -> Self {
        Self {
            mcp_status: McpServerStatus::Stopped,
            mention_query: String::new(),
            show_mention_picker: false,
            available_tools: Vec::new(),
        }
    }
}

impl AgentPanelModel {
    pub fn set_mcp_running(&mut self, port: u16) {
        self.mcp_status = McpServerStatus::Running { port };
    }

    pub fn set_mcp_stopped(&mut self) {
        self.mcp_status = McpServerStatus::Stopped;
    }

    pub fn mcp_status_label(&self) -> String {
        match &self.mcp_status {
            McpServerStatus::Starting => "Starting…".into(),
            McpServerStatus::Running { port } => format!("Running on 127.0.0.1:{port}"),
            McpServerStatus::Stopped => "Stopped".into(),
            McpServerStatus::Failed(reason) => format!("Failed: {reason}"),
        }
    }

    pub fn toggle_mention_picker(&mut self) {
        self.show_mention_picker = !self.show_mention_picker;
        if !self.show_mention_picker {
            self.mention_query.clear();
        }
    }

    pub fn update_mention_query(&mut self, query: String) {
        self.mention_query = query;
        self.show_mention_picker = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_default_stopped() {
        let model = AgentPanelModel::default();
        assert_eq!(model.mcp_status, McpServerStatus::Stopped);
        assert_eq!(model.mcp_status_label(), "Stopped");
    }

    #[test]
    fn agent_mcp_running() {
        let mut model = AgentPanelModel::default();
        model.set_mcp_running(19789);
        assert_eq!(model.mcp_status_label(), "Running on 127.0.0.1:19789");
    }

    #[test]
    fn agent_mcp_failed() {
        let status = McpServerStatus::Failed("port in use".into());
        let model = AgentPanelModel {
            mcp_status: status,
            ..Default::default()
        };
        assert_eq!(model.mcp_status_label(), "Failed: port in use");
    }

    #[test]
    fn agent_mention_toggle() {
        let mut model = AgentPanelModel::default();
        assert!(!model.show_mention_picker);
        model.toggle_mention_picker();
        assert!(model.show_mention_picker);
        model.toggle_mention_picker();
        assert!(!model.show_mention_picker);
    }
}

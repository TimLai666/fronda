//! Help tabs, MCP instructions, and feedback UX.
//!
//! Covers HELP-001 through HELP-006.

/// HELP-001..006: Help content and MCP instruction snippets.
pub struct HelpContent;

impl HelpContent {
    /// HELP-001: Help tabs.
    pub const TABS: &'static [&'static str] = &["Shortcuts", "MCP"];

    /// HELP-002: MCP server URL.
    pub fn mcp_server_url(port: u16) -> String {
        format!("http://127.0.0.1:{port}")
    }

    /// HELP-002: MCP endpoint.
    pub fn mcp_endpoint(port: u16) -> String {
        format!("http://127.0.0.1:{port}/mcp")
    }

    /// HELP-003: Copyable Claude Code command.
    pub fn claude_code_command(endpoint: &str) -> String {
        format!("claude mcp add --transport http palmier-pro {endpoint}")
    }

    /// HELP-004: Copyable Codex command.
    pub fn codex_command(endpoint: &str) -> String {
        format!("codex mcp add palmier-pro --url {endpoint}")
    }

    /// HELP-005: Cursor JSON config block.
    pub fn cursor_json_config(endpoint: &str) -> String {
        format!(r#"{{"mcpServers":{{"palmier-pro":{{"type":"http","url":"{endpoint}"}}}}}}"#)
    }

    /// HELP-006: Claude Desktop bundled extension identifier.
    pub const CLAUDE_DESKTOP_EXTENSION: &'static str = "palmier-pro.mcpb";

    const DEFAULT_PORT: u16 = 19789;

    /// Convenience: default endpoint for standard port.
    pub fn default_endpoint() -> String {
        Self::mcp_endpoint(Self::DEFAULT_PORT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_tabs() {
        assert_eq!(HelpContent::TABS, &["Shortcuts", "MCP"]);
    }

    #[test]
    fn help_mcp_server_url() {
        let url = HelpContent::mcp_server_url(19789);
        assert_eq!(url, "http://127.0.0.1:19789");
    }

    #[test]
    fn help_mcp_endpoint() {
        let ep = HelpContent::mcp_endpoint(19789);
        assert_eq!(ep, "http://127.0.0.1:19789/mcp");
    }

    #[test]
    fn help_mcp_endpoint_custom_port() {
        let ep = HelpContent::mcp_endpoint(9000);
        assert_eq!(ep, "http://127.0.0.1:9000/mcp");
    }

    #[test]
    fn help_claude_code_command() {
        let cmd = HelpContent::claude_code_command("http://127.0.0.1:19789/mcp");
        assert_eq!(
            cmd,
            "claude mcp add --transport http palmier-pro http://127.0.0.1:19789/mcp"
        );
    }

    #[test]
    fn help_codex_command() {
        let cmd = HelpContent::codex_command("http://127.0.0.1:19789/mcp");
        assert_eq!(
            cmd,
            "codex mcp add palmier-pro --url http://127.0.0.1:19789/mcp"
        );
    }

    #[test]
    fn help_cursor_config() {
        let config = HelpContent::cursor_json_config("http://127.0.0.1:19789/mcp");
        assert!(config.contains("palmier-pro"));
        assert!(config.contains("http"));
        assert!(config.contains("mcpServers"));
    }

    #[test]
    fn help_claude_desktop_extension() {
        assert_eq!(HelpContent::CLAUDE_DESKTOP_EXTENSION, "palmier-pro.mcpb");
    }

    #[test]
    fn help_default_endpoint() {
        let ep = HelpContent::default_endpoint();
        assert_eq!(ep, "http://127.0.0.1:19789/mcp");
    }
}

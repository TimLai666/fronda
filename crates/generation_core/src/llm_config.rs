//! LLM provider configuration (Issues #17, #140, #142).
//!
//! Supports Anthropic Claude (default), DeepSeek, Codex CLI (local),
//! and any OpenAI-compatible API via a custom base URL.

use serde::{Deserialize, Serialize};

/// LLM provider selection (Issues #140, #142).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum LlmProvider {
    /// Anthropic Claude API (default, cloud).
    #[default]
    Anthropic,
    /// DeepSeek API (OpenAI-compatible, Issue #140).
    DeepSeek,
    /// Local Codex CLI agent (Issue #142).
    CodexCli,
    /// Any OpenAI-compatible API with a custom base URL (Issue #17).
    OpenAiCompatible,
}

impl LlmProvider {
    /// Default base URL for this provider (None = use the library default).
    pub fn default_base_url(&self) -> Option<&'static str> {
        match self {
            LlmProvider::Anthropic => None,
            LlmProvider::DeepSeek => Some("https://api.deepseek.com/v1"),
            LlmProvider::CodexCli => None, // local process, no URL
            LlmProvider::OpenAiCompatible => None,
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            LlmProvider::Anthropic => "Anthropic Claude",
            LlmProvider::DeepSeek => "DeepSeek",
            LlmProvider::CodexCli => "Codex CLI (local)",
            LlmProvider::OpenAiCompatible => "Custom (OpenAI-compatible)",
        }
    }
}

/// Complete LLM configuration for the generation pipeline (Issues #17, #140, #142).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmConfig {
    pub provider: LlmProvider,
    /// Override the provider's default API base URL (Issue #17).
    /// Required when `provider` is `OpenAiCompatible`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_base_url: Option<String>,
    /// API key for cloud providers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    /// Model identifier string (e.g. "claude-opus-4-8", "deepseek-chat").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            provider: LlmProvider::Anthropic,
            api_base_url: None,
            api_key: None,
            model_id: None,
        }
    }
}

impl LlmConfig {
    pub fn anthropic(api_key: impl Into<String>) -> Self {
        Self {
            provider: LlmProvider::Anthropic,
            api_key: Some(api_key.into()),
            ..Default::default()
        }
    }

    /// Issue #140: DeepSeek provider preset.
    pub fn deepseek(api_key: impl Into<String>) -> Self {
        Self {
            provider: LlmProvider::DeepSeek,
            api_base_url: LlmProvider::DeepSeek.default_base_url().map(String::from),
            api_key: Some(api_key.into()),
            model_id: Some("deepseek-chat".into()),
        }
    }

    /// Issue #142: Codex CLI local agent.
    pub fn codex_cli() -> Self {
        Self {
            provider: LlmProvider::CodexCli,
            api_base_url: None,
            api_key: None,
            model_id: None,
        }
    }

    /// Issue #17: Custom OpenAI-compatible endpoint.
    pub fn custom(base_url: impl Into<String>, api_key: impl Into<String>) -> Self {
        Self {
            provider: LlmProvider::OpenAiCompatible,
            api_base_url: Some(base_url.into()),
            api_key: Some(api_key.into()),
            model_id: None,
        }
    }

    /// The effective base URL — provider default if `api_base_url` is not set.
    pub fn effective_base_url(&self) -> Option<&str> {
        self.api_base_url
            .as_deref()
            .or_else(|| self.provider.default_base_url())
    }

    /// Returns `Err` if the config is incomplete for the selected provider.
    pub fn validate(&self) -> Result<(), String> {
        match self.provider {
            LlmProvider::OpenAiCompatible => {
                if self.api_base_url.is_none() {
                    return Err("OpenAI-compatible provider requires api_base_url to be set".into());
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Issue #17: Custom API base URL

    #[test]
    fn issue_017_custom_provider_requires_base_url() {
        let cfg = LlmConfig {
            provider: LlmProvider::OpenAiCompatible,
            api_base_url: None,
            api_key: Some("key".into()),
            model_id: None,
        };
        assert!(cfg.validate().is_err(), "missing base_url must be an error");
    }

    #[test]
    fn issue_017_custom_provider_with_base_url_is_valid() {
        let cfg = LlmConfig::custom("http://localhost:4891/v1", "no-key");
        assert!(cfg.validate().is_ok());
        assert_eq!(cfg.effective_base_url(), Some("http://localhost:4891/v1"));
    }

    #[test]
    fn issue_017_api_base_url_overrides_provider_default() {
        let mut cfg = LlmConfig::deepseek("sk-...");
        cfg.api_base_url = Some("http://proxy.local/v1".into());
        assert_eq!(cfg.effective_base_url(), Some("http://proxy.local/v1"));
    }

    #[test]
    fn issue_017_serde_roundtrip() {
        let cfg = LlmConfig::custom("http://proxy/v1", "key");
        let json = serde_json::to_string(&cfg).unwrap();
        let restored: LlmConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.api_base_url.as_deref(), Some("http://proxy/v1"));
        assert_eq!(restored.provider, LlmProvider::OpenAiCompatible);
    }

    // Issue #140: Multi-provider (DeepSeek, custom OpenAI-compatible)

    #[test]
    fn issue_140_deepseek_preset_has_correct_url() {
        let cfg = LlmConfig::deepseek("sk-deepseek-xxx");
        assert_eq!(
            cfg.effective_base_url(),
            Some("https://api.deepseek.com/v1")
        );
        assert_eq!(cfg.model_id.as_deref(), Some("deepseek-chat"));
    }

    #[test]
    fn issue_140_deepseek_validate_passes() {
        let cfg = LlmConfig::deepseek("sk-xxx");
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn issue_140_provider_display_names_non_empty() {
        for p in [
            LlmProvider::Anthropic,
            LlmProvider::DeepSeek,
            LlmProvider::CodexCli,
            LlmProvider::OpenAiCompatible,
        ] {
            assert!(!p.display_name().is_empty(), "{p:?} has no display name");
        }
    }

    #[test]
    fn issue_140_anthropic_has_no_default_url() {
        assert!(LlmProvider::Anthropic.default_base_url().is_none());
    }

    // Issue #142: Codex CLI local agent

    #[test]
    fn issue_142_codex_cli_provider_no_url() {
        let cfg = LlmConfig::codex_cli();
        assert_eq!(cfg.provider, LlmProvider::CodexCli);
        assert!(cfg.effective_base_url().is_none());
        assert!(cfg.api_key.is_none());
    }

    #[test]
    fn issue_142_codex_cli_validate_passes() {
        let cfg = LlmConfig::codex_cli();
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn issue_142_default_provider_is_anthropic() {
        let cfg = LlmConfig::default();
        assert_eq!(cfg.provider, LlmProvider::Anthropic);
    }

    // Issue #122: McpConfig network access (tested in mcp_server)
    // These tests live in crates/mcp_server/tests/spec_mcp_contract.rs
}

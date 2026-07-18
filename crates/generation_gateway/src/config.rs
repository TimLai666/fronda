//! Gateway configuration, resolved from the environment.
//!
//!   FRONDA_GEN_GATEWAY_ADDR         — bind address (default 127.0.0.1:8787)
//!   FRONDA_GEN_GATEWAY_TOKEN        — bearer token (optional; unset → auth disabled)
//!   FRONDA_GEN_GATEWAY_PUBLIC_BASE  — externally reachable base URL for result URLs
//!                                     (default http://<bind_addr>)
//!   FRONDA_GEN_KEY_<PROVIDER>       — per-provider BYO key (e.g. FRONDA_GEN_KEY_GEMINI)
//!   FRONDA_GEN_DEFAULT_<KIND>       — override a kind's default provider (video/image/audio)
//!   FRONDA_GEN_GEMINI_MODEL         — Gemini model id override
//!   FRONDA_GEN_GEMINI_BASE          — Gemini API base URL override
//!   FRONDA_GEN_GEMINI_API_VERSION   — Gemini API version override
//!
//! Stub mode needs no key, so a bare environment still yields a runnable config.

use std::collections::HashMap;

use crate::provider::ProviderKind;

pub const ADDR_ENV: &str = "FRONDA_GEN_GATEWAY_ADDR";
pub const TOKEN_ENV: &str = "FRONDA_GEN_GATEWAY_TOKEN";
pub const PUBLIC_BASE_ENV: &str = "FRONDA_GEN_GATEWAY_PUBLIC_BASE";
pub const KEY_PREFIX: &str = "FRONDA_GEN_KEY_";
pub const DEFAULT_PREFIX: &str = "FRONDA_GEN_DEFAULT_";
pub const GEMINI_MODEL_ENV: &str = "FRONDA_GEN_GEMINI_MODEL";
pub const GEMINI_BASE_ENV: &str = "FRONDA_GEN_GEMINI_BASE";
pub const GEMINI_API_VERSION_ENV: &str = "FRONDA_GEN_GEMINI_API_VERSION";
pub const DEFAULT_ADDR: &str = "127.0.0.1:8787";

#[derive(Debug, Clone)]
pub struct GatewayConfig {
    pub bind_addr: String,
    pub auth_token: Option<String>,
    /// Externally reachable base URL used to build result URLs
    /// (`{public_base}/v1/results/{id}`). `None` → derived from `bind_addr`.
    pub public_base: Option<String>,
    /// Per-provider BYO keys, keyed by lowercase provider name. Held for phase-2
    /// adapters; the stub ignores them.
    pub provider_keys: HashMap<String, String>,
    /// Optional per-kind default-provider overrides.
    pub default_providers: HashMap<ProviderKind, String>,
    /// Gemini connection overrides (model/base/api version); `None` → defaults.
    pub gemini_model: Option<String>,
    pub gemini_base: Option<String>,
    pub gemini_api_version: Option<String>,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            bind_addr: DEFAULT_ADDR.to_string(),
            auth_token: None,
            public_base: None,
            provider_keys: HashMap::new(),
            default_providers: HashMap::new(),
            gemini_model: None,
            gemini_base: None,
            gemini_api_version: None,
        }
    }
}

impl GatewayConfig {
    /// Resolve from the process environment.
    pub fn from_env() -> Self {
        Self::from_vars(std::env::vars())
    }

    /// Pure resolution from an iterator of (key, value) pairs — factored out of
    /// process-global env so it is unit-testable.
    pub fn from_vars(vars: impl Iterator<Item = (String, String)>) -> Self {
        let mut config = GatewayConfig::default();
        for (key, value) in vars {
            if key == ADDR_ENV {
                let trimmed = value.trim();
                if !trimmed.is_empty() {
                    config.bind_addr = trimmed.to_string();
                }
            } else if key == TOKEN_ENV {
                let trimmed = value.trim();
                if !trimmed.is_empty() {
                    config.auth_token = Some(trimmed.to_string());
                }
            } else if key == PUBLIC_BASE_ENV {
                let trimmed = value.trim();
                if !trimmed.is_empty() {
                    config.public_base = Some(trimmed.to_string());
                }
            } else if key == GEMINI_MODEL_ENV {
                let trimmed = value.trim();
                if !trimmed.is_empty() {
                    config.gemini_model = Some(trimmed.to_string());
                }
            } else if key == GEMINI_BASE_ENV {
                let trimmed = value.trim();
                if !trimmed.is_empty() {
                    config.gemini_base = Some(trimmed.to_string());
                }
            } else if key == GEMINI_API_VERSION_ENV {
                let trimmed = value.trim();
                if !trimmed.is_empty() {
                    config.gemini_api_version = Some(trimmed.to_string());
                }
            } else if let Some(provider) = key.strip_prefix(KEY_PREFIX) {
                if !provider.is_empty() {
                    config
                        .provider_keys
                        .insert(provider.to_ascii_lowercase(), value);
                }
            } else if let Some(kind_token) = key.strip_prefix(DEFAULT_PREFIX) {
                if let Some(kind) = ProviderKind::from_token(&kind_token.to_ascii_lowercase()) {
                    let trimmed = value.trim();
                    if !trimmed.is_empty() {
                        config
                            .default_providers
                            .insert(kind, trimmed.to_ascii_lowercase());
                    }
                }
            }
        }
        config
    }

    /// BYO key for a provider (case-insensitive), if configured.
    pub fn provider_key(&self, name: &str) -> Option<&str> {
        self.provider_keys
            .get(&name.to_ascii_lowercase())
            .map(String::as_str)
    }

    /// Externally reachable base URL for result URLs. Explicit `public_base` when
    /// set, otherwise `http://<bind_addr>`.
    pub fn public_base(&self) -> String {
        self.public_base
            .clone()
            .unwrap_or_else(|| format!("http://{}", self.bind_addr))
    }

    /// True when the bind address targets loopback. A network bind without a
    /// token is a security risk (main.rs warns).
    pub fn is_loopback(&self) -> bool {
        let host = self
            .bind_addr
            .rsplit_once(':')
            .map(|(h, _)| h)
            .unwrap_or(&self.bind_addr);
        host == "127.0.0.1" || host == "localhost" || host == "::1" || host == "[::1]"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars(pairs: &[(&str, &str)]) -> impl Iterator<Item = (String, String)> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect::<Vec<_>>()
            .into_iter()
    }

    #[test]
    fn empty_env_yields_runnable_defaults() {
        let config = GatewayConfig::from_vars(vars(&[]));
        assert_eq!(config.bind_addr, DEFAULT_ADDR);
        assert!(config.auth_token.is_none());
        assert!(config.provider_keys.is_empty());
        assert!(config.default_providers.is_empty());
        assert!(config.is_loopback());
    }

    #[test]
    fn parses_addr_token_keys_and_defaults() {
        let config = GatewayConfig::from_vars(vars(&[
            (ADDR_ENV, "0.0.0.0:9000"),
            (TOKEN_ENV, "secret"),
            ("FRONDA_GEN_KEY_GEMINI", "g-key"),
            ("FRONDA_GEN_KEY_Fal", "fal-key"),
            ("FRONDA_GEN_DEFAULT_VIDEO", "gemini"),
            ("UNRELATED", "ignored"),
        ]));
        assert_eq!(config.bind_addr, "0.0.0.0:9000");
        assert_eq!(config.auth_token.as_deref(), Some("secret"));
        assert_eq!(config.provider_key("gemini"), Some("g-key"));
        assert_eq!(config.provider_key("GEMINI"), Some("g-key"));
        assert_eq!(config.provider_key("fal"), Some("fal-key"));
        assert_eq!(
            config.default_providers.get(&ProviderKind::Video),
            Some(&"gemini".to_string())
        );
        assert!(!config.is_loopback());
    }

    #[test]
    fn blank_token_stays_none() {
        let config = GatewayConfig::from_vars(vars(&[(TOKEN_ENV, "   ")]));
        assert!(config.auth_token.is_none());
    }

    #[test]
    fn unknown_default_kind_is_ignored() {
        let config = GatewayConfig::from_vars(vars(&[("FRONDA_GEN_DEFAULT_UPSCALE", "x")]));
        assert!(config.default_providers.is_empty());
    }

    #[test]
    fn public_base_defaults_to_bind_addr() {
        let config = GatewayConfig::from_vars(vars(&[]));
        assert_eq!(config.public_base(), format!("http://{DEFAULT_ADDR}"));
    }

    #[test]
    fn public_base_override_wins() {
        let config = GatewayConfig::from_vars(vars(&[(
            PUBLIC_BASE_ENV,
            "https://gen.example.com",
        )]));
        assert_eq!(config.public_base(), "https://gen.example.com");
    }

    #[test]
    fn parses_gemini_overrides() {
        let config = GatewayConfig::from_vars(vars(&[
            (GEMINI_MODEL_ENV, "gemini-3-image"),
            (GEMINI_BASE_ENV, "http://127.0.0.1:1234"),
            (GEMINI_API_VERSION_ENV, "v1"),
        ]));
        assert_eq!(config.gemini_model.as_deref(), Some("gemini-3-image"));
        assert_eq!(config.gemini_base.as_deref(), Some("http://127.0.0.1:1234"));
        assert_eq!(config.gemini_api_version.as_deref(), Some("v1"));
    }
}

//! Backend configuration contract.
//!
//! Covers CFG-001 through CFG-004.

/// CFG-001..004: Backend configuration from bundle info dictionary.
///
/// In the Swift baseline, these are read from `Bundle.main.infoDictionary`.
/// In Rust, they must be injected by the platform layer.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BackendConfig {
    /// PalmierClerkPublishableKey from bundle info (CFG-001).
    pub clerk_publishable_key: Option<String>,
    /// PalmierConvexDeploymentURL from bundle info (CFG-001).
    pub convex_deployment_url: Option<String>,
    /// PalmierConvexHttpURL from bundle info (CFG-001). Optional.
    pub convex_http_url: Option<String>,
}

impl BackendConfig {
    /// Bundle info dictionary keys used by the Swift baseline.
    pub const CLERK_KEY: &'static str = "PalmierClerkPublishableKey";
    pub const CONVEX_DEPLOYMENT_URL_KEY: &'static str = "PalmierConvexDeploymentURL";
    pub const CONVEX_HTTP_URL_KEY: &'static str = "PalmierConvexHttpURL";

    /// Create a config from raw key-value strings.
    pub fn from_raw(raw: &[(&str, &str)]) -> Self {
        let map: std::collections::HashMap<&str, &str> = raw.iter().cloned().collect();
        Self {
            clerk_publishable_key: map
                .get(Self::CLERK_KEY)
                .filter(|v| !v.is_empty())
                .map(|s| s.to_string()),
            convex_deployment_url: map
                .get(Self::CONVEX_DEPLOYMENT_URL_KEY)
                .filter(|v| !v.is_empty())
                .map(|s| s.to_string()),
            convex_http_url: map
                .get(Self::CONVEX_HTTP_URL_KEY)
                .filter(|v| !v.is_empty())
                .map(|s| s.to_string()),
        }
    }

    /// CFG-002: Empty strings are treated as missing.
    #[allow(dead_code)]
    fn non_empty(val: Option<&str>) -> Option<String> {
        val.filter(|v| !v.is_empty()).map(|s| s.to_string())
    }

    /// CFG-003: Backend is configured only when Clerk key AND Convex deployment URL are present.
    pub fn is_configured(&self) -> bool {
        self.clerk_publishable_key.is_some() && self.convex_deployment_url.is_some()
    }

    /// CFG-004: Convex HTTP URL may be absent without making the whole backend misconfigured,
    /// but features requiring it should fail cleanly.
    pub fn has_http_url(&self) -> bool {
        self.convex_http_url.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // CFG-001
    #[test]
    fn cfg_keys_are_correct() {
        assert_eq!(BackendConfig::CLERK_KEY, "PalmierClerkPublishableKey");
        assert_eq!(
            BackendConfig::CONVEX_DEPLOYMENT_URL_KEY,
            "PalmierConvexDeploymentURL"
        );
        assert_eq!(BackendConfig::CONVEX_HTTP_URL_KEY, "PalmierConvexHttpURL");
    }

    #[test]
    fn cfg_from_raw_populates_all() {
        let raw = &[
            ("PalmierClerkPublishableKey", "pk_test_123"),
            ("PalmierConvexDeploymentURL", "https://example.convex.cloud"),
            ("PalmierConvexHttpURL", "https://example.convex.cloud/http"),
        ];
        let config = BackendConfig::from_raw(raw);
        assert_eq!(config.clerk_publishable_key.as_deref(), Some("pk_test_123"));
        assert_eq!(
            config.convex_deployment_url.as_deref(),
            Some("https://example.convex.cloud")
        );
        assert_eq!(
            config.convex_http_url.as_deref(),
            Some("https://example.convex.cloud/http")
        );
    }

    #[test]
    fn cfg_from_raw_partial() {
        let raw = &[("PalmierClerkPublishableKey", "pk_test_123")];
        let config = BackendConfig::from_raw(raw);
        assert_eq!(config.clerk_publishable_key.as_deref(), Some("pk_test_123"));
        assert_eq!(config.convex_deployment_url, None);
        assert_eq!(config.convex_http_url, None);
    }

    // CFG-002
    #[test]
    fn cfg_empty_strings_treated_as_missing() {
        let raw = &[
            ("PalmierClerkPublishableKey", ""),
            ("PalmierConvexDeploymentURL", "https://example.convex.cloud"),
        ];
        let config = BackendConfig::from_raw(raw);
        assert_eq!(config.clerk_publishable_key, None);
        assert!(config.convex_deployment_url.is_some());
    }

    // CFG-003
    #[test]
    fn cfg_configured_when_both_keys_present() {
        let raw = &[
            ("PalmierClerkPublishableKey", "pk_test_123"),
            ("PalmierConvexDeploymentURL", "https://example.convex.cloud"),
        ];
        let config = BackendConfig::from_raw(raw);
        assert!(config.is_configured());
    }

    #[test]
    fn cfg_not_configured_when_clerk_missing() {
        let raw = &[("PalmierConvexDeploymentURL", "https://example.convex.cloud")];
        let config = BackendConfig::from_raw(raw);
        assert!(!config.is_configured());
    }

    #[test]
    fn cfg_not_configured_when_convex_missing() {
        let raw = &[("PalmierClerkPublishableKey", "pk_test_123")];
        let config = BackendConfig::from_raw(raw);
        assert!(!config.is_configured());
    }

    // CFG-004
    #[test]
    fn cfg_http_url_optional() {
        let raw = &[
            ("PalmierClerkPublishableKey", "pk_test_123"),
            ("PalmierConvexDeploymentURL", "https://example.convex.cloud"),
        ];
        let config = BackendConfig::from_raw(raw);
        assert!(config.is_configured());
        assert!(!config.has_http_url());
    }

    #[test]
    fn cfg_default_all_none() {
        let config = BackendConfig::default();
        assert!(!config.is_configured());
        assert_eq!(config.clerk_publishable_key, None);
        assert_eq!(config.convex_deployment_url, None);
        assert_eq!(config.convex_http_url, None);
    }
}

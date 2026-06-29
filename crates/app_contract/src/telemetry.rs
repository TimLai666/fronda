//! Telemetry configuration and event tracking.
//!
//! Covers TEL-001 through TEL-009. Pure configuration/contract — no platform
//! SDK dependency. The telemetry backend (Sentry etc.) is injected at the app layer.

/// TEL-001: Telemetry is enabled by default.
pub const TELEMETRY_DEFAULT_ENABLED: bool = true;

/// Key for persisting telemetry enabled state.
pub const TELEMETRY_ENABLED_KEY: &str = "io.palmier.pro.telemetry.enabled";

/// Key for reading Sentry DSN from bundle info dictionary.
pub const SENTRY_DSN_BUNDLE_KEY: &str = "SentryDSN";

/// TEL-002: Telemetry enabled state is latched for the current launch.
#[derive(Debug, Clone, PartialEq)]
pub struct TelemetryConfig {
    pub enabled: bool,
    pub dsn: String,
    pub environment: TelemetryEnvironment,
    pub traces_sample_rate: f64,
    pub app_hang_timeout_seconds: f64,
    pub attach_stacktrace: bool,
    pub enable_capture_failed_requests: bool,
    pub enable_uncaught_exception_reporting: bool,
    pub release_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TelemetryEnvironment {
    Development,
    Production,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            dsn: String::new(),
            environment: TelemetryEnvironment::Production,
            traces_sample_rate: 0.1,
            app_hang_timeout_seconds: 8.0,
            attach_stacktrace: true,
            enable_capture_failed_requests: false,
            enable_uncaught_exception_reporting: true,
            release_name: None,
        }
    }
}

/// TEL-003: Telemetry startup validation.
pub fn should_start_telemetry(config: &TelemetryConfig) -> bool {
    config.enabled && !config.dsn.is_empty()
}

/// TEL-004: Build a release name from version + build.
pub fn build_release_name(app_version: &str, build_number: &str) -> String {
    format!("palmier-pro@{app_version}+{build_number}")
}

/// TEL-005..006: Severity levels for log forwarding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Debug,
    Notice,
    Warning,
    Error,
    Fault,
}

impl LogLevel {
    /// Whether this level should create a breadcrumb (TEL-005) vs a full event (TEL-006).
    pub fn is_breadcrumb_level(&self) -> bool {
        matches!(self, LogLevel::Debug | LogLevel::Notice)
    }
}

/// TEL-008: Trace operation result.
#[derive(Debug, Clone, PartialEq)]
pub enum TraceResult<T> {
    Success(T),
    Failure(T, String),
}

/// TEL-009: Crash log path hint.
pub const CRASH_LOG_FILENAME: &str = "palmier-pro-crash.log";

/// TEL-007: Project-open telemetry context.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectTelemetryContext {
    pub track_count: usize,
    pub clip_count: usize,
    pub media_count: usize,
    pub generation_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    // TEL-002
    #[test]
    fn telemetry_config_default() {
        let config = TelemetryConfig::default();
        assert!(config.enabled);
        assert!(config.dsn.is_empty());
        assert_eq!(config.traces_sample_rate, 0.1);
        assert!((config.app_hang_timeout_seconds - 8.0).abs() < 1e-10);
        assert!(config.attach_stacktrace);
        assert!(config.enable_uncaught_exception_reporting);
    }

    // TEL-003
    #[test]
    fn should_start_when_enabled_and_dsn_present() {
        let config = TelemetryConfig {
            enabled: true,
            dsn: "https://key@sentry.io/123".into(),
            ..Default::default()
        };
        assert!(should_start_telemetry(&config));
    }

    #[test]
    fn should_not_start_when_disabled() {
        let config = TelemetryConfig {
            enabled: false,
            dsn: "https://key@sentry.io/123".into(),
            ..Default::default()
        };
        assert!(!should_start_telemetry(&config));
    }

    #[test]
    fn should_not_start_when_dsn_empty() {
        let config = TelemetryConfig {
            enabled: true,
            dsn: String::new(),
            ..Default::default()
        };
        assert!(!should_start_telemetry(&config));
    }

    // TEL-004
    #[test]
    fn release_name_format() {
        let name = build_release_name("0.3.5", "53");
        assert_eq!(name, "palmier-pro@0.3.5+53");
    }

    // TEL-005/006
    #[test]
    fn notice_level_is_breadcrumb() {
        assert!(LogLevel::Notice.is_breadcrumb_level());
        assert!(LogLevel::Debug.is_breadcrumb_level());
    }

    #[test]
    fn warning_level_is_not_breadcrumb() {
        assert!(!LogLevel::Warning.is_breadcrumb_level());
        assert!(!LogLevel::Error.is_breadcrumb_level());
        assert!(!LogLevel::Fault.is_breadcrumb_level());
    }

    // TEL-007
    #[test]
    fn project_context_defaults() {
        let ctx = ProjectTelemetryContext {
            track_count: 3,
            clip_count: 12,
            media_count: 8,
            generation_count: 2,
        };
        assert_eq!(ctx.track_count, 3);
        assert_eq!(ctx.clip_count, 12);
    }
}

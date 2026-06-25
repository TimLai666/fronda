//! Runtime envelope, package dependencies, and bundle/version metadata.
//!
//! Covers RUN-001 through RUN-007, PKG-001 through PKG-007,
//! and BNDL-001 through BNDL-013.

// ── RUN-001..007: Runtime baseline ────────────────────────────────

/// RUN-001: Current implementation baseline constants.
pub struct RuntimeBaseline;
impl RuntimeBaseline {
    pub const SWIFT_VERSION: &'static str = "6.2";
    pub const PACKAGE_NAME: &'static str = "PalmierPro";
    pub const EXECUTABLE: &'static str = "PalmierPro";
    pub const REWRITE_NAME: &'static str = "Fronda";
}

/// RUN-002: Supported platform.
pub const MINIMUM_MACOS_VERSION: &str = "26.0";
pub const TARGET_PLATFORM: &str = "macOS 26.0+";

/// RUN-003: Development prerequisites.
pub struct DevPrerequisites;
impl DevPrerequisites {
    pub const MACOS_VERSION: &str = "26+";
    pub const XCODE_VERSION: &str = "16+";
    pub const SWIFT_TOOLCHAIN: &str = "6.2";
}

/// RUN-004: Standard development commands.
pub struct DevCommands;
impl DevCommands {
    pub const BUILD: &'static str = "swift build";
    pub const RUN: &'static str = "swift run";
    pub const DEV_SCRIPT: &'static str = "./scripts/dev.sh";
}

// ── PKG-001..007: Package dependencies ────────────────────────────

/// PKG-001: A Swift package dependency from the baseline.
#[derive(Debug, Clone, PartialEq)]
pub struct SwiftPackageDependency {
    pub name: &'static str,
    pub version: &'static str,
}

/// PKG-001: The full list of Swift package dependencies.
pub fn swift_dependencies() -> Vec<SwiftPackageDependency> {
    vec![
        SwiftPackageDependency {
            name: "DSWaveformImage",
            version: "14.2.2",
        },
        SwiftPackageDependency {
            name: "modelcontextprotocol/swift-sdk",
            version: "0.11.0",
        },
        SwiftPackageDependency {
            name: "Sparkle",
            version: "2.7.0",
        },
        SwiftPackageDependency {
            name: "sentry-cocoa",
            version: "8.40.0",
        },
        SwiftPackageDependency {
            name: "clerk-convex-swift",
            version: "0.1.0",
        },
        SwiftPackageDependency {
            name: "clerk-ios",
            version: "1.0.0",
        },
        SwiftPackageDependency {
            name: "convex-swift",
            version: "0.8.0",
        },
        SwiftPackageDependency {
            name: "swift-transformers",
            version: "1.3.3",
        },
        SwiftPackageDependency {
            name: "lottie-ios",
            version: "4.6.1",
        },
    ]
}

/// PKG-002: Swift app target path.
pub const SWIFT_APP_TARGET_PATH: &str = "Sources/PalmierPro";

/// PKG-003: Files excluded from SwiftPM resource copying.
pub fn excluded_resource_files() -> Vec<&'static str> {
    vec![
        "Resources/Info.plist",
        "Resources/AppIcon.icon",
        "Resources/AppIcon.icns",
        "Resources/AppIcon.png",
    ]
}

/// PKG-004: Bundled copied resources.
pub fn bundled_resources() -> Vec<&'static str> {
    vec![
        "Resources/Fonts",
        "Resources/MCPB/palmier-pro.mcpb",
        "Resources/Images",
        "Resources/Changelog",
    ]
}

// ── BNDL-001..013: Bundle metadata ───────────────────────────────

/// BNDL-001..013: All bundle/version/UTI/URL-scheme/updater tokens.
pub struct BundleMetadata;
impl BundleMetadata {
    /// BNDL-001
    pub const DISPLAY_NAME: &'static str = "Palmier Pro";
    /// BNDL-002
    pub const EXECUTABLE_NAME: &'static str = "PalmierPro";
    /// BNDL-003
    pub const BUNDLE_IDENTIFIER: &'static str = "io.palmier.pro";
    /// BNDL-004
    pub const PACKAGE_TYPE: &'static str = "APPL";
    /// BNDL-005
    pub const VERSION_SHORT: &'static str = "0.3.5";
    /// BNDL-005
    pub const BUILD_NUMBER: &'static str = "53";
    /// BNDL-006
    pub const MIN_SYSTEM_VERSION: &'static str = "26.0";
    /// BNDL-008
    pub const URL_SCHEME: &'static str = "palmier";
    /// BNDL-008
    pub const URL_SCHEME_NAME: &'static str = "io.palmier.pro";
    /// BNDL-009
    pub const UTI_IDENTIFIER: &'static str = "io.palmier.project";
    /// BNDL-009
    pub const UTI_DESCRIPTION: &'static str = "Palmier Project";
    /// BNDL-009
    pub const FILE_EXTENSION: &'static str = "palmier";
    /// BNDL-009
    pub const UTI_CONFORMANCE: &'static str = "com.apple.package";
    /// BNDL-012
    pub const SPARKLE_FEED_URL: &'static str =
        "https://raw.githubusercontent.com/palmier-io/palmier-pro/main/appcast.xml";
}

// ── Tests ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // RUN tests

    #[test]
    fn run_baseline_constants() {
        assert_eq!(RuntimeBaseline::SWIFT_VERSION, "6.2");
        assert_eq!(RuntimeBaseline::PACKAGE_NAME, "PalmierPro");
        assert_eq!(RuntimeBaseline::EXECUTABLE, "PalmierPro");
        assert_eq!(RuntimeBaseline::REWRITE_NAME, "Fronda");
    }

    #[test]
    fn run_platform_constants() {
        assert_eq!(MINIMUM_MACOS_VERSION, "26.0");
        assert_eq!(TARGET_PLATFORM, "macOS 26.0+");
    }

    #[test]
    fn run_dev_prerequisites() {
        assert_eq!(DevPrerequisites::MACOS_VERSION, "26+");
        assert_eq!(DevPrerequisites::XCODE_VERSION, "16+");
        assert_eq!(DevPrerequisites::SWIFT_TOOLCHAIN, "6.2");
    }

    #[test]
    fn run_dev_commands() {
        assert_eq!(DevCommands::BUILD, "swift build");
        assert_eq!(DevCommands::RUN, "swift run");
        assert_eq!(DevCommands::DEV_SCRIPT, "./scripts/dev.sh");
    }

    // PKG tests

    #[test]
    fn pkg_dependencies_count() {
        let deps = swift_dependencies();
        assert_eq!(deps.len(), 9);
    }

    #[test]
    fn pkg_dependencies_sentinel_values() {
        let deps = swift_dependencies();
        assert!(deps
            .iter()
            .any(|d| d.name == "Sparkle" && d.version == "2.7.0"));
        assert!(deps
            .iter()
            .any(|d| d.name == "sentry-cocoa" && d.version == "8.40.0"));
        assert!(deps
            .iter()
            .any(|d| d.name == "lottie-ios" && d.version == "4.6.1"));
    }

    #[test]
    fn pkg_swift_target_path() {
        assert_eq!(SWIFT_APP_TARGET_PATH, "Sources/PalmierPro");
    }

    #[test]
    fn pkg_excluded_resources() {
        let excluded = excluded_resource_files();
        assert!(excluded.contains(&"Resources/Info.plist"));
        assert!(excluded.contains(&"Resources/AppIcon.png"));
        assert_eq!(excluded.len(), 4);
    }

    #[test]
    fn pkg_bundled_resources() {
        let bundled = bundled_resources();
        assert!(bundled.contains(&"Resources/Fonts"));
        assert!(bundled.contains(&"Resources/MCPB/palmier-pro.mcpb"));
        assert_eq!(bundled.len(), 4);
    }

    // BNDL tests

    #[test]
    fn bndl_display_name() {
        assert_eq!(BundleMetadata::DISPLAY_NAME, "Palmier Pro");
    }

    #[test]
    fn bndl_executable_name() {
        assert_eq!(BundleMetadata::EXECUTABLE_NAME, "PalmierPro");
    }

    #[test]
    fn bndl_bundle_identifier() {
        assert_eq!(BundleMetadata::BUNDLE_IDENTIFIER, "io.palmier.pro");
    }

    #[test]
    fn bndl_package_type() {
        assert_eq!(BundleMetadata::PACKAGE_TYPE, "APPL");
    }

    #[test]
    fn bndl_version_metadata() {
        assert_eq!(BundleMetadata::VERSION_SHORT, "0.3.5");
        assert_eq!(BundleMetadata::BUILD_NUMBER, "53");
    }

    #[test]
    fn bndl_min_system_version() {
        assert_eq!(BundleMetadata::MIN_SYSTEM_VERSION, "26.0");
    }

    #[test]
    fn bndl_url_scheme() {
        assert_eq!(BundleMetadata::URL_SCHEME, "palmier");
        assert_eq!(BundleMetadata::URL_SCHEME_NAME, "io.palmier.pro");
    }

    #[test]
    fn bndl_uti() {
        assert_eq!(BundleMetadata::UTI_IDENTIFIER, "io.palmier.project");
        assert_eq!(BundleMetadata::UTI_DESCRIPTION, "Palmier Project");
        assert_eq!(BundleMetadata::FILE_EXTENSION, "palmier");
        assert_eq!(BundleMetadata::UTI_CONFORMANCE, "com.apple.package");
    }

    #[test]
    fn bndl_sparkle_feed() {
        assert!(BundleMetadata::SPARKLE_FEED_URL.starts_with("https://"));
        assert!(BundleMetadata::SPARKLE_FEED_URL.contains("palmier-io"));
    }
}

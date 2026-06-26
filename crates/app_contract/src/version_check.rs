//! Version change detection for "What's New" display (APP-007).

/// APP-007: The result of comparing the last-launched version with the current version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionChange {
    /// First launch — no previous version recorded.
    FirstLaunch,
    /// Same version as last launch — no "What's New" needed.
    SameVersion,
    /// Version changed — "What's New" should be shown.
    VersionChanged,
}

/// APP-007: Check if the version has changed since the last launch.
///
/// * `last_launched_version` — The version string persisted from the previous launch,
///   or `None` if this is the first launch.
/// * `current_version` — The current app version string.
pub fn detect_version_change(
    last_launched_version: Option<&str>,
    current_version: &str,
) -> VersionChange {
    match last_launched_version {
        None => VersionChange::FirstLaunch,
        Some(prev) if prev == current_version => VersionChange::SameVersion,
        Some(_) => VersionChange::VersionChanged,
    }
}

/// APP-007: Determine whether the "What's New" surface should appear.
///
/// The surface appears only on a real version change (not on first install).
pub fn should_show_whats_new(change: VersionChange) -> bool {
    matches!(change, VersionChange::VersionChanged)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_007_first_launch_no_previous_version() {
        assert_eq!(
            detect_version_change(None, "0.3.5"),
            VersionChange::FirstLaunch
        );
    }

    #[test]
    fn app_007_same_version_no_change() {
        assert_eq!(
            detect_version_change(Some("0.3.5"), "0.3.5"),
            VersionChange::SameVersion
        );
    }

    #[test]
    fn app_007_version_upgrade_detected() {
        assert_eq!(
            detect_version_change(Some("0.3.4"), "0.3.5"),
            VersionChange::VersionChanged
        );
    }

    #[test]
    fn app_007_downgrade_also_detected() {
        assert_eq!(
            detect_version_change(Some("0.4.0"), "0.3.5"),
            VersionChange::VersionChanged
        );
    }

    #[test]
    fn app_007_should_show_only_on_real_change() {
        assert!(!should_show_whats_new(VersionChange::FirstLaunch));
        assert!(!should_show_whats_new(VersionChange::SameVersion));
        assert!(should_show_whats_new(VersionChange::VersionChanged));
    }

    #[test]
    fn app_007_major_version_change() {
        assert_eq!(
            detect_version_change(Some("0.3.5"), "1.0.0"),
            VersionChange::VersionChanged
        );
    }

    #[test]
    fn app_007_patch_version_change() {
        assert_eq!(
            detect_version_change(Some("0.3.5"), "0.3.6"),
            VersionChange::VersionChanged
        );
    }
}

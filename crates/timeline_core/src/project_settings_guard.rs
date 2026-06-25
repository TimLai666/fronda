use core_model::Timeline;

/// Result of checking project settings against incoming media
#[derive(Debug, Clone, PartialEq)]
pub enum SettingsGuardAction {
    /// PSET-003: No settings configured → auto-configure from first video asset
    AutoConfigure { fps: i64, width: i64, height: i64 },
    /// PSET-004: Timeline already has clips → proceed with existing settings (skip dialog)
    SkipDialog,
    /// PSET-005/006: Timeline empty but settings configured → need user decision
    ShowDialog {
        current_fps: i64,
        current_width: i64,
        current_height: i64,
        incoming_fps: i64,
        incoming_width: i64,
        incoming_height: i64,
    },
}

/// Configuration mismatch info
#[derive(Debug, Clone, PartialEq)]
pub struct SettingsMismatch {
    pub current_fps: i64,
    pub current_width: i64,
    pub current_height: i64,
    pub incoming_fps: i64,
    pub incoming_width: i64,
    pub incoming_height: i64,
}

/// Project settings guard
pub struct ProjectSettingsGuard;

impl ProjectSettingsGuard {
    /// PSET-001: Runs when adding media assets to the timeline
    /// PSET-002: Only inspects the FIRST video asset in the incoming set
    /// PSET-003: If no settings configured, first video silently auto-configures
    /// PSET-004: If timeline already has clips, skip dialog, proceed
    /// PSET-005: If timeline empty but settings configured, check mismatch
    pub fn check_first_video(
        timeline: &Timeline,
        incoming_fps: Option<i64>,
        incoming_width: Option<i64>,
        incoming_height: Option<i64>,
        has_existing_clips: bool,
    ) -> SettingsGuardAction {
        // No video in incoming set → SkipDialog (no configuration trigger)
        let (fps, width, height) = match (incoming_fps, incoming_width, incoming_height) {
            (Some(fps), Some(width), Some(height)) => (fps, width, height),
            _ => return SettingsGuardAction::SkipDialog,
        };

        // PSET-004: Timeline already has clips → skip dialog, proceed
        if has_existing_clips {
            return SettingsGuardAction::SkipDialog;
        }

        // PSET-003: No settings configured → auto-configure from first video
        if !is_settings_configured(timeline) {
            return SettingsGuardAction::AutoConfigure { fps, width, height };
        }

        // Settings configured, timeline empty → check for mismatch
        if timeline.fps != fps || timeline.width != width || timeline.height != height {
            SettingsGuardAction::ShowDialog {
                current_fps: timeline.fps,
                current_width: timeline.width,
                current_height: timeline.height,
                incoming_fps: fps,
                incoming_width: width,
                incoming_height: height,
            }
        } else {
            SettingsGuardAction::SkipDialog
        }
    }

    /// PSET-006: Keep Current keeps existing settings
    pub fn keep_current() -> SettingsGuardAction {
        SettingsGuardAction::SkipDialog
    }

    /// PSET-007: Change to Match applies clip fps/resolution
    pub fn change_to_match(
        timeline: &mut Timeline,
        incoming_fps: i64,
        incoming_width: i64,
        incoming_height: i64,
    ) {
        timeline.fps = incoming_fps;
        timeline.width = incoming_width;
        timeline.height = incoming_height;
        timeline.settings_configured = true;
    }
}

/// Helper: check if settings were configured
pub fn is_settings_configured(timeline: &Timeline) -> bool {
    timeline.settings_configured
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_timeline() -> Timeline {
        Timeline {
            settings_configured: false,
            ..Default::default()
        }
    }

    fn configured_timeline(fps: i64, width: i64, height: i64) -> Timeline {
        Timeline {
            fps,
            width,
            height,
            settings_configured: true,
            ..Default::default()
        }
    }

    // PSET-001: Guard runs for first video
    #[test]
    fn pset_001_guard_runs_for_first_video() {
        let timeline = empty_timeline();
        let action = ProjectSettingsGuard::check_first_video(
            &timeline,
            Some(60),
            Some(1920),
            Some(1080),
            false,
        );
        // We get AutoConfigure because no settings configured, proving the guard ran
        assert_eq!(
            action,
            SettingsGuardAction::AutoConfigure {
                fps: 60,
                width: 1920,
                height: 1080
            }
        );
    }

    // PSET-002: Only first video inspected; if first is image, second is video, no dialog
    #[test]
    fn pset_002_first_is_image_skips() {
        let timeline = empty_timeline();
        // First incoming item is not a video → None fps/width/height
        let action = ProjectSettingsGuard::check_first_video(&timeline, None, None, None, false);
        assert_eq!(action, SettingsGuardAction::SkipDialog);
    }

    // PSET-003: No settings + no existing clips → AutoConfigure
    #[test]
    fn pset_003_auto_configure() {
        let timeline = empty_timeline();
        let action = ProjectSettingsGuard::check_first_video(
            &timeline,
            Some(60),
            Some(3840),
            Some(2160),
            false,
        );
        assert_eq!(
            action,
            SettingsGuardAction::AutoConfigure {
                fps: 60,
                width: 3840,
                height: 2160
            }
        );
    }

    // PSET-004: Existing clips → SkipDialog (even if fps differs)
    #[test]
    fn pset_004_existing_clips_skips_dialog() {
        let timeline = configured_timeline(30, 1920, 1080);
        let action = ProjectSettingsGuard::check_first_video(
            &timeline,
            Some(60),
            Some(3840),
            Some(2160),
            true, // has existing clips
        );
        assert_eq!(action, SettingsGuardAction::SkipDialog);
    }

    // PSET-005: Timeline empty but settings configured + fps mismatch → ShowDialog
    #[test]
    fn pset_005_fps_mismatch_shows_dialog() {
        let timeline = configured_timeline(30, 1920, 1080);
        let action = ProjectSettingsGuard::check_first_video(
            &timeline,
            Some(60),
            Some(1920),
            Some(1080),
            false,
        );
        assert_eq!(
            action,
            SettingsGuardAction::ShowDialog {
                current_fps: 30,
                current_width: 1920,
                current_height: 1080,
                incoming_fps: 60,
                incoming_width: 1920,
                incoming_height: 1080,
            }
        );
    }

    // PSET-005: Same fps → SkipDialog (no mismatch)
    #[test]
    fn pset_005_same_fps_skips_dialog() {
        let timeline = configured_timeline(30, 1920, 1080);
        let action = ProjectSettingsGuard::check_first_video(
            &timeline,
            Some(30),
            Some(1920),
            Some(1080),
            false,
        );
        assert_eq!(action, SettingsGuardAction::SkipDialog);
    }

    // PSET-005: Width mismatch also triggers dialog
    #[test]
    fn pset_005_width_mismatch_shows_dialog() {
        let timeline = configured_timeline(30, 1920, 1080);
        let action = ProjectSettingsGuard::check_first_video(
            &timeline,
            Some(30),
            Some(3840),
            Some(1080),
            false,
        );
        assert_eq!(
            action,
            SettingsGuardAction::ShowDialog {
                current_fps: 30,
                current_width: 1920,
                current_height: 1080,
                incoming_fps: 30,
                incoming_width: 3840,
                incoming_height: 1080,
            }
        );
    }

    // PSET-005: Height mismatch also triggers dialog
    #[test]
    fn pset_005_height_mismatch_shows_dialog() {
        let timeline = configured_timeline(30, 1920, 1080);
        let action = ProjectSettingsGuard::check_first_video(
            &timeline,
            Some(30),
            Some(1920),
            Some(2160),
            false,
        );
        assert_eq!(
            action,
            SettingsGuardAction::ShowDialog {
                current_fps: 30,
                current_width: 1920,
                current_height: 1080,
                incoming_fps: 30,
                incoming_width: 1920,
                incoming_height: 2160,
            }
        );
    }

    // PSET-006: keep_current preserves settings
    #[test]
    fn pset_006_keep_current() {
        let result = ProjectSettingsGuard::keep_current();
        assert_eq!(result, SettingsGuardAction::SkipDialog);
    }

    // PSET-007: change_to_match applies new values + sets settingsConfigured=true
    #[test]
    fn pset_007_change_to_match() {
        let mut timeline = configured_timeline(30, 1920, 1080);
        ProjectSettingsGuard::change_to_match(&mut timeline, 60, 3840, 2160);
        assert_eq!(timeline.fps, 60);
        assert_eq!(timeline.width, 3840);
        assert_eq!(timeline.height, 2160);
        assert!(timeline.settings_configured);
    }

    // PSET-007: change_to_match on an unconfigured timeline also sets configured flag
    #[test]
    fn pset_007_change_to_match_on_unconfigured() {
        let mut timeline = empty_timeline();
        assert!(!timeline.settings_configured);
        ProjectSettingsGuard::change_to_match(&mut timeline, 24, 1280, 720);
        assert_eq!(timeline.fps, 24);
        assert_eq!(timeline.width, 1280);
        assert_eq!(timeline.height, 720);
        assert!(timeline.settings_configured);
    }

    // No video in incoming set → SkipDialog (no configuration trigger)
    #[test]
    fn no_video_in_incoming_skips_dialog() {
        let timeline = empty_timeline();
        let action = ProjectSettingsGuard::check_first_video(
            &timeline, None, // no fps means no video
            None, None, false,
        );
        assert_eq!(action, SettingsGuardAction::SkipDialog);
    }

    // Partially missing video info also skips
    #[test]
    fn partially_missing_video_info_skips() {
        let timeline = empty_timeline();
        // width missing even though fps is present — not a valid video
        let action =
            ProjectSettingsGuard::check_first_video(&timeline, Some(30), None, Some(1080), false);
        assert_eq!(action, SettingsGuardAction::SkipDialog);
    }

    // Settings configured but empty timeline with matching settings → SkipDialog
    #[test]
    fn configured_timeline_matching_settings_skips() {
        let timeline = configured_timeline(60, 3840, 2160);
        let action = ProjectSettingsGuard::check_first_video(
            &timeline,
            Some(60),
            Some(3840),
            Some(2160),
            false,
        );
        assert_eq!(action, SettingsGuardAction::SkipDialog);
    }

    // Empty timeline, not configured, incoming video → AutoConfigure even if defaults differ
    #[test]
    fn auto_configure_applies_incoming_not_defaults() {
        let timeline = Timeline::default(); // fps=30, w=1920, h=1080, settings_configured=false
        let action = ProjectSettingsGuard::check_first_video(
            &timeline,
            Some(24),
            Some(1280),
            Some(720),
            false,
        );
        assert_eq!(
            action,
            SettingsGuardAction::AutoConfigure {
                fps: 24,
                width: 1280,
                height: 720
            }
        );
    }
}

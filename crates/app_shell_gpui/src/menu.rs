/// Top-level menu groups (MENU-001).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuGroup {
    App,
    File,
    Edit,
    View,
    Help,
}

/// Menu item identifiers matching spec (MENU-002 to MENU-007).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuAction {
    // App menu (MENU-002)
    About,
    CheckForUpdates,
    Settings,
    Quit,
    // File menu (MENU-003)
    NewProject,
    OpenProject,
    SaveProject,
    SaveProjectAs,
    ImportMedia,
    Export,
    // Edit menu (MENU-004)
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    SelectAll,
    SplitAtPlayhead,
    TrimStartToPlayhead,
    TrimEndToPlayhead,
    Delete,
    /// Issue #164: ripple delete (⌥⌫, matching Premiere Pro / DaVinci Resolve).
    RippleDelete,
    // View menu (MENU-005)
    ToggleMediaPanel,
    ToggleInspector,
    ToggleAgentPanel,
    MaximizeFocusedPane,
    EnterFullScreen,
    // Layout submenu (MENU-006)
    LayoutDefault,
    LayoutMedia,
    LayoutVertical,
    // Help menu (MENU-007)
    Tutorial,
    KeyboardShortcuts,
    McpInstructions,
    SendFeedback,
    // Playback actions (KEY-001, Issue #164) ────────────────────────────────
    /// Space — play/pause (highest priority, Issue #164).
    PlayPause,
    /// J — play backward (JKL standard).
    PlayBackward,
    /// K — pause (JKL standard).
    PauseJkl,
    /// L — play forward (JKL standard).
    PlayForward,
    /// ← — step one frame backward (KEY-001).
    StepFrameBackward,
    /// → — step one frame forward (KEY-001).
    StepFrameForward,
    /// ⇧← — jump multiple frames backward (KEY-001).
    SkipFramesBackward,
    /// ⇧→ — jump multiple frames forward (KEY-001).
    SkipFramesForward,
    // Marking actions (Issue #164) ──────────────────────────────────────────
    /// I — mark range start (KEY-004).
    MarkIn,
    /// O — mark range end (KEY-004).
    MarkOut,
    /// ⌥I — clear mark in.
    ClearMarkIn,
    /// ⌥O — clear mark out.
    ClearMarkOut,
    /// ⌥X — clear both marks.
    ClearMarks,
    // Timeline zoom (Issue #164) ────────────────────────────────────────────
    /// = — zoom timeline in.
    TimelineZoomIn,
    /// - — zoom timeline out.
    TimelineZoomOut,
    /// ⇧Z — fit timeline to window.
    TimelineFitToWindow,
}

/// Modifier key flags.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub command: bool,
    pub shift: bool,
    pub option: bool,
    pub control: bool,
}

/// Keyboard shortcut binding.
#[derive(Debug, Clone)]
pub struct Shortcut {
    pub key: String,
    pub modifiers: Modifiers,
    pub action: MenuAction,
}

impl Shortcut {
    pub fn new(key: &str, modifiers: Modifiers, action: MenuAction) -> Self {
        Self {
            key: key.to_string(),
            modifiers,
            action,
        }
    }

    pub fn cmd(key: &str, action: MenuAction) -> Self {
        Self::new(
            key,
            Modifiers {
                command: true,
                ..Default::default()
            },
            action,
        )
    }

    pub fn cmd_shift(key: &str, action: MenuAction) -> Self {
        Self::new(
            key,
            Modifiers {
                command: true,
                shift: true,
                ..Default::default()
            },
            action,
        )
    }

    pub fn cmd_option(key: &str, action: MenuAction) -> Self {
        Self::new(
            key,
            Modifiers {
                command: true,
                option: true,
                ..Default::default()
            },
            action,
        )
    }
}

/// Returns all menu items organized by group matching MENU-001 to MENU-007.
pub fn all_menus() -> Vec<(MenuGroup, Vec<MenuAction>)> {
    vec![
        (
            MenuGroup::App,
            vec![
                MenuAction::About,
                MenuAction::CheckForUpdates,
                MenuAction::Settings,
                MenuAction::Quit,
            ],
        ),
        (
            MenuGroup::File,
            vec![
                MenuAction::NewProject,
                MenuAction::OpenProject,
                MenuAction::SaveProject,
                MenuAction::SaveProjectAs,
                MenuAction::ImportMedia,
                MenuAction::Export,
            ],
        ),
        (
            MenuGroup::Edit,
            vec![
                MenuAction::Undo,
                MenuAction::Redo,
                MenuAction::Cut,
                MenuAction::Copy,
                MenuAction::Paste,
                MenuAction::SelectAll,
                MenuAction::SplitAtPlayhead,
                MenuAction::TrimStartToPlayhead,
                MenuAction::TrimEndToPlayhead,
                MenuAction::Delete,
            ],
        ),
        (
            MenuGroup::View,
            vec![
                MenuAction::ToggleMediaPanel,
                MenuAction::ToggleInspector,
                MenuAction::ToggleAgentPanel,
                MenuAction::MaximizeFocusedPane,
                MenuAction::EnterFullScreen,
                MenuAction::LayoutDefault,
                MenuAction::LayoutMedia,
                MenuAction::LayoutVertical,
            ],
        ),
        (
            MenuGroup::Help,
            vec![
                MenuAction::Tutorial,
                MenuAction::KeyboardShortcuts,
                MenuAction::McpInstructions,
                MenuAction::SendFeedback,
            ],
        ),
    ]
}

/// Returns all keyboard shortcuts matching MENU-002 to MENU-007 key bindings.
pub fn all_shortcuts() -> Vec<Shortcut> {
    vec![
        // App menu (MENU-002)
        Shortcut::cmd(",", MenuAction::Settings),
        Shortcut::cmd("q", MenuAction::Quit),
        // File menu (MENU-003)
        Shortcut::cmd("n", MenuAction::NewProject),
        Shortcut::cmd("o", MenuAction::OpenProject),
        Shortcut::cmd("s", MenuAction::SaveProject),
        Shortcut::cmd_shift("s", MenuAction::SaveProjectAs),
        Shortcut::cmd("i", MenuAction::ImportMedia),
        Shortcut::cmd("e", MenuAction::Export),
        // Edit menu (MENU-004)
        Shortcut::cmd("z", MenuAction::Undo),
        Shortcut::cmd_shift("z", MenuAction::Redo),
        Shortcut::cmd("x", MenuAction::Cut),
        Shortcut::cmd("c", MenuAction::Copy),
        Shortcut::cmd("v", MenuAction::Paste),
        Shortcut::cmd("a", MenuAction::SelectAll),
        Shortcut::cmd("k", MenuAction::SplitAtPlayhead),
        Shortcut::new("q", Modifiers::default(), MenuAction::TrimStartToPlayhead),
        Shortcut::new("w", Modifiers::default(), MenuAction::TrimEndToPlayhead),
        Shortcut::new("backspace", Modifiers::default(), MenuAction::Delete),
        // View menu (MENU-005)
        Shortcut::cmd("0", MenuAction::ToggleMediaPanel),
        Shortcut::cmd_option("0", MenuAction::ToggleInspector),
        Shortcut::cmd_option("a", MenuAction::ToggleAgentPanel),
        Shortcut::new("`", Modifiers::default(), MenuAction::MaximizeFocusedPane),
        Shortcut::cmd("f", MenuAction::EnterFullScreen),
        // Layout submenu (MENU-006)
        Shortcut::cmd("1", MenuAction::LayoutDefault),
        Shortcut::cmd("2", MenuAction::LayoutMedia),
        Shortcut::cmd("3", MenuAction::LayoutVertical),
        // Help menu (MENU-007)
        Shortcut::cmd_shift("/", MenuAction::KeyboardShortcuts),
        // Playback (KEY-001, Issue #164)
        Shortcut::new("space", Modifiers::default(), MenuAction::PlayPause),
        Shortcut::new("j", Modifiers::default(), MenuAction::PlayBackward),
        Shortcut::new("k", Modifiers::default(), MenuAction::PauseJkl),
        Shortcut::new("l", Modifiers::default(), MenuAction::PlayForward),
        Shortcut::new("left", Modifiers::default(), MenuAction::StepFrameBackward),
        Shortcut::new("right", Modifiers::default(), MenuAction::StepFrameForward),
        Shortcut::new(
            "left",
            Modifiers {
                shift: true,
                ..Default::default()
            },
            MenuAction::SkipFramesBackward,
        ),
        Shortcut::new(
            "right",
            Modifiers {
                shift: true,
                ..Default::default()
            },
            MenuAction::SkipFramesForward,
        ),
        // Marking (KEY-004, Issue #164)
        Shortcut::new("i", Modifiers::default(), MenuAction::MarkIn),
        Shortcut::new("o", Modifiers::default(), MenuAction::MarkOut),
        Shortcut::new(
            "i",
            Modifiers {
                option: true,
                ..Default::default()
            },
            MenuAction::ClearMarkIn,
        ),
        Shortcut::new(
            "o",
            Modifiers {
                option: true,
                ..Default::default()
            },
            MenuAction::ClearMarkOut,
        ),
        Shortcut::new(
            "x",
            Modifiers {
                option: true,
                ..Default::default()
            },
            MenuAction::ClearMarks,
        ),
        // Ripple delete (Issue #164)
        Shortcut::new(
            "backspace",
            Modifiers {
                option: true,
                ..Default::default()
            },
            MenuAction::RippleDelete,
        ),
        // Timeline zoom (Issue #164)
        Shortcut::new("=", Modifiers::default(), MenuAction::TimelineZoomIn),
        Shortcut::new("-", Modifiers::default(), MenuAction::TimelineZoomOut),
        Shortcut::new(
            "z",
            Modifiers {
                shift: true,
                ..Default::default()
            },
            MenuAction::TimelineFitToWindow,
        ),
    ]
}

/// Route a keyboard event to the matching action.
pub fn route_shortcut(key: &str, modifiers: &Modifiers) -> Option<MenuAction> {
    all_shortcuts()
        .into_iter()
        .find(|s| s.key == key && s.modifiers == *modifiers)
        .map(|s| s.action)
}

/// True when a chord could also be typing (no command/option/control —
/// shift alone still types). These shortcuts are dispatched through gpui
/// key bindings with a `!input` context predicate, never via raw key_down
/// listeners, so text inputs win over them.
pub fn is_text_conflicting(modifiers: &Modifiers) -> bool {
    !modifiers.command && !modifiers.option && !modifiers.control
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find_group_items(group: MenuGroup, items: &[(MenuGroup, Vec<MenuAction>)]) -> &[MenuAction] {
        items
            .iter()
            .find(|(g, _)| *g == group)
            .map(|(_, actions)| actions.as_slice())
            .expect("group should exist")
    }

    #[test]
    fn menu_001_five_groups() {
        let menus = all_menus();
        assert_eq!(menus.len(), 5);
    }

    #[test]
    fn menu_002_app_menu_items() {
        let menus = all_menus();
        let items = find_group_items(MenuGroup::App, &menus);
        assert!(items.contains(&MenuAction::About));
        assert!(items.contains(&MenuAction::CheckForUpdates));
        assert!(items.contains(&MenuAction::Settings));
        assert!(items.contains(&MenuAction::Quit));
    }

    #[test]
    fn menu_003_file_menu_items() {
        let menus = all_menus();
        let items = find_group_items(MenuGroup::File, &menus);
        assert!(items.contains(&MenuAction::NewProject));
        assert!(items.contains(&MenuAction::OpenProject));
        assert!(items.contains(&MenuAction::SaveProject));
        assert!(items.contains(&MenuAction::SaveProjectAs));
        assert!(items.contains(&MenuAction::ImportMedia));
        assert!(items.contains(&MenuAction::Export));
    }

    #[test]
    fn menu_004_edit_menu_items() {
        let menus = all_menus();
        let items = find_group_items(MenuGroup::Edit, &menus);
        assert!(items.contains(&MenuAction::Undo));
        assert!(items.contains(&MenuAction::Redo));
        assert!(items.contains(&MenuAction::Cut));
        assert!(items.contains(&MenuAction::Copy));
        assert!(items.contains(&MenuAction::Paste));
        assert!(items.contains(&MenuAction::SelectAll));
        assert!(items.contains(&MenuAction::SplitAtPlayhead));
        assert!(items.contains(&MenuAction::TrimStartToPlayhead));
        assert!(items.contains(&MenuAction::TrimEndToPlayhead));
        assert!(items.contains(&MenuAction::Delete));
    }

    #[test]
    fn menu_005_view_menu_items() {
        let menus = all_menus();
        let items = find_group_items(MenuGroup::View, &menus);
        assert!(items.contains(&MenuAction::ToggleMediaPanel));
        assert!(items.contains(&MenuAction::ToggleInspector));
        assert!(items.contains(&MenuAction::ToggleAgentPanel));
        assert!(items.contains(&MenuAction::MaximizeFocusedPane));
        assert!(items.contains(&MenuAction::EnterFullScreen));
    }

    #[test]
    fn menu_006_layout_submenu() {
        let menus = all_menus();
        let items = find_group_items(MenuGroup::View, &menus);
        assert!(items.contains(&MenuAction::LayoutDefault));
        assert!(items.contains(&MenuAction::LayoutMedia));
        assert!(items.contains(&MenuAction::LayoutVertical));
    }

    #[test]
    fn menu_007_help_menu_items() {
        let menus = all_menus();
        let items = find_group_items(MenuGroup::Help, &menus);
        assert!(items.contains(&MenuAction::Tutorial));
        assert!(items.contains(&MenuAction::KeyboardShortcuts));
        assert!(items.contains(&MenuAction::McpInstructions));
        assert!(items.contains(&MenuAction::SendFeedback));
    }

    #[test]
    fn menu_008_shortcuts_count() {
        let shortcuts = all_shortcuts();
        // 27 original + 17 new playback/marking/ripple/zoom shortcuts (Issue #164)
        assert_eq!(shortcuts.len(), 44);
    }

    #[test]
    fn shortcut_route_cmd_z() {
        let modifiers = Modifiers {
            command: true,
            ..Default::default()
        };
        assert_eq!(route_shortcut("z", &modifiers), Some(MenuAction::Undo));
    }

    #[test]
    fn shortcut_route_cmd_shift_z() {
        let modifiers = Modifiers {
            command: true,
            shift: true,
            ..Default::default()
        };
        assert_eq!(route_shortcut("z", &modifiers), Some(MenuAction::Redo));
    }

    #[test]
    fn shortcut_route_unknown() {
        let modifiers = Modifiers {
            command: true,
            ..Default::default()
        };
        assert_eq!(route_shortcut("unknown", &modifiers), None);
    }

    // ---- Issue #164: playback / marking / zoom shortcuts --------------------

    #[test]
    fn issue_164_space_routes_play_pause() {
        assert_eq!(
            route_shortcut("space", &Modifiers::default()),
            Some(MenuAction::PlayPause)
        );
    }

    #[test]
    fn issue_164_jkl_routes_playback() {
        assert_eq!(
            route_shortcut("j", &Modifiers::default()),
            Some(MenuAction::PlayBackward)
        );
        assert_eq!(
            route_shortcut("k", &Modifiers::default()),
            Some(MenuAction::PauseJkl)
        );
        assert_eq!(
            route_shortcut("l", &Modifiers::default()),
            Some(MenuAction::PlayForward)
        );
    }

    #[test]
    fn issue_164_arrow_keys_route_frame_step() {
        assert_eq!(
            route_shortcut("left", &Modifiers::default()),
            Some(MenuAction::StepFrameBackward)
        );
        assert_eq!(
            route_shortcut("right", &Modifiers::default()),
            Some(MenuAction::StepFrameForward)
        );
    }

    #[test]
    fn issue_164_shift_arrow_routes_skip() {
        let shift = Modifiers {
            shift: true,
            ..Default::default()
        };
        assert_eq!(
            route_shortcut("left", &shift),
            Some(MenuAction::SkipFramesBackward)
        );
        assert_eq!(
            route_shortcut("right", &shift),
            Some(MenuAction::SkipFramesForward)
        );
    }

    #[test]
    fn issue_164_i_o_routes_mark_in_out() {
        assert_eq!(
            route_shortcut("i", &Modifiers::default()),
            Some(MenuAction::MarkIn)
        );
        assert_eq!(
            route_shortcut("o", &Modifiers::default()),
            Some(MenuAction::MarkOut)
        );
    }

    #[test]
    fn issue_164_option_backspace_routes_ripple_delete() {
        let opt = Modifiers {
            option: true,
            ..Default::default()
        };
        assert_eq!(
            route_shortcut("backspace", &opt),
            Some(MenuAction::RippleDelete)
        );
    }

    #[test]
    fn issue_164_timeline_zoom_shortcuts() {
        assert_eq!(
            route_shortcut("=", &Modifiers::default()),
            Some(MenuAction::TimelineZoomIn)
        );
        assert_eq!(
            route_shortcut("-", &Modifiers::default()),
            Some(MenuAction::TimelineZoomOut)
        );
        let shift = Modifiers {
            shift: true,
            ..Default::default()
        };
        assert_eq!(
            route_shortcut("z", &shift),
            Some(MenuAction::TimelineFitToWindow)
        );
    }

    #[test]
    fn issue_164_clear_marks_shortcuts() {
        let opt = Modifiers {
            option: true,
            ..Default::default()
        };
        assert_eq!(route_shortcut("i", &opt), Some(MenuAction::ClearMarkIn));
        assert_eq!(route_shortcut("o", &opt), Some(MenuAction::ClearMarkOut));
        assert_eq!(route_shortcut("x", &opt), Some(MenuAction::ClearMarks));
    }
}

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
    /// Import a timeline from an FCP7 XMEML / FCPXML file (#154).
    ImportTimeline,
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

    /// gpui keystroke string for non-macOS keybindings; the cmd modifier
    /// maps to the platform primary modifier Ctrl.
    pub fn keystroke(&self) -> String {
        let mut s = String::new();
        if self.modifiers.command || self.modifiers.control {
            s.push_str("ctrl-");
        }
        if self.modifiers.option {
            s.push_str("alt-");
        }
        if self.modifiers.shift {
            s.push_str("shift-");
        }
        s.push_str(&self.key);
        s
    }

    /// gpui keystroke string for macOS keybindings; the cmd modifier stays
    /// the platform (⌘) modifier. The keymap entry doubles as the native
    /// menu's key-equivalent source.
    pub fn keystroke_macos(&self) -> String {
        let mut s = String::new();
        if self.modifiers.command {
            s.push_str("cmd-");
        }
        if self.modifiers.control {
            s.push_str("ctrl-");
        }
        if self.modifiers.option {
            s.push_str("alt-");
        }
        if self.modifiers.shift {
            s.push_str("shift-");
        }
        s.push_str(&self.key);
        s
    }

    /// Human-readable hint for menu items ("Ctrl+Shift+S").
    pub fn display_hint(&self) -> String {
        let mut parts: Vec<&str> = Vec::new();
        if self.modifiers.command || self.modifiers.control {
            parts.push("Ctrl");
        }
        if self.modifiers.option {
            parts.push("Alt");
        }
        if self.modifiers.shift {
            parts.push("Shift");
        }
        let key = match self.key.as_str() {
            "backspace" => "⌫".to_string(),
            "space" => "Space".to_string(),
            "left" => "←".to_string(),
            "right" => "→".to_string(),
            k => k.to_uppercase(),
        };
        let mut out = parts.join("+");
        if !out.is_empty() {
            out.push('+');
        }
        out.push_str(&key);
        out
    }
}

const TEXT_INPUT_OWNED: [&str; 6] = ["a", "c", "v", "x", "z", "y"];

/// True for plain primary-modifier combos text inputs own
/// (Cmd/Ctrl+A/C/V/X/Z/Y — spec "Windows menu shortcuts" exclusion clause).
fn is_text_input_owned(s: &Shortcut) -> bool {
    let plain_primary = !s.modifiers.shift && !s.modifiers.option && !s.modifiers.control;
    plain_primary && TEXT_INPUT_OWNED.contains(&s.key.as_str())
}

/// Menu shortcuts to bind as global keybindings: the command-modifier
/// entries, minus the combos text inputs own.
pub fn menu_keybinding_shortcuts() -> Vec<Shortcut> {
    all_shortcuts()
        .into_iter()
        .filter(|s| s.modifiers.command && !is_text_input_owned(s))
        .collect()
}

/// The command shortcuts text inputs own (Cmd+A/C/V/X/Z in `all_shortcuts`).
/// Excluded from the plain menu bindings; macOS binds them separately with
/// an `!input` predicate so the Edit menu shows standard ⌘ key equivalents
/// while text fields keep priority.
pub fn text_input_owned_menu_shortcuts() -> Vec<Shortcut> {
    all_shortcuts()
        .into_iter()
        .filter(|s| s.modifiers.command && is_text_input_owned(s))
        .collect()
}

/// Shortcut display hint for a menu action, if it has one.
pub fn shortcut_hint_for(action: &MenuAction) -> Option<String> {
    all_shortcuts()
        .into_iter()
        .find(|s| &s.action == action)
        .map(|s| s.display_hint())
}

impl MenuGroup {
    /// Title-bar menu label (Swift MainMenu top-level titles).
    pub fn label(&self) -> &'static str {
        match self {
            MenuGroup::App => "Fronda",
            MenuGroup::File => "File",
            MenuGroup::Edit => "Edit",
            MenuGroup::View => "View",
            MenuGroup::Help => "Help",
        }
    }
}

impl MenuAction {
    /// Menu-item display label (Swift MainMenu item titles).
    pub fn label(&self) -> &'static str {
        match self {
            MenuAction::About => "About Fronda",
            MenuAction::CheckForUpdates => "Check for Updates…",
            MenuAction::Settings => "Settings…",
            MenuAction::Quit => "Quit Fronda",
            MenuAction::NewProject => "New Project",
            MenuAction::OpenProject => "Open Project…",
            MenuAction::SaveProject => "Save Project",
            MenuAction::SaveProjectAs => "Save Project As…",
            MenuAction::ImportMedia => "Import Media…",
            MenuAction::ImportTimeline => "Import Timeline…",
            MenuAction::Export => "Export…",
            MenuAction::Undo => "Undo",
            MenuAction::Redo => "Redo",
            MenuAction::Cut => "Cut",
            MenuAction::Copy => "Copy",
            MenuAction::Paste => "Paste",
            MenuAction::SelectAll => "Select All",
            MenuAction::SplitAtPlayhead => "Split at Playhead",
            MenuAction::TrimStartToPlayhead => "Trim Start to Playhead",
            MenuAction::TrimEndToPlayhead => "Trim End to Playhead",
            MenuAction::Delete => "Delete",
            MenuAction::RippleDelete => "Ripple Delete",
            MenuAction::ToggleMediaPanel => "Toggle Media Panel",
            MenuAction::ToggleInspector => "Toggle Inspector",
            MenuAction::ToggleAgentPanel => "Toggle Agent Panel",
            MenuAction::MaximizeFocusedPane => "Maximize Focused Panel",
            MenuAction::EnterFullScreen => "Enter Full Screen",
            MenuAction::LayoutDefault => "Layout: Default",
            MenuAction::LayoutMedia => "Layout: Media",
            MenuAction::LayoutVertical => "Layout: Vertical",
            MenuAction::Tutorial => "Tutorial",
            MenuAction::KeyboardShortcuts => "Keyboard Shortcuts",
            MenuAction::McpInstructions => "MCP Instructions",
            MenuAction::SendFeedback => "Send Feedback…",
            MenuAction::PlayPause => "Play/Pause",
            MenuAction::PlayBackward => "Play Backward",
            MenuAction::PauseJkl => "Pause",
            MenuAction::PlayForward => "Play Forward",
            MenuAction::StepFrameBackward => "Step Frame Backward",
            MenuAction::StepFrameForward => "Step Frame Forward",
            MenuAction::SkipFramesBackward => "Skip Frames Backward",
            MenuAction::SkipFramesForward => "Skip Frames Forward",
            MenuAction::MarkIn => "Mark In",
            MenuAction::MarkOut => "Mark Out",
            MenuAction::ClearMarkIn => "Clear Mark In",
            MenuAction::ClearMarkOut => "Clear Mark Out",
            MenuAction::ClearMarks => "Clear Marks",
            MenuAction::TimelineZoomIn => "Zoom In",
            MenuAction::TimelineZoomOut => "Zoom Out",
            MenuAction::TimelineFitToWindow => "Fit Timeline to Window",
        }
    }

    /// Short label for submenu contexts (Swift Layout submenu shows
    /// "Default" / "Media" / "Vertical", not the flattened title-bar form).
    pub fn short_label(&self) -> &'static str {
        match self {
            MenuAction::LayoutDefault => "Default",
            MenuAction::LayoutMedia => "Media",
            MenuAction::LayoutVertical => "Vertical",
            other => other.label(),
        }
    }
}

/// Menu items for one group, split into sections at Swift `MainMenu.swift`'s
/// separator positions. This is the primary menu definition; `all_menus`
/// flattens it so the two can never diverge.
pub fn menu_sections(group: MenuGroup) -> Vec<Vec<MenuAction>> {
    match group {
        MenuGroup::App => vec![
            vec![MenuAction::About],
            vec![MenuAction::CheckForUpdates],
            vec![MenuAction::Settings],
            vec![MenuAction::Quit],
        ],
        MenuGroup::File => vec![
            vec![MenuAction::NewProject, MenuAction::OpenProject],
            vec![MenuAction::SaveProject, MenuAction::SaveProjectAs],
            vec![MenuAction::ImportMedia, MenuAction::ImportTimeline],
            vec![MenuAction::Export],
        ],
        MenuGroup::Edit => vec![
            vec![MenuAction::Undo, MenuAction::Redo],
            vec![
                MenuAction::Cut,
                MenuAction::Copy,
                MenuAction::Paste,
                MenuAction::SelectAll,
            ],
            vec![
                MenuAction::SplitAtPlayhead,
                MenuAction::TrimStartToPlayhead,
                MenuAction::TrimEndToPlayhead,
            ],
            vec![MenuAction::Delete],
        ],
        MenuGroup::View => vec![
            vec![
                MenuAction::ToggleMediaPanel,
                MenuAction::ToggleInspector,
                MenuAction::ToggleAgentPanel,
            ],
            vec![MenuAction::MaximizeFocusedPane],
            vec![
                MenuAction::LayoutDefault,
                MenuAction::LayoutMedia,
                MenuAction::LayoutVertical,
            ],
            vec![MenuAction::EnterFullScreen],
        ],
        MenuGroup::Help => vec![
            vec![MenuAction::Tutorial],
            vec![MenuAction::KeyboardShortcuts, MenuAction::McpInstructions],
            vec![MenuAction::SendFeedback],
        ],
    }
}

/// Returns all menu items organized by group matching MENU-001 to MENU-007.
pub fn all_menus() -> Vec<(MenuGroup, Vec<MenuAction>)> {
    [
        MenuGroup::App,
        MenuGroup::File,
        MenuGroup::Edit,
        MenuGroup::View,
        MenuGroup::Help,
    ]
    .into_iter()
    .map(|group| (group, menu_sections(group).into_iter().flatten().collect()))
    .collect()
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
        Shortcut::cmd_shift("i", MenuAction::ImportTimeline),
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
        // Bracket aliases for trim (Swift ShortcutsPane "[ or Q" / "] or W", #164).
        Shortcut::new("[", Modifiers::default(), MenuAction::TrimStartToPlayhead),
        Shortcut::new("]", Modifiers::default(), MenuAction::TrimEndToPlayhead),
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
        // Ripple delete (Issue #164): ⌥⌫ (Premiere/Resolve) and ⇧⌫ (Swift canonical).
        Shortcut::new(
            "backspace",
            Modifiers {
                option: true,
                ..Default::default()
            },
            MenuAction::RippleDelete,
        ),
        Shortcut::new(
            "backspace",
            Modifiers {
                shift: true,
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

    #[test]
    fn menu_keybindings_exclude_text_input_combos() {
        let bindings = menu_keybinding_shortcuts();
        for owned in ["a", "c", "v", "x", "z", "y"] {
            assert!(
                !bindings.iter().any(|s| s.key == owned
                    && s.modifiers.command
                    && !s.modifiers.shift
                    && !s.modifiers.option),
                "plain Ctrl+{owned} must stay with text inputs"
            );
        }
        // Representative menu shortcuts survive, including shifted Z (redo).
        assert!(bindings
            .iter()
            .any(|s| s.action == MenuAction::NewProject && s.keystroke() == "ctrl-n"));
        assert!(bindings
            .iter()
            .any(|s| s.action == MenuAction::SaveProjectAs && s.keystroke() == "ctrl-shift-s"));
        assert!(bindings
            .iter()
            .any(|s| s.action == MenuAction::Redo && s.keystroke() == "ctrl-shift-z"));
        // Modifier-free shortcuts are global_shortcuts' job, not menu bindings.
        assert!(bindings.iter().all(|s| s.modifiers.command));
    }

    #[test]
    fn text_input_owned_shortcuts_are_exactly_the_excluded_five() {
        let owned = text_input_owned_menu_shortcuts();
        let mut pairs: Vec<(String, MenuAction)> =
            owned.into_iter().map(|s| (s.key, s.action)).collect();
        pairs.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(
            pairs,
            vec![
                ("a".to_string(), MenuAction::SelectAll),
                ("c".to_string(), MenuAction::Copy),
                ("v".to_string(), MenuAction::Paste),
                ("x".to_string(), MenuAction::Cut),
                ("z".to_string(), MenuAction::Undo),
            ]
        );
    }

    #[test]
    fn menu_sections_flatten_to_all_menus() {
        for (group, actions) in all_menus() {
            let flattened: Vec<MenuAction> =
                menu_sections(group).into_iter().flatten().collect();
            assert_eq!(flattened, actions, "sections diverge for {group:?}");
        }
    }

    #[test]
    fn menu_sections_match_swift_separator_grouping() {
        // Swift MainMenu.swift separator placement, group by group.
        let app = menu_sections(MenuGroup::App);
        assert_eq!(
            app,
            vec![
                vec![MenuAction::About],
                vec![MenuAction::CheckForUpdates],
                vec![MenuAction::Settings],
                vec![MenuAction::Quit],
            ]
        );
        let file = menu_sections(MenuGroup::File);
        assert_eq!(
            file,
            vec![
                vec![MenuAction::NewProject, MenuAction::OpenProject],
                vec![MenuAction::SaveProject, MenuAction::SaveProjectAs],
                vec![MenuAction::ImportMedia, MenuAction::ImportTimeline],
                vec![MenuAction::Export],
            ]
        );
        let edit = menu_sections(MenuGroup::Edit);
        assert_eq!(
            edit,
            vec![
                vec![MenuAction::Undo, MenuAction::Redo],
                vec![
                    MenuAction::Cut,
                    MenuAction::Copy,
                    MenuAction::Paste,
                    MenuAction::SelectAll,
                ],
                vec![
                    MenuAction::SplitAtPlayhead,
                    MenuAction::TrimStartToPlayhead,
                    MenuAction::TrimEndToPlayhead,
                ],
                vec![MenuAction::Delete],
            ]
        );
        let view = menu_sections(MenuGroup::View);
        assert_eq!(
            view,
            vec![
                vec![
                    MenuAction::ToggleMediaPanel,
                    MenuAction::ToggleInspector,
                    MenuAction::ToggleAgentPanel,
                ],
                vec![MenuAction::MaximizeFocusedPane],
                vec![
                    MenuAction::LayoutDefault,
                    MenuAction::LayoutMedia,
                    MenuAction::LayoutVertical,
                ],
                vec![MenuAction::EnterFullScreen],
            ]
        );
        let help = menu_sections(MenuGroup::Help);
        assert_eq!(
            help,
            vec![
                vec![MenuAction::Tutorial],
                vec![
                    MenuAction::KeyboardShortcuts,
                    MenuAction::McpInstructions,
                ],
                vec![MenuAction::SendFeedback],
            ]
        );
    }

    #[test]
    fn layout_actions_have_short_labels() {
        // Swift Layout submenu item titles (LayoutPreset labels).
        assert_eq!(MenuAction::LayoutDefault.short_label(), "Default");
        assert_eq!(MenuAction::LayoutMedia.short_label(), "Media");
        assert_eq!(MenuAction::LayoutVertical.short_label(), "Vertical");
        // Everything else falls back to the full label.
        assert_eq!(
            MenuAction::NewProject.short_label(),
            MenuAction::NewProject.label()
        );
    }

    #[test]
    fn keystroke_macos_maps_cmd_to_cmd() {
        assert_eq!(
            Shortcut::cmd("n", MenuAction::NewProject).keystroke_macos(),
            "cmd-n"
        );
        assert_eq!(
            Shortcut::cmd_shift("s", MenuAction::SaveProjectAs).keystroke_macos(),
            "cmd-shift-s"
        );
        assert_eq!(
            Shortcut::cmd_option("0", MenuAction::ToggleInspector).keystroke_macos(),
            "cmd-alt-0"
        );
    }

    #[test]
    fn keystroke_strings_map_cmd_to_ctrl() {
        assert_eq!(
            Shortcut::cmd("n", MenuAction::NewProject).keystroke(),
            "ctrl-n"
        );
        assert_eq!(
            Shortcut::cmd_shift("s", MenuAction::SaveProjectAs).keystroke(),
            "ctrl-shift-s"
        );
        assert_eq!(
            Shortcut::cmd_option("0", MenuAction::ToggleInspector).keystroke(),
            "ctrl-alt-0"
        );
        assert_eq!(
            Shortcut::cmd("n", MenuAction::NewProject).display_hint(),
            "Ctrl+N"
        );
    }

    #[test]
    fn all_menu_actions_have_labels_and_group_labels() {
        for (group, actions) in all_menus() {
            assert!(!group.label().is_empty());
            for action in actions {
                assert!(!action.label().is_empty(), "missing label for {action:?}");
            }
        }
    }

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
        assert!(items.contains(&MenuAction::ImportTimeline));
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
        // 44 prior + 3 Swift-parity aliases ([ / ] trim, ⇧⌫ ripple) (Issue #164)
        // + 1 Import Timeline (⌘⇧I, #154)
        assert_eq!(shortcuts.len(), 48);
    }

    #[test]
    fn issue_164_bracket_trim_aliases() {
        assert_eq!(
            route_shortcut("[", &Modifiers::default()),
            Some(MenuAction::TrimStartToPlayhead)
        );
        assert_eq!(
            route_shortcut("]", &Modifiers::default()),
            Some(MenuAction::TrimEndToPlayhead)
        );
    }

    #[test]
    fn issue_164_shift_backspace_ripple_delete() {
        let shift = Modifiers {
            shift: true,
            ..Default::default()
        };
        assert_eq!(
            route_shortcut("backspace", &shift),
            Some(MenuAction::RippleDelete)
        );
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

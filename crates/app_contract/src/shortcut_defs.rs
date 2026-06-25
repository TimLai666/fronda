//! Main menu structure and keyboard shortcut definitions.
//!
//! Covers MENU-001 through MENU-008 and KEY-001 through KEY-006.

use serde::{Deserialize, Serialize};

/// A keyboard modifier flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ModifierFlag {
    Command,
    Option,
    Shift,
    Control,
}

// ═══════════════════════════════════════════════════════════════════
// MENU-001..008: Main menu structure
// ═══════════════════════════════════════════════════════════════════

/// MENU-001: Top-level menu group identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MenuGroup {
    App,
    File,
    Edit,
    View,
    Help,
}

impl MenuGroup {
    pub const ALL: &'static [MenuGroup] = &[
        MenuGroup::App,
        MenuGroup::File,
        MenuGroup::Edit,
        MenuGroup::View,
        MenuGroup::Help,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            MenuGroup::App => "Palmier Pro",
            MenuGroup::File => "File",
            MenuGroup::Edit => "Edit",
            MenuGroup::View => "View",
            MenuGroup::Help => "Help",
        }
    }
}

/// A menu item descriptor.
#[derive(Debug, Clone, PartialEq)]
pub struct MenuItem {
    pub label: &'static str,
    pub action: &'static str,
    pub shortcut: Option<&'static str>,
    pub modifier_flags: Option<&'static [ModifierFlag]>,
}

/// MENU-002: App menu items.
pub fn app_menu_items() -> Vec<MenuItem> {
    vec![
        MenuItem {
            label: "About Palmier Pro",
            action: "orderFrontStandardAboutPanel:",
            shortcut: None,
            modifier_flags: None,
        },
        MenuItem {
            label: "Check for Updates…",
            action: "checkForUpdates:",
            shortcut: None,
            modifier_flags: None,
        },
        MenuItem {
            label: "Settings…",
            action: "showSettings:",
            shortcut: Some(","),
            modifier_flags: Some(&[ModifierFlag::Command]),
        },
        MenuItem {
            label: "Quit Palmier Pro",
            action: "terminate:",
            shortcut: Some("q"),
            modifier_flags: Some(&[ModifierFlag::Command]),
        },
    ]
}

/// MENU-003: File menu items.
pub fn file_menu_items() -> Vec<MenuItem> {
    vec![
        MenuItem {
            label: "New",
            action: "newDocument:",
            shortcut: Some("n"),
            modifier_flags: Some(&[ModifierFlag::Command]),
        },
        MenuItem {
            label: "Open…",
            action: "openDocument:",
            shortcut: Some("o"),
            modifier_flags: Some(&[ModifierFlag::Command]),
        },
        MenuItem {
            label: "Save",
            action: "save:",
            shortcut: Some("s"),
            modifier_flags: Some(&[ModifierFlag::Command]),
        },
        MenuItem {
            label: "Save As…",
            action: "saveAs:",
            shortcut: Some("S"),
            modifier_flags: Some(&[ModifierFlag::Command, ModifierFlag::Shift]),
        },
        MenuItem {
            label: "Import Media…",
            action: "importMedia:",
            shortcut: Some("i"),
            modifier_flags: Some(&[ModifierFlag::Command]),
        },
        MenuItem {
            label: "Export…",
            action: "showExport:",
            shortcut: Some("e"),
            modifier_flags: Some(&[ModifierFlag::Command]),
        },
    ]
}

/// MENU-004: Edit menu items.
pub fn edit_menu_items() -> Vec<MenuItem> {
    vec![
        MenuItem {
            label: "Undo",
            action: "undo:",
            shortcut: Some("z"),
            modifier_flags: Some(&[ModifierFlag::Command]),
        },
        MenuItem {
            label: "Redo",
            action: "redo:",
            shortcut: Some("Z"),
            modifier_flags: Some(&[ModifierFlag::Command, ModifierFlag::Shift]),
        },
        MenuItem {
            label: "Cut",
            action: "cut:",
            shortcut: Some("x"),
            modifier_flags: Some(&[ModifierFlag::Command]),
        },
        MenuItem {
            label: "Copy",
            action: "copy:",
            shortcut: Some("c"),
            modifier_flags: Some(&[ModifierFlag::Command]),
        },
        MenuItem {
            label: "Paste",
            action: "paste:",
            shortcut: Some("v"),
            modifier_flags: Some(&[ModifierFlag::Command]),
        },
        MenuItem {
            label: "Select All",
            action: "selectAll:",
            shortcut: Some("a"),
            modifier_flags: Some(&[ModifierFlag::Command]),
        },
        MenuItem {
            label: "Split at Playhead",
            action: "splitAtPlayhead:",
            shortcut: Some("k"),
            modifier_flags: Some(&[ModifierFlag::Command]),
        },
        MenuItem {
            label: "Trim Start to Playhead",
            action: "trimStartToPlayhead:",
            shortcut: Some("q"),
            modifier_flags: None,
        },
        MenuItem {
            label: "Trim End to Playhead",
            action: "trimEndToPlayhead:",
            shortcut: Some("w"),
            modifier_flags: None,
        },
        MenuItem {
            label: "Delete",
            action: "deleteSelectedClips:",
            shortcut: Some("\u{8}"),
            modifier_flags: None,
        }, // backspace
    ]
}

/// MENU-005: View menu items.
pub fn view_menu_items() -> Vec<MenuItem> {
    vec![
        MenuItem {
            label: "Media Panel",
            action: "toggleMediaPanel:",
            shortcut: Some("0"),
            modifier_flags: Some(&[ModifierFlag::Command]),
        },
        MenuItem {
            label: "Inspector",
            action: "toggleInspectorPanel:",
            shortcut: Some("0"),
            modifier_flags: Some(&[ModifierFlag::Command, ModifierFlag::Option]),
        },
        MenuItem {
            label: "Agent Panel",
            action: "toggleAgentPanel:",
            shortcut: Some("a"),
            modifier_flags: Some(&[ModifierFlag::Command, ModifierFlag::Option]),
        },
        MenuItem {
            label: "Maximize Focused Panel",
            action: "toggleMaximizePanel:",
            shortcut: Some("`"),
            modifier_flags: None,
        },
    ]
}

/// MENU-006: Layout submenu items.
pub fn layout_submenu_items() -> Vec<MenuItem> {
    vec![
        MenuItem {
            label: "Default",
            action: "setLayoutDefault:",
            shortcut: Some("1"),
            modifier_flags: Some(&[ModifierFlag::Command]),
        },
        MenuItem {
            label: "Media",
            action: "setLayoutMedia:",
            shortcut: Some("2"),
            modifier_flags: Some(&[ModifierFlag::Command]),
        },
        MenuItem {
            label: "Vertical",
            action: "setLayoutVertical:",
            shortcut: Some("3"),
            modifier_flags: Some(&[ModifierFlag::Command]),
        },
    ]
}

/// MENU-007: Help menu items.
pub fn help_menu_items() -> Vec<MenuItem> {
    vec![
        MenuItem {
            label: "Tutorial",
            action: "showTutorial:",
            shortcut: None,
            modifier_flags: None,
        },
        MenuItem {
            label: "Keyboard Shortcuts",
            action: "showKeyboardShortcuts:",
            shortcut: Some("?"),
            modifier_flags: Some(&[ModifierFlag::Command]),
        },
        MenuItem {
            label: "MCP Instructions",
            action: "showMCPInstructions:",
            shortcut: None,
            modifier_flags: None,
        },
        MenuItem {
            label: "Send Feedback…",
            action: "showFeedback:",
            shortcut: None,
            modifier_flags: None,
        },
    ]
}

// ═══════════════════════════════════════════════════════════════════
// KEY-001..006: Shortcut help content
// ═══════════════════════════════════════════════════════════════════

/// A keyboard shortcut help entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShortcutEntry {
    pub group: &'static str,
    pub keys: &'static str,
    pub description: &'static str,
}

/// KEY-001: Playback shortcuts.
pub fn playback_shortcuts() -> Vec<ShortcutEntry> {
    vec![
        ShortcutEntry {
            group: "Playback",
            keys: "Space",
            description: "Play / Pause",
        },
        ShortcutEntry {
            group: "Playback",
            keys: "Left",
            description: "Step backward",
        },
        ShortcutEntry {
            group: "Playback",
            keys: "Right",
            description: "Step forward",
        },
        ShortcutEntry {
            group: "Playback",
            keys: "Shift+Left",
            description: "Skip backward",
        },
        ShortcutEntry {
            group: "Playback",
            keys: "Shift+Right",
            description: "Skip forward",
        },
    ]
}

/// KEY-002: Tools shortcuts.
pub fn tools_shortcuts() -> Vec<ShortcutEntry> {
    vec![
        ShortcutEntry {
            group: "Tools",
            keys: "V",
            description: "Selection tool",
        },
        ShortcutEntry {
            group: "Tools",
            keys: "C",
            description: "Razor tool",
        },
    ]
}

/// KEY-003: Editing shortcuts.
pub fn editing_shortcuts() -> Vec<ShortcutEntry> {
    vec![
        ShortcutEntry {
            group: "Editing",
            keys: "Cmd+K",
            description: "Split",
        },
        ShortcutEntry {
            group: "Editing",
            keys: "[ or Q",
            description: "Trim start",
        },
        ShortcutEntry {
            group: "Editing",
            keys: "] or W",
            description: "Trim end",
        },
        ShortcutEntry {
            group: "Editing",
            keys: "Backspace",
            description: "Delete",
        },
        ShortcutEntry {
            group: "Editing",
            keys: "Shift+Backspace",
            description: "Ripple delete",
        },
        ShortcutEntry {
            group: "Editing",
            keys: "Option+Drag",
            description: "Duplicate clip",
        },
    ]
}

/// KEY-004: Timeline shortcuts.
pub fn timeline_shortcuts() -> Vec<ShortcutEntry> {
    vec![
        ShortcutEntry {
            group: "Timeline",
            keys: "Shift+Drag Ruler",
            description: "Select range",
        },
        ShortcutEntry {
            group: "Timeline",
            keys: "Drag Range Edge",
            description: "Adjust range",
        },
        ShortcutEntry {
            group: "Timeline",
            keys: "I",
            description: "Mark range start",
        },
        ShortcutEntry {
            group: "Timeline",
            keys: "O",
            description: "Mark range end",
        },
        ShortcutEntry {
            group: "Timeline",
            keys: "Option+Scroll",
            description: "Zoom to cursor",
        },
        ShortcutEntry {
            group: "Timeline",
            keys: "Pinch zoom",
            description: "Zoom to cursor",
        },
        ShortcutEntry {
            group: "Timeline",
            keys: "Cmd+Scroll",
            description: "Scroll horizontally",
        },
    ]
}

/// KEY-005: General shortcuts.
pub fn general_shortcuts() -> Vec<ShortcutEntry> {
    vec![
        ShortcutEntry {
            group: "General",
            keys: "Cmd+Scroll",
            description: "Preview zoom",
        },
        ShortcutEntry {
            group: "General",
            keys: "Escape",
            description: "Deselect / reset tool",
        },
    ]
}

/// KEY-006: The key-column width for shortcut layout.
pub const SHORTCUT_KEY_COLUMN_WIDTH: f64 = 118.0;

/// All shortcut groups in display order (left column first, right column second).
pub fn all_shortcut_groups() -> Vec<Vec<ShortcutEntry>> {
    vec![
        playback_shortcuts(),
        tools_shortcuts(),
        editing_shortcuts(),
        timeline_shortcuts(),
        general_shortcuts(),
    ]
}

/// Shortcut groups displayed on the left column (KEY-006).
pub fn left_column_shortcut_groups() -> Vec<Vec<ShortcutEntry>> {
    vec![
        playback_shortcuts(),
        tools_shortcuts(),
        editing_shortcuts(),
        timeline_shortcuts(),
    ]
}

/// Shortcut groups displayed on the right column (KEY-006).
pub fn right_column_shortcut_groups() -> Vec<Vec<ShortcutEntry>> {
    vec![general_shortcuts()]
}

// ═══════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // MENU-001
    #[test]
    fn menu_groups_count() {
        assert_eq!(MenuGroup::ALL.len(), 5);
    }

    #[test]
    fn menu_group_labels() {
        assert_eq!(MenuGroup::App.label(), "Palmier Pro");
        assert_eq!(MenuGroup::File.label(), "File");
        assert_eq!(MenuGroup::Edit.label(), "Edit");
        assert_eq!(MenuGroup::View.label(), "View");
        assert_eq!(MenuGroup::Help.label(), "Help");
    }

    // MENU-002
    #[test]
    fn app_menu_has_settings_shortcut() {
        let items = app_menu_items();
        let settings = items.iter().find(|i| i.label.contains("Settings")).unwrap();
        assert_eq!(settings.shortcut, Some(","));
        assert!(settings
            .modifier_flags
            .unwrap()
            .contains(&ModifierFlag::Command));
    }

    #[test]
    fn app_menu_has_quit() {
        let items = app_menu_items();
        assert!(items.iter().any(|i| i.label.contains("Quit")));
    }

    // MENU-003
    #[test]
    fn file_menu_has_save_shortcuts() {
        let items = file_menu_items();
        assert!(items
            .iter()
            .any(|i| i.label == "New" && i.shortcut == Some("n")));
        assert!(items
            .iter()
            .any(|i| i.label == "Open…" && i.shortcut == Some("o")));
        assert!(items
            .iter()
            .any(|i| i.label == "Save" && i.shortcut == Some("s")));
        assert!(items
            .iter()
            .any(|i| i.label == "Export…" && i.shortcut == Some("e")));
    }

    // MENU-004
    #[test]
    fn edit_menu_has_undo_redo() {
        let items = edit_menu_items();
        assert!(items
            .iter()
            .any(|i| i.label == "Undo" && i.shortcut == Some("z")));
        assert!(items
            .iter()
            .any(|i| i.label == "Redo" && i.shortcut == Some("Z")));
    }

    #[test]
    fn edit_menu_has_split_and_trim() {
        let items = edit_menu_items();
        assert!(items.iter().any(|i| i.label == "Split at Playhead"));
        assert!(items.iter().any(|i| i.label == "Trim Start to Playhead"));
        assert!(items.iter().any(|i| i.label == "Trim End to Playhead"));
    }

    // MENU-005
    #[test]
    fn view_menu_items_count() {
        let items = view_menu_items();
        assert_eq!(items.len(), 4);
    }

    // MENU-006
    #[test]
    fn test_layout_submenu_items() {
        let items = layout_submenu_items();
        assert_eq!(items.len(), 3);
        assert!(items.iter().any(|i| i.label == "Default"));
        assert!(items.iter().any(|i| i.label == "Media"));
        assert!(items.iter().any(|i| i.label == "Vertical"));
    }

    // MENU-007
    #[test]
    fn help_menu_items_count() {
        let items = help_menu_items();
        assert_eq!(items.len(), 4);
        assert!(items.iter().any(|i| i.label == "Keyboard Shortcuts"));
        assert!(items.iter().any(|i| i.label == "Send Feedback…"));
    }

    // KEY-001
    #[test]
    fn test_playback_shortcuts_count() {
        let shortcuts = playback_shortcuts();
        assert_eq!(shortcuts.len(), 5);
        assert!(shortcuts.iter().any(|s| s.keys == "Space"));
    }

    // KEY-002
    #[test]
    fn test_tools_shortcuts() {
        let shortcuts = tools_shortcuts();
        assert_eq!(shortcuts.len(), 2);
        assert!(shortcuts.iter().any(|s| s.keys == "V"));
        assert!(shortcuts.iter().any(|s| s.keys == "C"));
    }

    // KEY-003
    #[test]
    fn test_editing_shortcuts_count() {
        let shortcuts = editing_shortcuts();
        assert_eq!(shortcuts.len(), 6);
        assert!(shortcuts.iter().any(|s| s.keys == "Cmd+K"));
        assert!(shortcuts.iter().any(|s| s.keys == "Backspace"));
        assert!(shortcuts.iter().any(|s| s.keys == "Option+Drag"));
    }

    // KEY-004
    #[test]
    fn test_timeline_shortcuts_count() {
        let shortcuts = timeline_shortcuts();
        assert_eq!(shortcuts.len(), 7);
        assert!(shortcuts.iter().any(|s| s.keys == "I"));
        assert!(shortcuts.iter().any(|s| s.keys == "O"));
    }

    // KEY-005
    #[test]
    fn test_general_shortcuts_count() {
        let shortcuts = general_shortcuts();
        assert_eq!(shortcuts.len(), 2);
    }

    // KEY-006
    #[test]
    fn shortcut_key_column_width() {
        assert!((SHORTCUT_KEY_COLUMN_WIDTH - 118.0).abs() < 1e-10);
    }

    #[test]
    fn left_column_contains_four_groups() {
        let groups = left_column_shortcut_groups();
        assert_eq!(groups.len(), 4);
    }
}

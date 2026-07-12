//! Modifier-free global shortcuts as gpui actions with a `!input` context
//! predicate: when focus sits anywhere inside a text input (any element
//! whose key context contains `input`), the binding is inert and the key
//! reaches the field as text instead. This replaces raw key_down routing
//! for these keys — a listener can't tell typing from a shortcut, the
//! binding system can.

use gpui::{actions, App, KeyBinding};

/// Menu-shortcut dispatch: one action carrying the target MenuAction, bound
/// per shortcut on non-macOS (macOS routes through the system menu).
#[derive(Clone, PartialEq, gpui::Action)]
#[action(namespace = fronda_menu, no_json)]
pub struct RunMenuAction {
    pub action: crate::menu::MenuAction,
}

/// Register menu-shortcut keybindings (Ctrl-modifier chords). No-op on
/// macOS. Call once at boot, before the window opens.
pub fn bind_menu_shortcut_keys(cx: &mut App) {
    if cfg!(target_os = "macos") {
        return;
    }
    let bindings: Vec<KeyBinding> = crate::menu::menu_keybinding_shortcuts()
        .into_iter()
        .map(|s| {
            KeyBinding::new(
                &s.keystroke(),
                RunMenuAction { action: s.action },
                None,
            )
        })
        .collect();
    cx.bind_keys(bindings);
}

actions!(
    fronda_shortcuts,
    [
        PlayPause,
        PlayBackward,
        PauseJkl,
        PlayForward,
        StepFrameBackward,
        StepFrameForward,
        SkipFramesBackward,
        SkipFramesForward,
        TrimStartToPlayhead,
        TrimEndToPlayhead,
        DeleteSelection,
        RippleDeleteSelection,
        MaximizeFocusedPane,
        MarkIn,
        MarkOut,
        TimelineZoomIn,
        TimelineZoomOut,
        TimelineFitToWindow,
    ]
);

/// Register the bindings. Call once at boot, before the window opens.
pub fn bind_global_shortcut_keys(cx: &mut App) {
    const CTX: Option<&str> = Some("!input");
    cx.bind_keys([
        KeyBinding::new("space", PlayPause, CTX),
        KeyBinding::new("j", PlayBackward, CTX),
        KeyBinding::new("k", PauseJkl, CTX),
        KeyBinding::new("l", PlayForward, CTX),
        KeyBinding::new("left", StepFrameBackward, CTX),
        KeyBinding::new("right", StepFrameForward, CTX),
        KeyBinding::new("shift-left", SkipFramesBackward, CTX),
        KeyBinding::new("shift-right", SkipFramesForward, CTX),
        KeyBinding::new("q", TrimStartToPlayhead, CTX),
        KeyBinding::new("w", TrimEndToPlayhead, CTX),
        // Bracket aliases for trim (Swift "[ or Q" / "] or W", Issue #164).
        KeyBinding::new("[", TrimStartToPlayhead, CTX),
        KeyBinding::new("]", TrimEndToPlayhead, CTX),
        KeyBinding::new("backspace", DeleteSelection, CTX),
        // ⇧⌫ ripple delete (Swift canonical; ⌥⌫ stays via the chord path).
        KeyBinding::new("shift-backspace", RippleDeleteSelection, CTX),
        KeyBinding::new("`", MaximizeFocusedPane, CTX),
        KeyBinding::new("i", MarkIn, CTX),
        KeyBinding::new("o", MarkOut, CTX),
        KeyBinding::new("=", TimelineZoomIn, CTX),
        KeyBinding::new("-", TimelineZoomOut, CTX),
        KeyBinding::new("shift-z", TimelineFitToWindow, CTX),
    ]);
}

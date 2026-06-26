//! Home screen — colors and focus handle container.
//!
//! The full DOM tree with click handlers is rendered by `AppRoot::render_home`
//! so that handlers can call AppRoot methods directly without an event bridge.

use gpui::{App, FocusHandle, Focusable, Hsla};

/// Colors for the home view.
pub struct HomeColors;
impl HomeColors {
    pub const BACKGROUND: Hsla = Hsla {
        h: 0.0,
        s: 0.0,
        l: 0.07,
        a: 1.0,
    };
    pub const CARD_BG: Hsla = Hsla {
        h: 0.0,
        s: 0.0,
        l: 0.12,
        a: 1.0,
    };
    pub const TEXT_PRIMARY: Hsla = Hsla {
        h: 0.0,
        s: 0.0,
        l: 1.0,
        a: 1.0,
    };
    pub const TEXT_SECONDARY: Hsla = Hsla {
        h: 0.0,
        s: 0.0,
        l: 1.0,
        a: 0.62,
    };
}

/// Home screen focusable state (used inside AppRoot).
#[derive(Debug, Clone)]
pub struct HomeView {
    pub focus_handle: FocusHandle,
}

impl HomeView {
    pub fn new(focus_handle: FocusHandle) -> Self {
        Self { focus_handle }
    }
}

impl Focusable for HomeView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

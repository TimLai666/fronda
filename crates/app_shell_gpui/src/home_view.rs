//! Home screen — focus handle container.
//!
//! The full DOM tree with click handlers is rendered by `AppRoot::render_home`
//! so that handlers can call AppRoot methods directly without an event bridge.

use gpui::{App, FocusHandle, Focusable};

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

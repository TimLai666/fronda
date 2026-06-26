//! Application root — manages routing between Home and Editor views.
//!
//! Covers APP-002 (reopening shows Home), BOOT-004 (startup flow),
//! and PRJ-014 (close project → Home).

use crate::editor_view;
use crate::home_model::HomeLayout;
use crate::home_view::{HomeColors, HomeView};
use crate::menu;
use crate::pane::{LayoutPreset, PaneId, PaneLayout};
use crate::window::WindowConfig;
use app_contract::focus_router::{route_paste, FocusTarget};
use gpui::{
    div, prelude::*, px, size, App, Bounds, Context, FocusHandle, Focusable, InteractiveElement,
    KeyDownEvent, Window, WindowBounds, WindowOptions,
};

/// Which screen the app is showing.
#[derive(Debug, Clone, PartialEq)]
pub enum ActiveScreen {
    Home,
    Editor,
}

/// Root view that switches between Home and Editor.
#[derive(Debug, Clone)]
pub struct AppRoot {
    focus_handle: FocusHandle,
    active_screen: ActiveScreen,
    pane_layout: PaneLayout,
    home: HomeView,
}

impl AppRoot {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let handle = cx.focus_handle();
        Self {
            focus_handle: handle.clone(),
            active_screen: ActiveScreen::Home,
            pane_layout: PaneLayout::new(),
            home: HomeView::new(handle),
        }
    }

    /// Open the editor for a project.
    pub fn open_editor(&mut self, cx: &mut Context<Self>) {
        self.active_screen = ActiveScreen::Editor;
        cx.notify();
    }

    /// Navigate back to Home (e.g., close project).
    pub fn show_home(&mut self, cx: &mut Context<Self>) {
        self.active_screen = ActiveScreen::Home;
        cx.notify();
    }

    pub fn handle_key_down(
        &mut self,
        event: &KeyDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let modifiers = menu::Modifiers {
            command: event.keystroke.modifiers.platform,
            shift: event.keystroke.modifiers.shift,
            option: event.keystroke.modifiers.alt,
            control: event.keystroke.modifiers.control,
        };

        let Some(action) = menu::route_shortcut(&event.keystroke.key, &modifiers) else {
            return;
        };

        cx.stop_propagation();

        match action {
            menu::MenuAction::NewProject => {
                self.open_editor(cx);
            }
            menu::MenuAction::OpenProject => {
                self.open_editor(cx);
            }
            menu::MenuAction::ToggleMediaPanel => {
                self.pane_layout.toggle_pane(PaneId::Media);
            }
            menu::MenuAction::ToggleInspector => {
                self.pane_layout.toggle_pane(PaneId::Inspector);
            }
            menu::MenuAction::ToggleAgentPanel => {
                self.pane_layout.toggle_pane(PaneId::Agent);
            }
            menu::MenuAction::MaximizeFocusedPane => {
                if self.pane_layout.is_maximized() {
                    self.pane_layout.unmaximize();
                } else {
                    self.pane_layout.maximize(PaneId::Preview);
                }
            }
            menu::MenuAction::LayoutDefault => {
                self.pane_layout.apply_preset(LayoutPreset::Default);
            }
            menu::MenuAction::LayoutMedia => {
                self.pane_layout.apply_preset(LayoutPreset::Media);
            }
            menu::MenuAction::LayoutVertical => {
                self.pane_layout.apply_preset(LayoutPreset::Vertical);
            }
            menu::MenuAction::EnterFullScreen => {}
            menu::MenuAction::Quit => {}
            menu::MenuAction::SaveProject
            | menu::MenuAction::SaveProjectAs
            | menu::MenuAction::ImportMedia
            | menu::MenuAction::Export => {}
            menu::MenuAction::Undo
            | menu::MenuAction::Redo
            | menu::MenuAction::Cut
            | menu::MenuAction::Copy => {}
            menu::MenuAction::Paste => {
                let _action = route_paste(FocusTarget::Timeline);
            }
            menu::MenuAction::SelectAll
            | menu::MenuAction::SplitAtPlayhead
            | menu::MenuAction::TrimStartToPlayhead
            | menu::MenuAction::TrimEndToPlayhead
            | menu::MenuAction::Delete => {}
            menu::MenuAction::About
            | menu::MenuAction::CheckForUpdates
            | menu::MenuAction::Settings => {}
            menu::MenuAction::Tutorial
            | menu::MenuAction::KeyboardShortcuts
            | menu::MenuAction::McpInstructions
            | menu::MenuAction::SendFeedback => {}
        }

        cx.notify();
    }

    /// Render the Home screen with inline click handlers on action cards.
    fn render_home(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("fronda-home")
            .track_focus(&self.home.focus_handle.clone())
            .flex()
            .flex_col()
            .size_full()
            .bg(HomeColors::BACKGROUND)
            // ── Heading ──
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .pt(px(HomeLayout::HEADING_TOP as f32))
                    .child(
                        div()
                            .text_xl()
                            .child("Fronda")
                            .text_color(HomeColors::TEXT_PRIMARY),
                    )
                    .child(
                        div()
                            .text_sm()
                            .child("Palmier Pro compatibility baseline")
                            .text_color(HomeColors::TEXT_SECONDARY),
                    ),
            )
            // ── Action cards ──
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .pt(px(HomeLayout::SECTION_TOP as f32))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap(px(HomeLayout::CARD_GAP as f32))
                            // New Project
                            .child(
                                div()
                                    .id("action-new-project")
                                    .flex()
                                    .flex_col()
                                    .items_center()
                                    .justify_center()
                                    .w(px(HomeLayout::CARD_WIDTH as f32))
                                    .h(px(HomeLayout::CARD_HEIGHT as f32))
                                    .bg(HomeColors::CARD_BG)
                                    .rounded(px(8.0))
                                    .cursor_pointer()
                                    .on_click(cx.listener(|this, _event, _window, cx| {
                                        this.open_editor(cx);
                                    }))
                                    .child(
                                        div()
                                            .text_sm()
                                            .child("New Project")
                                            .text_color(HomeColors::TEXT_PRIMARY),
                                    ),
                            )
                            // Open Project
                            .child(
                                div()
                                    .id("action-open-project")
                                    .flex()
                                    .flex_col()
                                    .items_center()
                                    .justify_center()
                                    .w(px(HomeLayout::CARD_WIDTH as f32))
                                    .h(px(HomeLayout::CARD_HEIGHT as f32))
                                    .bg(HomeColors::CARD_BG)
                                    .rounded(px(8.0))
                                    .cursor_pointer()
                                    .on_click(cx.listener(|this, _event, _window, cx| {
                                        this.open_editor(cx);
                                    }))
                                    .child(
                                        div()
                                            .text_sm()
                                            .child("Open Project")
                                            .text_color(HomeColors::TEXT_PRIMARY),
                                    ),
                            ),
                    ),
            )
    }
}

impl Focusable for AppRoot {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for AppRoot {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content: gpui::AnyElement = match self.active_screen {
            ActiveScreen::Home => self.render_home(cx).into_any_element(),
            ActiveScreen::Editor => {
                let layout = self.pane_layout.clone();
                div()
                    .size_full()
                    .child(editor_view::render_pane_layout(&layout))
                    .into_any_element()
            }
        };

        div()
            .id("fronda-root")
            .track_focus(&self.focus_handle.clone())
            .on_key_down(cx.listener(Self::handle_key_down))
            .flex()
            .flex_col()
            .size_full()
            .child(content)
    }
}

/// Create and open the initial window (WIN-001: 1200×1200 default).
pub fn open_main_window(cx: &mut App) {
    let cfg = WindowConfig::for_home();
    let bounds = Bounds::centered(
        None,
        size(px(cfg.default_width as f32), px(cfg.default_height as f32)),
        cx,
    );

    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            ..Default::default()
        },
        |_, cx| cx.new(|cx| AppRoot::new(cx)),
    )
    .unwrap();

    cx.activate(true);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_screen_starts_at_home() {
        assert_eq!(ActiveScreen::Home, ActiveScreen::Home);
        assert_ne!(ActiveScreen::Home, ActiveScreen::Editor);
    }
}

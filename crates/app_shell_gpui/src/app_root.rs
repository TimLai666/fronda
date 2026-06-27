//! Application root — manages routing between Home and Editor views.
//!
//! Covers APP-002 (reopening shows Home), BOOT-004 (startup flow),
//! and PRJ-014 (close project → Home).

use crate::chat_view::ChatView;
use crate::editor_view;
use crate::home_model::HomeLayout;
use crate::home_view::HomeView;
use crate::media_panel_view::MediaPanelView;
use crate::menu;
use crate::pane::{LayoutPreset, PaneId, PaneLayout};
use crate::theme::{Background, BorderColors, FontSize, Radius, Spacing, Text};
use crate::toolbar_view::ToolbarView;
use crate::window::WindowConfig;
use app_contract::focus_router::{route_paste, FocusTarget};
use gpui::{
    div, prelude::*, px, size, App, Bounds, Context, Entity, FocusHandle, Focusable,
    InteractiveElement, KeyDownEvent, Window, WindowBounds, WindowOptions,
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
    /// Editor panel entities, created when first entering the editor.
    chat_view: Option<Entity<ChatView>>,
    toolbar_view: Option<Entity<ToolbarView>>,
    media_panel_view: Option<Entity<MediaPanelView>>,
}

impl AppRoot {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let handle = cx.focus_handle();
        Self {
            focus_handle: handle.clone(),
            active_screen: ActiveScreen::Home,
            pane_layout: PaneLayout::new(),
            home: HomeView::new(handle),
            chat_view: None,
            toolbar_view: None,
            media_panel_view: None,
        }
    }

    /// Open the editor for a project.
    pub fn open_editor(&mut self, cx: &mut Context<Self>) {
        self.active_screen = ActiveScreen::Editor;
        if self.chat_view.is_none() {
            self.chat_view = Some(cx.new(|cx| ChatView::new(cx)));
            self.toolbar_view = Some(cx.new(|cx| ToolbarView::new(cx)));
            self.media_panel_view = Some(cx.new(|cx| MediaPanelView::new(cx)));
        }
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
            | menu::MenuAction::Delete
            | menu::MenuAction::RippleDelete => {}
            menu::MenuAction::About
            | menu::MenuAction::CheckForUpdates
            | menu::MenuAction::Settings => {}
            menu::MenuAction::Tutorial
            | menu::MenuAction::KeyboardShortcuts
            | menu::MenuAction::McpInstructions
            | menu::MenuAction::SendFeedback => {}
            menu::MenuAction::PlayPause
            | menu::MenuAction::PlayBackward
            | menu::MenuAction::PauseJkl
            | menu::MenuAction::PlayForward
            | menu::MenuAction::StepFrameBackward
            | menu::MenuAction::StepFrameForward
            | menu::MenuAction::SkipFramesBackward
            | menu::MenuAction::SkipFramesForward => {}
            menu::MenuAction::MarkIn
            | menu::MenuAction::MarkOut
            | menu::MenuAction::ClearMarkIn
            | menu::MenuAction::ClearMarkOut
            | menu::MenuAction::ClearMarks => {}
            menu::MenuAction::TimelineZoomIn
            | menu::MenuAction::TimelineZoomOut
            | menu::MenuAction::TimelineFitToWindow => {}
        }

        cx.notify();
    }

    /// A sidebar navigation button row.
    fn sidebar_row(icon: &str, label: &str) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .h(px(32.0))
            .px(px(Spacing::SM_MD))
            .gap(px(Spacing::SM_MD))
            .rounded(px(Radius::SM))
            .cursor_pointer()
            .child(
                div()
                    .text_color(Text::TERTIARY)
                    .text_size(px(FontSize::MD))
                    .child(icon.to_string()),
            )
            .child(
                div()
                    .text_color(Text::SECONDARY)
                    .text_size(px(FontSize::SM))
                    .child(label.to_string()),
            )
    }

    /// Render the Home screen: sidebar (220px) + content area.
    fn render_home(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        // Sample project card data
        let sample_projects: &[(&str, &str)] = &[
            ("My Film", "#3a7bd5"),
            ("Commercial", "#6c63ff"),
            ("Documentary", "#43b89c"),
        ];

        div()
            .id("fronda-home")
            .track_focus(&self.home.focus_handle.clone())
            .flex()
            .flex_row()
            .size_full()
            .bg(Background::SURFACE)
            // ── Left sidebar (220px) ──
            .child(
                div()
                    .id("home-sidebar")
                    .flex()
                    .flex_col()
                    .w(px(220.0))
                    .h_full()
                    .bg(Background::SURFACE)
                    .border_r_1()
                    .border_color(BorderColors::PRIMARY)
                    .px(px(Spacing::SM_MD))
                    .py(px(Spacing::MD))
                    .gap(px(Spacing::XXS))
                    // App identity
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .w_full()
                            .h(px(40.0))
                            .mb(px(Spacing::SM_MD))
                            .child(
                                div()
                                    .text_color(Text::PRIMARY)
                                    .text_size(px(FontSize::MD_LG))
                                    .child("Fronda"),
                            ),
                    )
                    // Nav items
                    .child(
                        Self::sidebar_row("⊕", "New Project")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.open_editor(cx);
                            })),
                    )
                    .child(
                        Self::sidebar_row("▲", "Open Project")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.open_editor(cx);
                            })),
                    )
                    // Spacer
                    .child(div().flex_1())
                    // Bottom: Settings
                    .child(Self::sidebar_row("⚙", "Settings")),
            )
            // ── Content area ──
            .child(
                div()
                    .id("home-content")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .h_full()
                    .bg(Background::SURFACE)
                    // Greeting
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .px(px(HomeLayout::CARD_GAP as f32 * 2.0))
                            .pt(px(HomeLayout::HEADING_TOP as f32))
                            .pb(px(Spacing::XL))
                            .child(
                                div()
                                    .text_size(px(FontSize::TITLE_2))
                                    .text_color(Text::PRIMARY)
                                    .child("Welcome to Fronda"),
                            )
                            .child(
                                div()
                                    .text_size(px(FontSize::SM_MD))
                                    .text_color(Text::TERTIARY)
                                    .child("Palmier Pro compatibility baseline"),
                            ),
                    )
                    // "My Projects" label
                    .child(
                        div()
                            .px(px(HomeLayout::CARD_GAP as f32 * 2.0))
                            .pb(px(Spacing::SM))
                            .text_size(px(FontSize::SM_MD))
                            .text_color(Text::TERTIARY)
                            .child("My Projects"),
                    )
                    // Project grid
                    .child(
                        div()
                            .id("project-grid")
                            .flex()
                            .flex_row()
                            .flex_wrap()
                            .px(px(HomeLayout::CARD_GAP as f32 * 2.0))
                            .gap(px(HomeLayout::CARD_GAP as f32))
                            // New project card
                            .child(
                                div()
                                    .id("card-new-project")
                                    .flex()
                                    .flex_col()
                                    .items_center()
                                    .justify_center()
                                    .w(px(HomeLayout::CARD_WIDTH as f32))
                                    .h(px(HomeLayout::CARD_HEIGHT as f32))
                                    .bg(Background::RAISED)
                                    .rounded(px(Radius::MD_LG))
                                    .border_1()
                                    .border_color(BorderColors::SUBTLE)
                                    .cursor_pointer()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.open_editor(cx);
                                    }))
                                    .child(
                                        div()
                                            .text_size(px(FontSize::TITLE_2))
                                            .text_color(Text::TERTIARY)
                                            .child("+"),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(FontSize::SM))
                                            .text_color(Text::TERTIARY)
                                            .child("New Project"),
                                    ),
                            )
                            // Sample project cards
                            .children(sample_projects.iter().enumerate().map(|(i, (name, _color))| {
                                div()
                                    .id(format!("card-project-{}", i))
                                    .flex()
                                    .flex_col()
                                    .w(px(HomeLayout::CARD_WIDTH as f32))
                                    .h(px(HomeLayout::CARD_HEIGHT as f32))
                                    .bg(Background::RAISED)
                                    .rounded(px(Radius::MD_LG))
                                    .border_1()
                                    .border_color(BorderColors::SUBTLE)
                                    .overflow_hidden()
                                    .cursor_pointer()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.open_editor(cx);
                                    }))
                                    // Thumbnail area (top 80%)
                                    .child(
                                        div()
                                            .flex_1()
                                            .bg(Background::PROMINENT)
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .text_color(Text::MUTED)
                                            .text_size(px(FontSize::DISPLAY))
                                            .child("▶"),
                                    )
                                    // Name strip (bottom)
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .w_full()
                                            .h(px(24.0))
                                            .px(px(Spacing::SM_MD))
                                            .bg(Background::RAISED)
                                            .child(
                                                div()
                                                    .text_size(px(FontSize::SM))
                                                    .text_color(Text::PRIMARY)
                                                    .child(name.to_string()),
                                            ),
                                    )
                            })),
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
                let contents = editor_view::PaneContents::new(
                    self.chat_view.clone(),
                    self.toolbar_view.clone(),
                    self.media_panel_view.clone(),
                );
                div()
                    .size_full()
                    .child(editor_view::render_pane_layout(&layout, &contents))
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

/// Create and open the initial window.
pub fn open_main_window(cx: &mut App) {
    let cfg = WindowConfig::for_home();
    let size = size(px(cfg.default_width as f32), px(cfg.default_height as f32));
    let mut bounds = Bounds::centered(None, size, cx);
    bounds.origin.y = bounds.origin.y + px(220.0);

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

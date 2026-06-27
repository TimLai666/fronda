//! Custom editor title bar — matches Swift TitleBarLeadingView + TitleBarTrailingView.
//!
//! Layout:
//!   [chat bubble toggle]  ───  project name  ───  [Export] [avatar]
//!
//! macOS has a native NSToolbar; the gpui cross-platform version uses a 28px strip.

use crate::pane::{PaneId, PaneLayout};
use crate::theme::{Accent, Background, BorderColors, FontSize, Layout, Opacity, Radius, Spacing, Text};
use gpui::Hsla;
use gpui::{
    div, prelude::*, px, App, ClickEvent, Context, FocusHandle, Focusable, InteractiveElement,
    ParentElement, Render, SharedString, Styled, Window,
};

/// State carried by the title bar.
#[derive(Debug, Clone)]
pub struct TitleBarState {
    pub project_name: SharedString,
    pub agent_panel_visible: bool,
}

impl Default for TitleBarState {
    fn default() -> Self {
        Self {
            project_name: "Untitled Project".into(),
            agent_panel_visible: true,
        }
    }
}

/// Title bar view entity.
pub struct TitleBarView {
    pub state: TitleBarState,
    focus_handle: FocusHandle,
}

impl TitleBarView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            state: TitleBarState::default(),
            focus_handle: cx.focus_handle(),
        }
    }

    /// Returns the color of the chat-bubble icon — bright when panel is open.
    fn agent_icon_color(&self) -> Hsla {
        if self.state.agent_panel_visible {
            Accent::PRIMARY
        } else {
            Hsla {
                h: 0.0,
                s: 0.0,
                l: 1.0,
                a: Opacity::STRONG,
            }
        }
    }
}

impl Focusable for TitleBarView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TitleBarView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let icon_color = self.agent_icon_color();
        let project_name = self.state.project_name.clone();

        div()
            .id("titlebar")
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .h(px(Layout::PANEL_HEADER_HEIGHT))
            .bg(Background::RAISED)
            .border_b_1()
            .border_color(BorderColors::PRIMARY)
            // ── Leading: agent toggle (matches TitleBarLeadingView) ──
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .px(px(Spacing::SM_MD))
                    .gap(px(Spacing::SM_MD))
                    .child(
                        div()
                            .id("btn-agent-toggle")
                            .w(px(26.0))
                            .h(px(26.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(Radius::SM))
                            .cursor_pointer()
                            .text_color(icon_color)
                            .text_size(px(FontSize::MD))
                            .on_click(cx.listener(
                                |this: &mut TitleBarView,
                                 _event: &ClickEvent,
                                 _window: &mut Window,
                                 cx: &mut Context<TitleBarView>| {
                                    this.state.agent_panel_visible =
                                        !this.state.agent_panel_visible;
                                    cx.notify();
                                },
                            ))
                            .child("✦"),
                    ),
            )
            // ── Center: project name ──
            .child(
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(Text::SECONDARY)
                    .text_size(px(FontSize::SM))
                    .child(project_name.to_string()),
            )
            // ── Trailing: Export button + account avatar ──
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .px(px(Spacing::SM_MD))
                    .gap(px(Spacing::SM))
                    // Export button (matches TitleBarTrailingView)
                    .child(
                        div()
                            .id("btn-titlebar-export")
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(Spacing::XS))
                            .px(px(Spacing::SM))
                            .h(px(26.0))
                            .rounded(px(Radius::SM))
                            .border_1()
                            .border_color(BorderColors::SUBTLE)
                            .cursor_pointer()
                            .text_color(Text::SECONDARY)
                            .text_size(px(FontSize::SM))
                            .child("↑ Export"),
                    )
                    // Account avatar circle (matches UserAvatarButton)
                    .child(
                        div()
                            .id("btn-account-avatar")
                            .w(px(22.0))
                            .h(px(22.0))
                            .rounded_full()
                            .bg(Accent::PRIMARY)
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .text_color(Background::BASE)
                            .text_size(px(FontSize::XXS))
                            .child("P"),
                    ),
            )
    }
}

/// Synchronise the title bar's agent-panel-visible flag from the PaneLayout.
/// Call this whenever the layout changes.
pub fn sync_agent_toggle(bar: &mut TitleBarView, layout: &PaneLayout) {
    bar.state.agent_panel_visible = layout.is_visible(PaneId::Agent);
}

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
    div, prelude::*, px, svg, App, ClickEvent, Context, FocusHandle, Focusable,
    InteractiveElement, ParentElement, Render, SharedString, Styled, Window,
};

/// State carried by the title bar.
#[derive(Debug, Clone)]
pub struct TitleBarState {
    pub project_name: SharedString,
    pub agent_panel_visible: bool,
    /// Update badge: None = no update, Some(version) = update available.
    pub update_version: Option<SharedString>,
    /// Signed-in user display initial (None = not signed in, shows person icon).
    pub account_initial: Option<char>,
}

impl Default for TitleBarState {
    fn default() -> Self {
        Self {
            project_name: "Untitled Project".into(),
            agent_panel_visible: true,
            update_version: None,
            account_initial: None,
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

    /// Returns the color of the chat-bubble icon.
    /// Swift uses aiGradient (monochrome silver shimmer: white→0.78→0.60→white).
    /// We approximate with l=0.78 (mid-point silver) at full opacity when panel is
    /// open, or dimmed when closed — closer to the gradient's average tone than pure white.
    fn agent_icon_color(&self) -> Hsla {
        Hsla {
            h: 0.0,
            s: 0.0,
            l: if self.state.agent_panel_visible { 0.78 } else { 0.50 },
            a: 1.0,
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
                            // bubble.left / bubble.left.fill equivalent via embedded SVG
                            .child(
                                svg()
                                    .path("icons/chat.svg")
                                    .w(px(14.0))
                                    .h(px(14.0))
                                    .text_color(icon_color),
                            ),
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
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .child(project_name.to_string()),
            )
            // ── Trailing: update badge + Export button + account avatar ──
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .px(px(Spacing::SM_MD))
                    .gap(px(Spacing::SM))
                    // Update badge — visible only when update_version is Some (Swift UpdateBadgeView)
                    .when_some(self.state.update_version.clone(), |el, ver| {
                        el.child(
                            div()
                                .id("badge-update")
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(0.0))
                                .rounded_full()
                                .border_1()
                                .border_color(BorderColors::SUBTLE)
                                .overflow_hidden()
                                .child(
                                    div()
                                        .px(px(Spacing::SM))
                                        .py(px(Spacing::XXS))
                                        .text_color(Text::PRIMARY)
                                        .text_size(px(FontSize::XS))
                                        .cursor_pointer()
                                        .child(format!("Update v{ver}")),
                                )
                                .child(
                                    div()
                                        .px(px(Spacing::XS))
                                        .py(px(Spacing::XXS))
                                        .text_color(Text::TERTIARY)
                                        .text_size(px(FontSize::XS))
                                        .cursor_pointer()
                                        .child("✕"),
                                ),
                        )
                    })
                    // Export button (matches TitleBarTrailingView — square.and.arrow.up)
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
                            .on_click(cx.listener(|_, _, _, _| {}))
                            .child(
                                svg()
                                    .path("icons/export.svg")
                                    .w(px(11.0))
                                    .h(px(11.0))
                                    .text_color(Text::SECONDARY),
                            )
                            .child(
                                div()
                                    .text_color(Text::SECONDARY)
                                    .text_size(px(FontSize::SM))
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .child("Export"),
                            ),
                    )
                    // Account avatar (Swift: UserAvatarButton).
                    // Signed in: accent circle + display initial.
                    // Signed out: white@soft circle + ⊙ person approximation.
                    .child({
                        let signed_in = self.state.account_initial.is_some();
                        let initial_str = self.state.account_initial
                            .map(|c| c.to_string())
                            .unwrap_or_else(|| "⊙".to_string());
                        div()
                            .id("btn-account-avatar")
                            .w(px(22.0))
                            .h(px(22.0))
                            .rounded_full()
                            .bg(if signed_in {
                                Hsla { h: Accent::PRIMARY.h, s: Accent::PRIMARY.s, l: Accent::PRIMARY.l, a: 0.5 }
                            } else {
                                Hsla { h: 0.0, s: 0.0, l: 1.0, a: 0.12 }
                            })
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .on_click(cx.listener(|_, _, _, _| {}))
                            .text_color(if signed_in { Text::PRIMARY } else { Text::TERTIARY })
                            .text_size(px(FontSize::XXS))
                            .child(initial_str)
                    }),
            )
    }
}

/// Synchronise the title bar's agent-panel-visible flag from the PaneLayout.
/// Call this whenever the layout changes.
pub fn sync_agent_toggle(bar: &mut TitleBarView, layout: &PaneLayout) {
    bar.state.agent_panel_visible = layout.is_visible(PaneId::Agent);
}

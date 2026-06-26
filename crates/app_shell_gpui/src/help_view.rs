//! Help gpui view — renders the Help window with Shortcuts and MCP tabs.
//!
//! Requires the `desktop-app` feature (gpui).

use app_contract::help_model::{HelpTab, HelpViewModel};
use gpui::{
    div, prelude::*, px, App, Context, FocusHandle, Focusable, Hsla, InteractiveElement,
    ParentElement, Render, Styled, Window,
};

/// Colors for the help view.
pub struct HelpColors;
impl HelpColors {
    pub const BACKGROUND: Hsla = Hsla {
        h: 0.0,
        s: 0.0,
        l: 0.07,
        a: 1.0,
    };
    pub const TAB_BG: Hsla = Hsla {
        h: 0.0,
        s: 0.0,
        l: 0.12,
        a: 1.0,
    };
    pub const TAB_ACTIVE_BG: Hsla = Hsla {
        h: 210.0 / 360.0,
        s: 0.5,
        l: 0.2,
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
    pub const SIDEBAR_BG: Hsla = Hsla {
        h: 0.0,
        s: 0.0,
        l: 0.1,
        a: 1.0,
    };
}

/// gpui Help view component.
#[derive(Debug, Clone)]
pub struct HelpView {
    focus_handle: FocusHandle,
    model: HelpViewModel,
}

impl HelpView {
    pub fn new(mcp_port: u16, cx: &mut Context<Self>) -> Self {
        let handle = cx.focus_handle();
        Self {
            focus_handle: handle,
            model: HelpViewModel::new(mcp_port),
        }
    }
}

impl Focusable for HelpView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for HelpView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let mut sidebar = div().flex().flex_col().gap(px(4.0)).w(px(220.0));

        for tab in HelpTab::ALL {
            let is_active = self.model.active_tab == *tab;
            let bg = if is_active {
                HelpColors::TAB_ACTIVE_BG
            } else {
                HelpColors::TAB_BG
            };
            let color = if is_active {
                HelpColors::TEXT_PRIMARY
            } else {
                HelpColors::TEXT_SECONDARY
            };

            sidebar = sidebar.child(
                div()
                    .id(gpui::SharedString::from(format!(
                        "help-tab-{}",
                        tab.label()
                    )))
                    .px(px(12.0))
                    .py(px(8.0))
                    .rounded(px(4.0))
                    .bg(bg)
                    .cursor_pointer()
                    .child(div().text_sm().child(tab.label()).text_color(color)),
            );
        }

        let content = match self.model.active_tab {
            HelpTab::Shortcuts => div().flex_1().child(
                div().px(px(16.0)).py(px(16.0)).child(
                    div()
                        .text_sm()
                        .child("Keyboard Shortcuts")
                        .text_color(HelpColors::TEXT_PRIMARY),
                ),
            ),
            HelpTab::Mcp => div().flex_1().child(
                div()
                    .px(px(16.0))
                    .py(px(16.0))
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_sm()
                            .child("MCP Server")
                            .text_color(HelpColors::TEXT_PRIMARY),
                    )
                    .child(
                        div()
                            .text_xs()
                            .child(format!("Endpoint: {}", self.model.mcp_endpoint()))
                            .text_color(HelpColors::TEXT_SECONDARY),
                    ),
            ),
        };

        div()
            .id("fronda-help")
            .track_focus(&self.focus_handle.clone())
            .flex()
            .flex_row()
            .size_full()
            .bg(HelpColors::BACKGROUND)
            .child(div().bg(HelpColors::SIDEBAR_BG).child(sidebar))
            .child(content)
    }
}

//! Help gpui view — renders the Help window with Shortcuts and MCP tabs.
//!
//! Requires the `desktop-app` feature (gpui).

use app_contract::help_model::{HelpTab, HelpViewModel};
use crate::theme::{Background, BorderColors, FontSize, Radius, Spacing, Text};
use gpui::{
    div, prelude::*, px, App, Context, FocusHandle, Focusable,
    ParentElement, Render, Styled, Window,
};

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
        let active = self.model.active_tab;

        let mut sidebar = div()
            .flex()
            .flex_col()
            .w(px(200.0))
            .h_full()
            .bg(Background::SURFACE)
            .border_r_1()
            .border_color(BorderColors::PRIMARY)
            .py(px(Spacing::MD))
            .gap(px(Spacing::XXS));

        for tab in HelpTab::ALL {
            let is_active = self.model.active_tab == *tab;
            sidebar = sidebar.child(
                div()
                    .id(gpui::SharedString::from(format!(
                        "help-tab-{}",
                        tab.label()
                    )))
                    .flex()
                    .flex_row()
                    .items_center()
                    .w_full()
                    .h(px(32.0))
                    .px(px(Spacing::MD_LG))
                    .rounded(px(Radius::SM))
                    .cursor_pointer()
                    .bg(if is_active {
                        BorderColors::PRIMARY
                    } else {
                        Background::SURFACE
                    })
                    .child(
                        div()
                            .text_size(px(FontSize::SM))
                            .text_color(if is_active { Text::PRIMARY } else { Text::SECONDARY })
                            .child(tab.label()),
                    ),
            );
        }

        let content = match active {
            HelpTab::Shortcuts => div()
                .flex_1()
                .flex()
                .flex_col()
                .px(px(Spacing::XL))
                .py(px(Spacing::XL))
                .gap(px(Spacing::MD))
                .child(
                    div()
                        .text_size(px(FontSize::MD_LG))
                        .text_color(Text::PRIMARY)
                        .child("Keyboard Shortcuts"),
                )
                .child(
                    div()
                        .text_size(px(FontSize::SM))
                        .text_color(Text::TERTIARY)
                        .child("Common shortcuts for editing and navigation."),
                )
                .into_any_element(),
            HelpTab::Mcp => div()
                .flex_1()
                .flex()
                .flex_col()
                .px(px(Spacing::XL))
                .py(px(Spacing::XL))
                .gap(px(Spacing::MD))
                .child(
                    div()
                        .text_size(px(FontSize::MD_LG))
                        .text_color(Text::PRIMARY)
                        .child("MCP Server"),
                )
                .child(
                    div()
                        .text_size(px(FontSize::SM))
                        .text_color(Text::TERTIARY)
                        .child(format!("Endpoint: {}", self.model.mcp_endpoint())),
                )
                .into_any_element(),
        };

        div()
            .id("fronda-help")
            .track_focus(&self.focus_handle.clone())
            .flex()
            .flex_row()
            .size_full()
            .bg(Background::BASE)
            .child(sidebar)
            .child(content)
    }
}

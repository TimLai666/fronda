//! Settings gpui view — renders the Settings window with tab navigation.
//!
//! Requires the `desktop-app` feature (gpui).

use app_contract::settings_model::SettingsWindowModel;
use app_contract::settings_storage::SettingsTab;
use crate::theme::{Background, BorderColors, FontSize, Radius, Spacing, Text};
use gpui::{
    div, prelude::*, px, App, Context, FocusHandle, Focusable, InteractiveElement,
    ParentElement, Render, Styled, Window,
};

/// gpui Settings view component.
#[derive(Debug, Clone)]
pub struct SettingsView {
    focus_handle: FocusHandle,
    model: SettingsWindowModel,
}

impl SettingsView {
    pub fn new(backend_configured: bool, cx: &mut Context<Self>) -> Self {
        let handle = cx.focus_handle();
        Self {
            focus_handle: handle,
            model: SettingsWindowModel::new(SettingsTab::General, backend_configured),
        }
    }
}

impl Focusable for SettingsView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

/// A settings row with a label and value.
fn settings_row(label: &str, value: &str) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .w_full()
        .h(px(32.0))
        .px(px(Spacing::LG))
        .child(
            div()
                .flex_1()
                .text_color(Text::SECONDARY)
                .text_size(px(FontSize::SM))
                .child(label.to_string()),
        )
        .child(
            div()
                .text_color(Text::TERTIARY)
                .text_size(px(FontSize::SM))
                .child(value.to_string()),
        )
}

impl Render for SettingsView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let active_tab = self.model.active_tab;

        let mut sidebar = div()
            .flex()
            .flex_col()
            .w(px(180.0))
            .h_full()
            .bg(Background::SURFACE)
            .border_r_1()
            .border_color(BorderColors::PRIMARY)
            .py(px(Spacing::MD));

        for tab in SettingsTab::ALL {
            if !self.model.is_tab_visible(tab) {
                continue;
            }
            let is_active = self.model.active_tab == *tab;
            sidebar = sidebar.child(
                div()
                    .id(gpui::SharedString::from(format!(
                        "settings-tab-{}",
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

        // Content area for the active tab
        let content = div()
            .flex()
            .flex_col()
            .flex_1()
            .h_full()
            .bg(Background::SURFACE)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .px(px(Spacing::XL_XXL))
                    .py(px(Spacing::XL))
                    .gap(px(Spacing::MD))
                    .child(
                        div()
                            .text_color(Text::SECONDARY)
                            .text_size(px(FontSize::LG))
                            .child(active_tab.label()),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .rounded(px(Radius::SM))
                            .border_1()
                            .border_color(BorderColors::SUBTLE)
                            .overflow_hidden()
                            .child(settings_row("Version", "1.0.0"))
                            .child(
                                div()
                                    .w_full()
                                    .h(px(1.0))
                                    .bg(BorderColors::SUBTLE),
                            )
                            .child(settings_row("Build", "fronda-core"))
                            .child(
                                div()
                                    .w_full()
                                    .h(px(1.0))
                                    .bg(BorderColors::SUBTLE),
                            )
                            .child(settings_row("Language", "en-US")),
                    ),
            );

        div()
            .id("fronda-settings")
            .track_focus(&self.focus_handle.clone())
            .flex()
            .flex_row()
            .size_full()
            .bg(Background::BASE)
            .child(sidebar)
            .child(content)
    }
}

//! Settings gpui view — renders the Settings window with tab navigation.
//!
//! Requires the `desktop-app` feature (gpui).

use app_contract::settings_model::SettingsWindowModel;
use app_contract::settings_storage::SettingsTab;
use gpui::{
    div, prelude::*, px, App, Context, FocusHandle, Focusable, Hsla, InteractiveElement,
    ParentElement, Render, Styled, Window,
};

/// Colors for the settings view.
pub struct SettingsColors;
impl SettingsColors {
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
}

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

impl Render for SettingsView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let mut sidebar = div().flex().flex_col().gap(px(4.0)).w(px(200.0));

        for tab in SettingsTab::ALL {
            if !self.model.is_tab_visible(tab) {
                continue;
            }
            let is_active = self.model.active_tab == *tab;
            let bg = if is_active {
                SettingsColors::TAB_ACTIVE_BG
            } else {
                SettingsColors::TAB_BG
            };
            let color = if is_active {
                SettingsColors::TEXT_PRIMARY
            } else {
                SettingsColors::TEXT_SECONDARY
            };

            sidebar = sidebar.child(
                div()
                    .id(gpui::SharedString::from(format!(
                        "settings-tab-{}",
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

        div()
            .id("fronda-settings")
            .track_focus(&self.focus_handle.clone())
            .flex()
            .flex_row()
            .size_full()
            .bg(SettingsColors::BACKGROUND)
            .child(sidebar)
    }
}

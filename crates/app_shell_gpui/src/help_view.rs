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

// ── Shortcut data matching Swift ShortcutsPane.allShortcuts ──
const SHORTCUT_GROUPS: &[(&str, &[(&str, &str)])] = &[
    ("Playback", &[
        ("Space", "Play / Pause"),
        ("←", "Step Backward"),
        ("→", "Step Forward"),
        ("Shift + ←", "Skip Backward"),
        ("Shift + →", "Skip Forward"),
    ]),
    ("Tools", &[
        ("V", "Selection Tool"),
        ("C", "Razor Tool"),
    ]),
    ("Editing", &[
        ("Cmd + K", "Split at Playhead"),
        ("[ or Q", "Trim Start to Playhead"),
        ("] or W", "Trim End to Playhead"),
        ("Backspace", "Delete"),
        ("Shift + Backspace", "Ripple Delete"),
        ("Opt + Drag", "Duplicate Clip"),
    ]),
    ("Timeline", &[
        ("Shift + Drag Ruler", "Select Range"),
        ("Drag Range Edge", "Adjust Range"),
        ("I", "Mark Range Start"),
        ("O", "Mark Range End"),
        ("Opt + Scroll", "Zoom to Cursor"),
        ("Pinch", "Zoom to Cursor"),
        ("Cmd + Scroll", "Scroll Horizontally"),
    ]),
    ("File", &[
        ("Cmd + N", "New"),
        ("Cmd + O", "Open"),
        ("Cmd + S", "Save"),
        ("Cmd + Shift + S", "Save As"),
        ("Cmd + I", "Import Media"),
        ("Cmd + E", "Export"),
    ]),
    ("Edit", &[
        ("Cmd + Z", "Undo"),
        ("Cmd + Shift + Z", "Redo"),
        ("Cmd + X", "Cut"),
        ("Cmd + C", "Copy"),
        ("Cmd + V", "Paste"),
        ("Cmd + A", "Select All"),
    ]),
    ("View", &[
        ("Cmd + F", "Full Screen"),
        ("`", "Maximize Focused Panel"),
        ("Cmd + Scroll", "Zoom Preview to Cursor"),
        ("Esc", "Deselect & Reset Tool"),
    ]),
];

/// Shortcut group column: title + rows. Matches Swift shortcutColumn.
fn shortcut_column(groups: &[(&str, &[(&str, &str)])]) -> impl IntoElement {
    let mut col = div()
        .flex()
        .flex_col()
        .flex_1()
        .gap(px(20.0));

    for (title, rows) in groups {
        let mut group = div()
            .flex()
            .flex_col()
            .gap(px(8.0))
            // Section title: 10pt semibold uppercase TERTIARY
            .child(
                div()
                    .text_color(Text::TERTIARY)
                    .text_size(px(10.0))
                    .child(title.to_uppercase()),
            );

        for (shortcut, desc) in *rows {
            group = group.child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(10.0))
                    // Key: 118px, caption2 monospaced, semibold, PRIMARY
                    .child(
                        div()
                            .w(px(118.0))
                            .text_color(Text::PRIMARY)
                            .text_size(px(FontSize::XXS))
                            .child(shortcut.to_string()),
                    )
                    // Description: 11pt, SECONDARY
                    .child(
                        div()
                            .text_color(Text::SECONDARY)
                            .text_size(px(11.0))
                            .child(desc.to_string()),
                    ),
            );
        }
        col = col.child(group);
    }
    col
}

/// Shortcuts pane: 2 columns (left = first 4 groups, right = last 3).
fn shortcuts_pane() -> impl IntoElement {
    let left = &SHORTCUT_GROUPS[..4];
    let right = &SHORTCUT_GROUPS[4..];

    div()
        .id("shortcuts-pane-scroll")
        .flex_1()
        .overflow_y_scroll()
        .child(
            div()
                .flex()
                .flex_row()
                .items_start()
                .gap(px(24.0))
                .px(px(20.0))
                .py(px(20.0))
                .child(shortcut_column(left))
                .child(shortcut_column(right)),
        )
}

/// MCP instructions pane.
fn mcp_pane(endpoint: &str) -> impl IntoElement {
    div()
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
                .text_color(Text::SECONDARY)
                .child("Connect your AI assistant to Fronda's MCP server to control the editor programmatically."),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(Spacing::XS))
                .child(
                    div()
                        .text_size(px(FontSize::XS))
                        .text_color(Text::TERTIARY)
                        .child("ENDPOINT"),
                )
                .child(
                    div()
                        .px(px(Spacing::SM_MD))
                        .py(px(Spacing::SM))
                        .rounded(px(crate::theme::Radius::SM))
                        .border_1()
                        .border_color(BorderColors::SUBTLE)
                        .bg(crate::theme::Background::RAISED)
                        .text_size(px(FontSize::SM))
                        .text_color(Text::PRIMARY)
                        .child(endpoint.to_string()),
                ),
        )
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
            HelpTab::Shortcuts => shortcuts_pane().into_any_element(),
            HelpTab::Mcp => mcp_pane(&self.model.mcp_endpoint()).into_any_element(),
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

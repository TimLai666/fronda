//! Home screen — focus handle container + the shared sidebar row (#319).
//!
//! The full DOM tree with click handlers is rendered by `AppRoot::render_home`
//! so that handlers can call AppRoot methods directly without an event bridge.

use crate::theme::{FontSize, IconSize, Opacity, Radius, Spacing, Text};
use gpui::{div, prelude::*, px, svg, App, FocusHandle, Focusable, Hsla};

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

/// Selected-row fill (Swift HoverHighlight `(isActive, !hovered)`).
const ROW_ACTIVE_FILL: Hsla = Hsla {
    h: 0.0,
    s: 0.0,
    l: 1.0,
    a: Opacity::SOFT,
};

/// Hover fill (Swift HoverHighlight `(!isActive, hovered)`).
const ROW_HOVER_FILL: Hsla = Hsla {
    h: 0.0,
    s: 0.0,
    l: 1.0,
    a: Opacity::FAINT,
};

/// Unified sidebar row (Swift `SidebarRowButton`, #319): icon in an
/// `IconSize.sm` frame, `mdLg` regular label, primary text, capsule-radius
/// hover highlight. Shared by the Home and Settings sidebars; the caller
/// attaches `on_click`.
pub fn sidebar_row_button(
    id: impl Into<gpui::SharedString>,
    icon_path: &'static str,
    label: &str,
    is_selected: bool,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id.into())
        .flex()
        .flex_row()
        .items_center()
        .gap(px(Spacing::SM_MD))
        .w_full()
        .px(px(Spacing::MD))
        .py(px(Spacing::SM))
        .rounded(px(Radius::XL))
        .cursor_pointer()
        .when(is_selected, |el| el.bg(ROW_ACTIVE_FILL))
        .when(!is_selected, |el| {
            el.hover(|s| s.bg(ROW_HOVER_FILL))
        })
        .child(
            div()
                .w(px(IconSize::SM))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    svg()
                        .path(icon_path)
                        .w(px(FontSize::MD))
                        .h(px(FontSize::MD))
                        .text_color(Text::PRIMARY),
                ),
        )
        .child(
            div()
                .text_color(Text::PRIMARY)
                .text_size(px(FontSize::MD_LG))
                .child(label.to_string()),
        )
}

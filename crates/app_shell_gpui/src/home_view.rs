//! Home gpui view — renders the Home screen with project actions.
//!
//! Requires the `desktop-app` feature (gpui).

use crate::home_model::HomeLayout;
use gpui::{
    div, prelude::*, px, App, Context, FocusHandle, Focusable, Hsla, InteractiveElement,
    ParentElement, Render, Styled, Window,
};

/// Colors for the home view.
pub struct HomeColors;
impl HomeColors {
    pub const BACKGROUND: Hsla = Hsla {
        h: 0.0,
        s: 0.0,
        l: 0.07,
        a: 1.0,
    };
    pub const CARD_BG: Hsla = Hsla {
        h: 0.0,
        s: 0.0,
        l: 0.12,
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

/// gpui Home view component.
#[derive(Debug, Clone)]
pub struct HomeView {
    focus_handle: FocusHandle,
}

impl HomeView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let handle = cx.focus_handle();
        // focus handled by gpui
        Self {
            focus_handle: handle,
        }
    }
}

impl Focusable for HomeView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for HomeView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("fronda-home")
            .track_focus(&self.focus_handle.clone())
            .flex()
            .flex_col()
            .size_full()
            .bg(HomeColors::BACKGROUND)
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
                                    .child(
                                        div()
                                            .text_sm()
                                            .child("New Project")
                                            .text_color(HomeColors::TEXT_PRIMARY),
                                    ),
                            )
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

//! TourOverlay — matches Swift TourOverlay.
//!
//! Full-screen overlay rendered on top of the app window during onboarding.
//!
//! Three card types:
//!   • Intro: 600px wide, title + instruction + hero image + Skip/Next buttons
//!   • Spotlight callout: 320px wide, "Step N of M" + title + instruction + Skip/Back/Next
//!   • Outro: 600px wide, completion message + Done button
//!
//! Scrim: semi-transparent black (Opacity::STRONG) covers the full screen.
//! Spotlight step would additionally show a gradient border cutout (not implemented here).

use crate::theme::{Accent, Background, BorderColors, FontSize, Opacity, Radius, Spacing, Text};
use gpui::{
    div, prelude::*, px, App, ClickEvent, Context, FocusHandle, Focusable, IntoElement,
    ParentElement, Render, Styled, Window,
};

#[derive(Debug, Clone, PartialEq)]
pub enum TourCardKind {
    Intro,
    Spotlight { step: u32, total: u32 },
    Outro,
}

#[derive(Debug, Clone)]
pub struct TourOverlayState {
    pub card: TourCardKind,
    pub title: String,
    pub instruction: String,
    pub visible: bool,
}

impl Default for TourOverlayState {
    fn default() -> Self {
        Self {
            card: TourCardKind::Intro,
            title: "Welcome to Fronda".to_string(),
            instruction: "A cross-platform video editor built with Rust and gpui.".to_string(),
            visible: true,
        }
    }
}

pub struct TourOverlayView {
    pub state: TourOverlayState,
    focus_handle: FocusHandle,
}

impl TourOverlayView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            state: TourOverlayState::default(),
            focus_handle: cx.focus_handle(),
        }
    }
}

impl Focusable for TourOverlayView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

fn tour_button(label: &str, is_primary: bool) -> impl IntoElement {
    div()
        .px(px(Spacing::MD_LG))
        .py(px(Spacing::SM))
        .rounded_full()
        .cursor_pointer()
        .when(is_primary, |el| {
            el.bg(Accent::PRIMARY)
              .text_color(Background::BASE)
        })
        .when(!is_primary, |el| {
            el.border_1()
              .border_color(BorderColors::PRIMARY)
              .text_color(Text::SECONDARY)
        })
        .text_size(px(FontSize::SM))
        .child(label.to_string())
}

fn intro_card(title: &str, instruction: &str) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(Spacing::MD_LG))
        .w(px(600.0))
        .rounded(px(Radius::LG))
        .bg(Background::RAISED)
        .p(px(Spacing::XL))
        .border_1()
        .border_color(BorderColors::SUBTLE)
        // Title
        .child(
            div()
                .text_color(Text::PRIMARY)
                .text_size(px(FontSize::XL))
                .child(title.to_string()),
        )
        // Instruction
        .child(
            div()
                .text_color(Text::SECONDARY)
                .text_size(px(FontSize::SM_MD))
                .child(instruction.to_string()),
        )
        // Hero image placeholder (300px tall)
        .child(
            div()
                .w_full()
                .h(px(180.0))
                .rounded(px(Radius::MD))
                .bg(Background::SURFACE)
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_color(Text::MUTED)
                        .text_size(px(FontSize::MD))
                        .child("[ Preview ]"),
                ),
        )
        // Buttons row
        .child(
            div()
                .flex()
                .flex_row()
                .justify_end()
                .gap(px(Spacing::SM))
                .child(tour_button("Skip", false))
                .child(tour_button("Get Started", true)),
        )
}

fn callout_card(step: u32, total: u32, title: &str, instruction: &str) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(Spacing::MD))
        .w(px(320.0))
        .rounded(px(Radius::MD))
        .bg(Background::RAISED)
        .p(px(Spacing::LG))
        .border_1()
        .border_color(BorderColors::SUBTLE)
        // Step indicator
        .child(
            div()
                .text_color(Text::TERTIARY)
                .text_size(px(FontSize::XS))
                .child(format!("Step {} of {}", step, total)),
        )
        // Title
        .child(
            div()
                .text_color(Text::PRIMARY)
                .text_size(px(FontSize::MD))
                .child(title.to_string()),
        )
        // Instruction
        .child(
            div()
                .text_color(Text::SECONDARY)
                .text_size(px(FontSize::SM_MD))
                .child(instruction.to_string()),
        )
        // Buttons
        .child(
            div()
                .flex()
                .flex_row()
                .justify_end()
                .gap(px(Spacing::SM))
                .child(tour_button("Skip", false))
                .child(tour_button("Back", false))
                .child(tour_button("Next", true)),
        )
}

fn outro_card(title: &str, instruction: &str) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(Spacing::MD_LG))
        .w(px(600.0))
        .rounded(px(Radius::LG))
        .bg(Background::RAISED)
        .p(px(Spacing::XL))
        .border_1()
        .border_color(BorderColors::SUBTLE)
        .child(
            div()
                .text_color(Text::PRIMARY)
                .text_size(px(FontSize::XL))
                .child(title.to_string()),
        )
        .child(
            div()
                .text_color(Text::SECONDARY)
                .text_size(px(FontSize::SM_MD))
                .child(instruction.to_string()),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .justify_end()
                .child(tour_button("Done", true)),
        )
}

impl Render for TourOverlayView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let visible = self.state.visible;
        let title = self.state.title.clone();
        let instruction = self.state.instruction.clone();
        let card_kind = self.state.card.clone();

        let scrim = div()
            .id("tour-overlay")
            .track_focus(&self.focus_handle.clone())
            .size_full()
            .absolute()
            .top(px(0.0))
            .left(px(0.0))
            .flex()
            .items_center()
            .justify_center()
            .bg(gpui::Hsla { h: 0.0, s: 0.0, l: 0.0, a: Opacity::STRONG })
            .on_click(cx.listener(|this, _: &ClickEvent, _: &mut Window, cx| {
                this.state.visible = false;
                cx.notify();
            }));

        div()
            .size_full()
            .relative()
            .when(visible, |el| {
                match &card_kind {
                    TourCardKind::Intro => el.child(
                        scrim.child(intro_card(&title, &instruction))
                    ),
                    TourCardKind::Spotlight { step, total } => el.child(
                        scrim.child(callout_card(*step, *total, &title, &instruction))
                    ),
                    TourCardKind::Outro => el.child(
                        scrim.child(outro_card(&title, &instruction))
                    ),
                }
            })
    }
}

//! UpdateOverlay — matches Swift UpdateOverlay.
//!
//! Shown on first launch after an app update ("What's New in vX").
//! Card layout: version badge + title + section list + full-changelog link + Continue button.
//!
//! Scrim + centered card, same visual pattern as WelcomeOverlay.

use crate::theme::{Accent, Background, BorderColors, FontSize, Opacity, Radius, Spacing, Text};
use gpui::{
    div, prelude::*, px, App, ClickEvent, Context, FocusHandle, Focusable, IntoElement,
    ParentElement, Render, SharedString, Styled, Window,
};

/// One "What's New" entry shown in the changelog card.
#[derive(Debug, Clone)]
pub struct UpdateSection {
    pub emoji: &'static str,
    pub title: SharedString,
    pub detail: SharedString,
}

#[derive(Debug, Clone)]
pub struct UpdateOverlayState {
    pub version: SharedString,
    pub sections: Vec<UpdateSection>,
    pub visible: bool,
}

impl Default for UpdateOverlayState {
    fn default() -> Self {
        Self {
            version: "1.0".into(),
            sections: vec![
                UpdateSection {
                    emoji: "✦",
                    title: "New in this version".into(),
                    detail: "See the full changelog for details.".into(),
                },
            ],
            visible: false,
        }
    }
}

pub struct UpdateOverlayView {
    pub state: UpdateOverlayState,
    focus_handle: FocusHandle,
}

impl UpdateOverlayView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            state: UpdateOverlayState::default(),
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn show_for_version(&mut self, version: impl Into<SharedString>) {
        self.state.version = version.into();
        self.state.visible = true;
    }
}

impl Focusable for UpdateOverlayView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

fn section_row(s: &UpdateSection) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .items_start()
        .gap(px(Spacing::SM))
        .child(
            div()
                .w(px(20.0))
                .text_color(Accent::PRIMARY)
                .text_size(px(FontSize::SM))
                .child(s.emoji),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(Spacing::XXS))
                .flex_1()
                .child(
                    div()
                        .text_color(Text::PRIMARY)
                        .text_size(px(FontSize::SM))
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .child(s.title.clone()),
                )
                .child(
                    div()
                        .text_color(Text::SECONDARY)
                        .text_size(px(FontSize::XS))
                        .child(s.detail.clone()),
                ),
        )
}

impl Render for UpdateOverlayView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let visible = self.state.visible;
        let version = self.state.version.clone();
        let sections = self.state.sections.clone();

        div()
            .size_full()
            .relative()
            .when(visible, |el| {
                el.child(
                    div()
                        .id("update-overlay-scrim")
                        .track_focus(&self.focus_handle.clone())
                        .size_full()
                        .absolute()
                        .top(px(0.0))
                        .left(px(0.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(gpui::Hsla { h: 0.0, s: 0.0, l: 0.0, a: Opacity::STRONG })
                        // Card
                        .child(
                            div()
                                .id("update-card")
                                .flex()
                                .flex_col()
                                .gap(px(Spacing::MD_LG))
                                .w(px(520.0))
                                .rounded(px(Radius::LG))
                                .bg(Background::RAISED)
                                .p(px(Spacing::XL))
                                .border_1()
                                .border_color(BorderColors::SUBTLE)
                                // Version badge
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .gap(px(Spacing::XS))
                                        .child(
                                            div()
                                                .px(px(Spacing::SM))
                                                .py(px(Spacing::XXS))
                                                .rounded_full()
                                                .bg(gpui::Hsla {
                                                    h: Accent::PRIMARY.h,
                                                    s: Accent::PRIMARY.s,
                                                    l: Accent::PRIMARY.l,
                                                    a: 0.15,
                                                })
                                                .text_color(Accent::PRIMARY)
                                                .text_size(px(FontSize::XS))
                                                .child(format!("v{version}")),
                                        )
                                        .child(
                                            div()
                                                .text_color(Text::TERTIARY)
                                                .text_size(px(FontSize::XS))
                                                .child("just installed"),
                                        ),
                                )
                                // Title
                                .child(
                                    div()
                                        .text_color(Text::PRIMARY)
                                        .text_size(px(FontSize::XL))
                                        .font_weight(gpui::FontWeight::MEDIUM)
                                        .child(format!("What's New in {version}")),
                                )
                                // Section list
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap(px(Spacing::MD))
                                        .children(sections.iter().map(section_row)),
                                )
                                // Footer: changelog link + Continue button
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .justify_between()
                                        .child(
                                            div()
                                                .id("changelog-link")
                                                .text_color(Accent::PRIMARY)
                                                .text_size(px(FontSize::SM))
                                                .cursor_pointer()
                                                .child("Full changelog →"),
                                        )
                                        .child(
                                            div()
                                                .id("update-continue-btn")
                                                .px(px(Spacing::MD_LG))
                                                .py(px(Spacing::SM))
                                                .rounded_full()
                                                .bg(Accent::PRIMARY)
                                                .cursor_pointer()
                                                .on_click(cx.listener(|this: &mut UpdateOverlayView, _: &ClickEvent, _, cx| {
                                                    this.state.visible = false;
                                                    cx.notify();
                                                }))
                                                .text_color(Background::BASE)
                                                .text_size(px(FontSize::SM))
                                                .font_weight(gpui::FontWeight::MEDIUM)
                                                .child("Continue"),
                                        ),
                                ),
                        ),
                )
            })
    }
}

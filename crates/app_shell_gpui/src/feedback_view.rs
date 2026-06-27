//! Feedback gpui view — renders the Send Feedback window.
//!
//! Requires the `desktop-app` feature (gpui).

use app_contract::feedback_model::FeedbackViewModel;
use crate::theme::{Background, BorderColors, FontSize, Radius, Spacing, Text};
use gpui::{
    div, prelude::*, px, App, Context, FocusHandle, Focusable,
    ParentElement, Render, Styled, Window,
};

/// gpui Feedback view component.
#[derive(Debug, Clone)]
pub struct FeedbackView {
    focus_handle: FocusHandle,
    #[allow(dead_code)]
    model: FeedbackViewModel,
}

impl FeedbackView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let handle = cx.focus_handle();
        Self {
            focus_handle: handle,
            model: FeedbackViewModel::default(),
        }
    }
}

impl Focusable for FeedbackView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for FeedbackView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("fronda-feedback")
            .track_focus(&self.focus_handle.clone())
            .flex()
            .flex_col()
            .size_full()
            .bg(Background::SURFACE)
            .px(px(Spacing::LG_XL))
            .py(px(Spacing::LG_XL))
            .gap(px(Spacing::MD_LG))
            .child(
                div()
                    .text_size(px(FontSize::MD_LG))
                    .text_color(Text::PRIMARY)
                    .child("Send Feedback"),
            )
            .child(
                div()
                    .text_size(px(FontSize::SM))
                    .text_color(Text::TERTIARY)
                    .child("Describe your experience or report an issue."),
            )
            // Textarea area
            .child(
                div()
                    .flex_1()
                    .rounded(px(Radius::SM))
                    .border_1()
                    .border_color(BorderColors::SUBTLE)
                    .bg(Background::RAISED)
                    .p(px(Spacing::SM_MD))
                    .child(
                        div()
                            .text_size(px(FontSize::SM))
                            .text_color(Text::MUTED)
                            .child("Your feedback…"),
                    ),
            )
            // Footer: Send button
            .child(
                div()
                    .flex()
                    .flex_row()
                    .justify_end()
                    .child(
                        div()
                            .px(px(Spacing::MD_LG))
                            .py(px(Spacing::SM))
                            .rounded(px(Radius::SM))
                            .bg(BorderColors::PRIMARY)
                            .cursor_pointer()
                            .child(
                                div()
                                    .text_size(px(FontSize::SM))
                                    .text_color(Text::PRIMARY)
                                    .child("Send"),
                            ),
                    ),
            )
    }
}

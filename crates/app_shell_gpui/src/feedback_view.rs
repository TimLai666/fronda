//! Feedback gpui view — renders the Send Feedback window.
//!
//! Requires the `desktop-app` feature (gpui).

use app_contract::feedback_model::FeedbackViewModel;
use gpui::{
    div, prelude::*, px, App, Context, FocusHandle, Focusable, Hsla, InteractiveElement,
    ParentElement, Render, Styled, Window,
};

/// Colors for the feedback view.
pub struct FeedbackColors;
impl FeedbackColors {
    pub const BACKGROUND: Hsla = Hsla {
        h: 0.0,
        s: 0.0,
        l: 0.07,
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
            .bg(FeedbackColors::BACKGROUND)
            .px(px(16.0))
            .py(px(16.0))
            .child(
                div()
                    .text_sm()
                    .child("Send Feedback")
                    .text_color(FeedbackColors::TEXT_PRIMARY)
                    .mb(px(12.0)),
            )
            .child(
                div()
                    .text_xs()
                    .child("Describe your experience or report an issue.")
                    .text_color(FeedbackColors::TEXT_SECONDARY),
            )
    }
}

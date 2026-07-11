//! Feedback gpui view — matches Swift FeedbackView structure exactly.
//!
//! Two render branches:
//!   • Form (default): description + optional email + may-contact checkbox +
//!     optional screenshot row + context note + error text + Cancel/Send footer.
//!   • Success: checkmark heading + detail text + Done button.

use crate::text_area::{TextArea, TextAreaEvent};
use crate::text_field::{TextField, TextFieldEvent};
use crate::theme::{Accent, Background, BorderColors, FontSize, Radius, Spacing, Text};
use app_contract::feedback_model::FeedbackViewModel;
use gpui::{
    div, prelude::*, px, Animation, AnimationExt as _, App, ClickEvent, Context, Entity,
    FocusHandle, Focusable, InteractiveElement, KeyDownEvent, ParentElement, Render, Styled,
    Window,
};
use std::time::Duration;

/// gpui Feedback view component.
#[derive(Clone)]
pub struct FeedbackView {
    focus_handle: FocusHandle,
    pub model: FeedbackViewModel,
    /// True when the user is signed in (hides the email field).
    pub is_signed_in: bool,
    /// True when a screenshot was captured at open time.
    pub has_screenshot: bool,
    /// Real text field (IME-capable) for the reply email.
    email_field: Entity<TextField>,
    /// Multiline message editor (IME-capable); `model.message` mirrors it.
    message_area: Entity<TextArea>,
}

impl FeedbackView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let email_field = cx.new(|cx| TextField::new(cx, "you@example.com — so we can reply"));
        cx.subscribe(&email_field, |this, field, event, cx| {
            if matches!(event, TextFieldEvent::Edited) {
                this.model.email = field.read(cx).text().to_string();
                cx.notify();
            }
        })
        .detach();
        let message_area = cx.new(|cx| {
            TextArea::new(cx, "Your feedback…")
                .with_min_lines(5)
                .with_max_lines(10)
        });
        cx.subscribe(&message_area, |this, area, event, cx| {
            if matches!(event, TextAreaEvent::Edited) {
                this.model.message = area.read(cx).text().to_string();
                cx.notify();
            }
        })
        .detach();
        Self {
            focus_handle: cx.focus_handle(),
            model: FeedbackViewModel::default(),
            is_signed_in: false,
            has_screenshot: false,
            email_field,
            message_area,
        }
    }

    /// Tab cycles focus between the message area and the email field
    /// (both pass Tab through to the host).
    fn handle_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.keystroke.key.as_str() != "tab" || self.is_signed_in {
            return;
        }
        if self.message_area.focus_handle(cx).is_focused(window) {
            window.focus(&self.email_field.focus_handle(cx), cx);
        } else if self.email_field.focus_handle(cx).is_focused(window) {
            window.focus(&self.message_area.focus_handle(cx), cx);
        } else {
            return;
        }
        cx.stop_propagation();
        cx.notify();
    }

    fn can_submit(&self) -> bool {
        !self.model.is_sending && !self.model.message.trim().is_empty()
    }

    fn has_reply_email(&self) -> bool {
        self.is_signed_in || !self.model.email.trim().is_empty()
    }

    fn render_field_label(label: &str) -> impl IntoElement {
        div()
            .text_size(px(FontSize::SM))
            .text_color(Text::SECONDARY)
            .font_weight(gpui::FontWeight::MEDIUM)
            .child(label.to_string())
    }

    fn render_form(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let is_signed_in = self.is_signed_in;
        let has_screenshot = self.has_screenshot;
        let include_screenshot = self.model.include_screenshot;
        let may_contact = self.model.may_contact;
        let has_reply = self.has_reply_email();
        let can_submit = self.can_submit();
        let is_sending = self.model.is_sending;
        let error_text = self.model.error.clone();

        let mut form = div().flex().flex_col().gap(px(Spacing::LG));

        // Description textarea
        form = form.child(
            div()
                .flex()
                .flex_col()
                .gap(px(Spacing::XS))
                .child(Self::render_field_label("Describe the issue or feedback"))
                .child(
                    div()
                        .id("feedback-message-input")
                        .rounded(px(Radius::SM))
                        .border_1()
                        .border_color(BorderColors::SUBTLE)
                        .bg(Background::SURFACE)
                        .p(px(Spacing::SM_MD))
                        .cursor_text()
                        .on_click(cx.listener(|this, _, window, cx| {
                            window.focus(&this.message_area.focus_handle(cx), cx);
                            cx.notify();
                        }))
                        .child(
                            div()
                                .flex_1()
                                .text_size(px(FontSize::MD))
                                .text_color(Text::PRIMARY)
                                .child(self.message_area.clone()),
                        ),
                ),
        );

        // Email field — shown only when not signed in
        if !is_signed_in {
            form = form.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(Spacing::XS))
                    .child(Self::render_field_label("Email (optional)"))
                    .child(
                        div()
                            .id("feedback-email-input")
                            .rounded(px(Radius::SM))
                            .border_1()
                            .border_color(BorderColors::SUBTLE)
                            .bg(Background::SURFACE)
                            .px(px(Spacing::MD_LG))
                            .py(px(Spacing::SM_MD))
                            .text_size(px(FontSize::MD))
                            .text_color(Text::PRIMARY)
                            .child(self.email_field.clone()),
                    ),
            );
        }

        // May contact checkbox
        form = form.child(
            div()
                .id("feedback-may-contact")
                .flex()
                .flex_row()
                .items_center()
                .gap(px(Spacing::SM))
                .cursor_pointer()
                .on_click(cx.listener(
                    |this: &mut FeedbackView,
                     _: &ClickEvent,
                     _: &mut Window,
                     cx: &mut Context<FeedbackView>| {
                        if this.has_reply_email() {
                            this.model.may_contact = !this.model.may_contact;
                            cx.notify();
                        }
                    },
                ))
                .child(
                    // Checkbox glyph
                    div()
                        .w(px(14.0))
                        .h(px(14.0))
                        .rounded(px(2.0))
                        .border_1()
                        .border_color(if may_contact && has_reply {
                            Accent::PRIMARY
                        } else {
                            BorderColors::PRIMARY
                        })
                        .bg(if may_contact && has_reply {
                            Accent::PRIMARY
                        } else {
                            Background::BASE
                        })
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .text_color(Background::BASE)
                                .text_size(px(8.0))
                                .child(if may_contact && has_reply { "✓" } else { "" }),
                        ),
                )
                .child(
                    div()
                        .text_size(px(FontSize::MD))
                        .text_color(if has_reply {
                            Text::SECONDARY
                        } else {
                            Text::TERTIARY
                        })
                        .child("We may email you for follow-up questions"),
                ),
        );

        // Screenshot row — shown when has_screenshot
        if has_screenshot {
            form = form.child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(Spacing::MD_LG))
                    .child(
                        div()
                            .id("feedback-include-screenshot")
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(Spacing::SM))
                            .cursor_pointer()
                            .on_click(cx.listener(
                                |this: &mut FeedbackView,
                                 _: &ClickEvent,
                                 _: &mut Window,
                                 cx: &mut Context<FeedbackView>| {
                                    this.model.include_screenshot = !this.model.include_screenshot;
                                    cx.notify();
                                },
                            ))
                            .child(
                                div()
                                    .w(px(14.0))
                                    .h(px(14.0))
                                    .rounded(px(2.0))
                                    .border_1()
                                    .border_color(if include_screenshot {
                                        Accent::PRIMARY
                                    } else {
                                        BorderColors::PRIMARY
                                    })
                                    .bg(if include_screenshot {
                                        Accent::PRIMARY
                                    } else {
                                        Background::BASE
                                    })
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(
                                        div()
                                            .text_color(Background::BASE)
                                            .text_size(px(8.0))
                                            .child(if include_screenshot { "✓" } else { "" }),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(FontSize::MD))
                                    .text_color(Text::SECONDARY)
                                    .child("Include screenshot"),
                            ),
                    )
                    .child(div().flex_1())
                    // Screenshot thumbnail placeholder
                    .child(
                        div()
                            .w(px(88.0))
                            .h(px(56.0))
                            .rounded(px(Radius::XS_SM))
                            .border_1()
                            .border_color(BorderColors::SUBTLE)
                            .bg(Background::RAISED)
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(Text::MUTED)
                            .text_size(px(FontSize::XS))
                            .child("preview"),
                    ),
            );
        }

        // Context note: "ⓘ App version X and macOS Y are included."
        form = form.child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(Spacing::XS))
                .child(
                    div()
                        .text_color(Text::TERTIARY)
                        .text_size(px(FontSize::XS))
                        .child("ⓘ"),
                )
                .child(
                    div()
                        .text_color(Text::TERTIARY)
                        .text_size(px(FontSize::XS))
                        .child("App version and OS info are included automatically."),
                ),
        );

        // Error text
        if let Some(err) = error_text {
            form = form.child(
                div()
                    .text_color(gpui::Hsla {
                        h: 0.0,
                        s: 0.85,
                        l: 0.55,
                        a: 1.0,
                    })
                    .text_size(px(FontSize::SM))
                    .child(err),
            );
        }

        // Footer: [Cancel] [Send / Sending…]
        form = form.child(
            div()
                .flex()
                .flex_row()
                .justify_end()
                .gap(px(Spacing::SM_MD))
                // Cancel button
                .child(
                    div()
                        .id("feedback-cancel")
                        .px(px(Spacing::MD_LG))
                        .py(px(Spacing::SM))
                        .rounded_full()
                        .border_1()
                        .border_color(BorderColors::SUBTLE)
                        .cursor_pointer()
                        .text_color(Text::SECONDARY)
                        .text_size(px(FontSize::SM))
                        .child("Cancel"),
                )
                // Send / Sending button
                .child(
                    div()
                        .id("feedback-send")
                        .px(px(Spacing::MD_LG))
                        .py(px(Spacing::SM))
                        .rounded_full()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(Spacing::XS))
                        .bg(if can_submit {
                            Accent::PRIMARY
                        } else {
                            Background::PROMINENT
                        })
                        .cursor_pointer()
                        .on_click(cx.listener(
                            |this: &mut FeedbackView,
                             _: &ClickEvent,
                             _: &mut Window,
                             cx: &mut Context<FeedbackView>| {
                                if this.can_submit() {
                                    this.model.is_sending = true;
                                    cx.notify();
                                }
                            },
                        ))
                        .text_color(if can_submit {
                            Background::BASE
                        } else {
                            Text::MUTED
                        })
                        .text_size(px(FontSize::SM))
                        .when(is_sending, |el| el.child(sending_spinner()))
                        .child(if is_sending { "Sending" } else { "Send" }),
                ),
        );

        form
    }

    fn render_success(&self) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap(px(Spacing::MD))
            // Checkmark + heading
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(Spacing::XS))
                    .child(
                        div()
                            .text_color(Accent::PRIMARY)
                            .text_size(px(FontSize::MD_LG))
                            .child("✓"),
                    )
                    .child(
                        div()
                            .text_color(Text::PRIMARY)
                            .text_size(px(FontSize::MD))
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .child("Thanks for the feedback."),
                    ),
            )
            // Detail text
            .child({
                let detail = if self.is_signed_in || !self.model.email.trim().is_empty() {
                    if self.model.may_contact {
                        "We read every message and may reach out at your email address."
                    } else {
                        "We read every message. We won't email you, as requested."
                    }
                } else {
                    "We read every message. Add an email next time if you'd like a reply."
                };
                div()
                    .text_color(Text::TERTIARY)
                    .text_size(px(FontSize::SM))
                    .child(detail)
            })
            // Done button
            .child(
                div().flex().flex_row().justify_end().child(
                    div()
                        .id("feedback-done")
                        .px(px(Spacing::MD_LG))
                        .py(px(Spacing::SM))
                        .rounded_full()
                        .bg(Accent::PRIMARY)
                        .cursor_pointer()
                        .text_color(Background::BASE)
                        .text_size(px(FontSize::SM))
                        .child("Done"),
                ),
            )
    }
}

fn sending_spinner() -> impl gpui::IntoElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(2.0))
        .children((0u32..3).map(|i| {
            div()
                .w(px(4.0))
                .h(px(4.0))
                .rounded_full()
                .bg(Background::BASE)
                .with_animation(
                    format!("send-dot-{i}"),
                    Animation::new(Duration::from_millis(900)).repeat(),
                    move |el, delta| {
                        let phase = (delta + i as f32 / 3.0) % 1.0;
                        let a: f32 = if phase < 1.0 / 3.0 { 1.0 } else { 0.25 };
                        el.opacity(a)
                    },
                )
        }))
}

impl Focusable for FeedbackView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for FeedbackView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let did_send = self.model.sent;

        div()
            .id("fronda-feedback")
            .track_focus(&self.focus_handle.clone())
            .on_key_down(cx.listener(Self::handle_key_down))
            .flex()
            .flex_col()
            .size_full()
            .bg(Background::SURFACE)
            .px(px(Spacing::XL_XXL))
            .py(px(Spacing::XL_XXL))
            .gap(px(Spacing::LG_XL))
            .child(if did_send {
                self.render_success().into_any_element()
            } else {
                self.render_form(cx).into_any_element()
            })
    }
}

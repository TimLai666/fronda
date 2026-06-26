//! Chat/Agent panel gpui view — renders the chat interface.
//!
//! Requires the `desktop-app` feature (gpui).

use app_contract::chat_model::{ChatMessage, ChatPanelModel, ChatRole, MessageStatus};
use gpui::{
    div, prelude::*, px, App, Context, FocusHandle, Focusable, Hsla, InteractiveElement,
    ParentElement, Render, Styled, Window,
};

/// Colors for the chat view.
pub struct ChatColors;
impl ChatColors {
    pub const BACKGROUND: Hsla = Hsla {
        h: 0.0,
        s: 0.0,
        l: 0.07,
        a: 1.0,
    };
    pub const USER_MSG_BG: Hsla = Hsla {
        h: 210.0 / 360.0,
        s: 0.3,
        l: 0.18,
        a: 1.0,
    };
    pub const ASSISTANT_MSG_BG: Hsla = Hsla {
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
    pub const INPUT_BG: Hsla = Hsla {
        h: 0.0,
        s: 0.0,
        l: 0.1,
        a: 1.0,
    };
    pub const INPUT_BORDER: Hsla = Hsla {
        h: 0.0,
        s: 0.0,
        l: 0.2,
        a: 1.0,
    };
    pub const ATTACHMENT_BG: Hsla = Hsla {
        h: 210.0 / 360.0,
        s: 0.25,
        l: 0.14,
        a: 1.0,
    };
    pub const STOP_BG: Hsla = Hsla {
        h: 0.0,
        s: 0.7,
        l: 0.35,
        a: 1.0,
    };
}

/// Role label for display.
fn role_label(role: &ChatRole) -> &str {
    match role {
        ChatRole::User => "You",
        ChatRole::Assistant => "Assistant",
        ChatRole::System => "System",
    }
}

/// gpui Chat/Agent panel view component.
#[derive(Debug, Clone)]
pub struct ChatView {
    focus_handle: FocusHandle,
    model: ChatPanelModel,
}

impl ChatView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let handle = cx.focus_handle();
        Self {
            focus_handle: handle,
            model: ChatPanelModel::default(),
        }
    }

    fn render_message(msg: &ChatMessage) -> impl IntoElement {
        let bg = match msg.role {
            ChatRole::User => ChatColors::USER_MSG_BG,
            ChatRole::Assistant => ChatColors::ASSISTANT_MSG_BG,
            ChatRole::System => ChatColors::ASSISTANT_MSG_BG,
        };

        let status_icon = match &msg.status {
            MessageStatus::Failed(_) => "⚠",
            MessageStatus::Sending => "⋯",
            _ => "",
        };

        let role = role_label(&msg.role).to_string();
        let text = msg.text.clone();

        div()
            .flex()
            .flex_col()
            .mb(px(8.0))
            .px(px(8.0))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(6.0))
                    .child(
                        div()
                            .text_xs()
                            .child(role)
                            .text_color(ChatColors::TEXT_SECONDARY),
                    )
                    .child(
                        div()
                            .text_xs()
                            .child(status_icon)
                            .text_color(ChatColors::TEXT_SECONDARY),
                    ),
            )
            .child(
                div().px(px(8.0)).py(px(6.0)).rounded(px(6.0)).bg(bg).child(
                    div()
                        .text_sm()
                        .child(text)
                        .text_color(ChatColors::TEXT_PRIMARY),
                ),
            )
    }
}

impl Focusable for ChatView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ChatView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let mut messages_div = div()
            .id("chat-messages")
            .flex()
            .flex_col()
            .flex_1()
            .overflow_y_scroll();

        for msg in &self.model.messages {
            messages_div = messages_div.child(Self::render_message(msg));
        }

        let input_placeholder = if self.model.input.is_empty() {
            "Ask the agent…".to_string()
        } else {
            self.model.input.text.clone()
        };

        let input_bar = div()
            .flex()
            .flex_row()
            .px(px(8.0))
            .py(px(6.0))
            .gap(px(6.0))
            .bg(ChatColors::INPUT_BG)
            .border_1()
            .border_color(ChatColors::INPUT_BORDER)
            .child(
                div().flex_1().px(px(8.0)).py(px(6.0)).child(
                    div()
                        .text_sm()
                        .child(input_placeholder)
                        .text_color(ChatColors::TEXT_SECONDARY),
                ),
            );

        div()
            .id("fronda-chat")
            .track_focus(&self.focus_handle.clone())
            .flex()
            .flex_col()
            .size_full()
            .bg(ChatColors::BACKGROUND)
            .child(messages_div)
            .child(input_bar)
    }
}

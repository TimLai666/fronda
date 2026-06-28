//! Chat history list popover — matches Swift ChatHistoryList.
//!
//! Shows past chat sessions. Width: 280px, max content height 360px.
//! Active session row has Accent::PRIMARY @ 15% bg.
//! Each row: title (XS) + relative time (9pt) on left, trash icon on right (inactive only).

use crate::theme::{Accent, Background, BorderColors, FontSize, Radius, Spacing, Text};
use gpui::{
    div, prelude::*, px, App, Context, FocusHandle, Focusable, IntoElement, ParentElement,
    Render, SharedString, Styled, Window,
};

/// A single chat session in the history list.
#[derive(Debug, Clone)]
pub struct ChatSessionEntry {
    pub id: SharedString,
    pub title: SharedString,
    pub relative_time: SharedString,
    pub is_current: bool,
}

/// State for the chat history popover.
#[derive(Debug, Clone, Default)]
pub struct ChatHistoryState {
    pub sessions: Vec<ChatSessionEntry>,
}

/// Chat history popover view.
pub struct ChatHistoryView {
    pub state: ChatHistoryState,
    focus_handle: FocusHandle,
}

impl ChatHistoryView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            state: ChatHistoryState::default(),
            focus_handle: cx.focus_handle(),
        }
    }
}

impl Focusable for ChatHistoryView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

fn session_row(entry: &ChatSessionEntry) -> impl IntoElement {
    let is_current = entry.is_current;
    let active_bg = gpui::Hsla {
        h: Accent::PRIMARY.h,
        s: Accent::PRIMARY.s,
        l: Accent::PRIMARY.l,
        a: 0.15,
    };

    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(Spacing::SM_MD))
        .w_full()
        .px(px(Spacing::MD))
        .py(px(6.0))
        .cursor_pointer()
        .bg(if is_current { active_bg } else { Background::BASE })
        // Title + time column
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .gap(px(Spacing::XXS))
                .child(
                    div()
                        .text_color(Text::PRIMARY)
                        .text_size(px(FontSize::XS))
                        .when(is_current, |el| el.font_weight(gpui::FontWeight::SEMIBOLD))
                        .child(entry.title.clone()),
                )
                .child(
                    div()
                        .text_color(Text::TERTIARY)
                        .text_size(px(9.0))
                        .child(entry.relative_time.clone()),
                ),
        )
        // Trash icon (only for non-current sessions)
        .when(!is_current, |el| {
            el.child(
                div()
                    .text_color(Text::MUTED)
                    .text_size(px(FontSize::XS))
                    .cursor_pointer()
                    .child("🗑"),
            )
        })
}

impl Render for ChatHistoryView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let is_empty = self.state.sessions.is_empty();
        let sessions: Vec<ChatSessionEntry> = self.state.sessions.clone();

        div()
            .id("chat-history-list")
            .track_focus(&self.focus_handle.clone())
            .flex()
            .flex_col()
            .w(px(280.0))
            .bg(Background::RAISED)
            .rounded(px(Radius::MD))
            .border_1()
            .border_color(BorderColors::PRIMARY)
            .overflow_hidden()
            .child(if is_empty {
                div()
                    .text_color(Text::MUTED)
                    .text_size(px(FontSize::XS))
                    .p(px(Spacing::MD))
                    .child("No conversations yet")
                    .into_any_element()
            } else {
                div()
                    .id("chat-sessions-scroll")
                    .flex()
                    .flex_col()
                    .max_h(px(360.0))
                    .overflow_y_scroll()
                    .children(sessions.iter().map(|s| session_row(s)))
                    .into_any_element()
            })
    }
}

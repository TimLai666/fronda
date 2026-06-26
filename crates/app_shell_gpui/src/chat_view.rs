//! Chat/Agent panel gpui view — renders the chat interface with full interaction.
//!
//! Implements CHAT-001 through CHAT-010.
//! Requires the `desktop-app` feature (gpui).

use app_contract::chat_model::{ChatMessage, ChatPanelModel, ChatRole, MessageStatus};
use app_contract::mention_picker::{MentionCandidate, MentionCategory, MentionPickerState};
use app_contract::session_manager::SessionManager;
use gpui::{
    div, prelude::*, px, App, ClickEvent, Context, FocusHandle, Focusable, Hsla,
    InteractiveElement, KeyDownEvent, ParentElement, Render, Styled, Window,
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
    pub const SEND_ENABLED: Hsla = Hsla {
        h: 210.0 / 360.0,
        s: 0.5,
        l: 0.35,
        a: 1.0,
    };
    pub const SEND_DISABLED: Hsla = Hsla {
        h: 0.0,
        s: 0.0,
        l: 0.18,
        a: 1.0,
    };
    pub const STOP_BG: Hsla = Hsla {
        h: 0.0,
        s: 0.7,
        l: 0.35,
        a: 1.0,
    };
    pub const TAB_BG: Hsla = Hsla {
        h: 0.0,
        s: 0.0,
        l: 0.1,
        a: 1.0,
    };
    pub const TAB_ACTIVE_BG: Hsla = Hsla {
        h: 210.0 / 360.0,
        s: 0.4,
        l: 0.18,
        a: 1.0,
    };
    pub const PICKER_BG: Hsla = Hsla {
        h: 0.0,
        s: 0.0,
        l: 0.12,
        a: 1.0,
    };
    pub const PICKER_HIGHLIGHT: Hsla = Hsla {
        h: 210.0 / 360.0,
        s: 0.4,
        l: 0.22,
        a: 1.0,
    };
    pub const TAB_HIGHLIGHT_BG: Hsla = Hsla {
        h: 210.0 / 360.0,
        s: 0.3,
        l: 0.14,
        a: 1.0,
    };
}

/// Role label for display.
fn role_label(role: &ChatRole) -> &'static str {
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
    session_mgr: SessionManager,
    mention_picker: MentionPickerState,
    shift_held: bool,
}

impl ChatView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let handle = cx.focus_handle();
        // Build initial mention candidates
        let mention_candidates = vec![
            MentionCandidate {
                id: "add_clips".into(),
                label: "Add Clips".into(),
                category: MentionCategory::Tools,
                subtitle: None,
            },
            MentionCandidate {
                id: "split_clip".into(),
                label: "Split Clip".into(),
                category: MentionCategory::Tools,
                subtitle: None,
            },
            MentionCandidate {
                id: "remove_clips".into(),
                label: "Remove Clips".into(),
                category: MentionCategory::Tools,
                subtitle: None,
            },
            MentionCandidate {
                id: "media-current".into(),
                label: "beach.mp4".into(),
                category: MentionCategory::Media,
                subtitle: Some("00:01:23".into()),
            },
            MentionCandidate {
                id: "context-selection".into(),
                label: "Current Selection".into(),
                category: MentionCategory::Context,
                subtitle: None,
            },
            MentionCandidate {
                id: "context-timeline".into(),
                label: "Timeline State".into(),
                category: MentionCategory::Context,
                subtitle: None,
            },
        ];

        Self {
            focus_handle: handle,
            model: ChatPanelModel::default(),
            session_mgr: SessionManager::new(),
            mention_picker: MentionPickerState::new(mention_candidates),
            shift_held: false,
        }
    }

    /// Handle key down for Enter/Shift+Enter and @mention trigger.
    fn handle_key_down(
        &mut self,
        event: &KeyDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event.keystroke.key.as_str() {
            "enter" => {
                if self.model.handle_send_action(self.shift_held).is_some() {
                    self.session_mgr.increment_message_count();
                    if self
                        .session_mgr
                        .active_session()
                        .map(|s| s.message_count == 1)
                        .unwrap_or(false)
                    {
                        let title = truncate_title(
                            self.model
                                .messages
                                .last()
                                .map(|m| m.text.as_str())
                                .unwrap_or(""),
                        );
                        self.session_mgr.set_active_title(title);
                    }
                }
                cx.notify();
            }
            "@" => {
                self.model.toggle_mention_picker();
                if self.model.show_mention_picker {
                    self.mention_picker.open("");
                }
                cx.notify();
            }
            "escape" => {
                if self.model.show_mention_picker {
                    self.model.toggle_mention_picker();
                    self.mention_picker.close();
                    cx.notify();
                }
            }
            _ => {}
        }
    }

    // ── Render helpers ──

    fn render_tab_bar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let count = self.session_mgr.sessions.len();
        let active_idx = self.session_mgr.active_index;

        let mut bar = div()
            .id("chat-tab-bar")
            .flex()
            .flex_row()
            .px(px(4.0))
            .py(px(2.0))
            .gap(px(2.0))
            .bg(ChatColors::BACKGROUND);

        for (i, session) in self.session_mgr.sessions.iter().enumerate() {
            let is_active = i == active_idx;
            let tab_bg = if is_active {
                ChatColors::TAB_ACTIVE_BG
            } else {
                ChatColors::TAB_BG
            };
            let title = session.title.clone();

            let tab = div()
                .id(gpui::SharedString::from(format!("chat-tab-{i}")))
                .flex()
                .flex_row()
                .items_center()
                .px(px(8.0))
                .py(px(4.0))
                .rounded(px(4.0))
                .bg(tab_bg)
                .cursor_pointer()
                .on_click(cx.listener(
                    move |this: &mut ChatView,
                          _event: &ClickEvent,
                          _window: &mut Window,
                          cx: &mut Context<ChatView>| {
                        this.session_mgr.select_tab(i);
                        cx.notify();
                    },
                ))
                .child(
                    div()
                        .text_xs()
                        .child(title)
                        .text_color(ChatColors::TEXT_PRIMARY),
                );

            bar = bar.child(tab);
        }

        // New tab button
        let new_tab_bg = ChatColors::TAB_BG;
        bar = bar.child(
            div()
                .id("chat-new-tab")
                .flex()
                .items_center()
                .justify_center()
                .px(px(6.0))
                .py(px(4.0))
                .rounded(px(4.0))
                .bg(new_tab_bg)
                .cursor_pointer()
                .on_click(cx.listener(
                    |this: &mut ChatView,
                     _event: &ClickEvent,
                     _window: &mut Window,
                     cx: &mut Context<ChatView>| {
                        this.session_mgr.new_tab();
                        cx.notify();
                    },
                ))
                .child(
                    div()
                        .text_xs()
                        .child("+")
                        .text_color(ChatColors::TEXT_SECONDARY),
                ),
        );

        if count > 1 {
            let close_tab_idx = active_idx;
            bar = bar.child(
                div().flex_1().flex().justify_end().child(
                    div()
                        .id("chat-close-tab")
                        .px(px(6.0))
                        .py(px(2.0))
                        .cursor_pointer()
                        .on_click(cx.listener(
                            move |this: &mut ChatView,
                                  _event: &ClickEvent,
                                  _window: &mut Window,
                                  cx: &mut Context<ChatView>| {
                                this.session_mgr.close_tab(close_tab_idx);
                                cx.notify();
                            },
                        ))
                        .child(
                            div()
                                .text_xs()
                                .child("✕")
                                .text_color(ChatColors::TEXT_SECONDARY),
                        ),
                ),
            );
        }

        bar
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
            .id(gpui::SharedString::from(format!(
                "chat-msg-{}",
                role_label(&msg.role)
            )))
            .flex()
            .flex_col()
            .mb(px(4.0))
            .px(px(8.0))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(6.0))
                    .mb(px(2.0))
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
                div()
                    .px(px(10.0))
                    .py(px(8.0))
                    .rounded(px(6.0))
                    .bg(bg)
                    .child(
                        div()
                            .text_sm()
                            .child(text)
                            .text_color(ChatColors::TEXT_PRIMARY),
                    ),
            )
    }

    fn render_mention_picker(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.mention_picker.visible {
            return div().id("mention-picker-hidden").size_0();
        }

        let mut category_bar = div()
            .id("mention-category-bar")
            .flex()
            .flex_row()
            .gap(px(2.0))
            .px(px(8.0))
            .py(px(4.0));

        for cat in MentionCategory::ALL {
            let is_active = *cat == self.mention_picker.active_category;
            let bg = if is_active {
                ChatColors::TAB_HIGHLIGHT_BG
            } else {
                ChatColors::PICKER_BG
            };
            let cat_val = *cat;

            category_bar = category_bar.child(
                div()
                    .id(gpui::SharedString::from(format!(
                        "mention-cat-{}",
                        cat.label()
                    )))
                    .px(px(8.0))
                    .py(px(3.0))
                    .rounded(px(3.0))
                    .bg(bg)
                    .cursor_pointer()
                    .on_click(cx.listener(
                        move |this: &mut ChatView,
                              _event: &ClickEvent,
                              _window: &mut Window,
                              cx: &mut Context<ChatView>| {
                            this.mention_picker.active_category = cat_val;
                            this.mention_picker.refresh_filter();
                            cx.notify();
                        },
                    ))
                    .child(
                        div()
                            .text_xs()
                            .child(cat.label().to_string())
                            .text_color(ChatColors::TEXT_PRIMARY),
                    ),
            );
        }

        let mut candidate_list = div()
            .id("mention-candidate-list")
            .flex()
            .flex_col()
            .px(px(8.0))
            .py(px(4.0))
            .gap(px(2.0));

        let candidates_snapshot: Vec<(String, String, String)> = self
            .mention_picker
            .candidates
            .iter()
            .map(|c| {
                (
                    c.id.clone(),
                    c.label.clone(),
                    c.subtitle.clone().unwrap_or_default(),
                )
            })
            .collect();

        for (i, (_id, label, subtitle)) in candidates_snapshot.iter().enumerate() {
            let is_highlighted = i == self.mention_picker.highlighted_index;
            let bg = if is_highlighted {
                ChatColors::PICKER_HIGHLIGHT
            } else {
                ChatColors::PICKER_BG
            };
            let display_label = label.clone();
            let mention_label_capture = label.clone();
            let mention_subtitle = subtitle.clone();

            candidate_list = candidate_list.child(
                div()
                    .id(gpui::SharedString::from(format!("mention-{i}")))
                    .flex()
                    .flex_row()
                    .items_center()
                    .px(px(8.0))
                    .py(px(4.0))
                    .rounded(px(3.0))
                    .bg(bg)
                    .cursor_pointer()
                    .on_click(cx.listener(
                        move |this: &mut ChatView,
                              _event: &ClickEvent,
                              _window: &mut Window,
                              cx: &mut Context<ChatView>| {
                            let mention_text = format!("@{} ", mention_label_capture);
                            this.model.input.text.push_str(&mention_text);
                            this.model.input.cursor_position = this.model.input.text.len();
                            this.model.show_mention_picker = false;
                            this.mention_picker.close();
                            cx.notify();
                        },
                    ))
                    .child(
                        div()
                            .flex_1()
                            .text_sm()
                            .child(display_label)
                            .text_color(ChatColors::TEXT_PRIMARY),
                    )
                    .child(
                        div()
                            .text_xs()
                            .child(mention_subtitle)
                            .text_color(ChatColors::TEXT_SECONDARY),
                    ),
            );
        }

        div()
            .id("mention-picker")
            .flex()
            .flex_col()
            .bg(ChatColors::PICKER_BG)
            .border_1()
            .border_color(ChatColors::INPUT_BORDER)
            .rounded(px(6.0))
            .mb(px(4.0))
            .mx(px(8.0))
            .child(category_bar)
            .child(div().h(px(1.0)).bg(ChatColors::INPUT_BORDER).w_full())
            .child(candidate_list)
    }
}

impl Focusable for ChatView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ChatView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // ── Messages area ──
        let mut messages_div = div()
            .id("chat-messages")
            .flex()
            .flex_col()
            .flex_1()
            .overflow_y_scroll()
            .px(px(4.0))
            .py(px(8.0));

        for msg in &self.model.messages {
            messages_div = messages_div.child(Self::render_message(msg));
        }

        // ── Mention picker ──
        let picker = self.render_mention_picker(cx);

        // ── Input bar ──
        let can_send = self.model.can_send();
        let send_bg = if can_send {
            ChatColors::SEND_ENABLED
        } else {
            ChatColors::SEND_DISABLED
        };
        let is_running = self.model.is_agent_running;
        let input_placeholder = if self.model.input.is_empty() {
            "Ask the agent…".to_string()
        } else {
            self.model.input.text.clone()
        };

        let mut input_row = div()
            .id("chat-input-row")
            .flex()
            .flex_row()
            .px(px(8.0))
            .py(px(6.0))
            .gap(px(6.0))
            .items_center()
            .bg(ChatColors::INPUT_BG);

        // Text input area
        input_row = input_row.child(
            div()
                .flex_1()
                .px(px(10.0))
                .py(px(8.0))
                .rounded(px(6.0))
                .bg(ChatColors::BACKGROUND)
                .border_1()
                .border_color(ChatColors::INPUT_BORDER)
                .child(
                    div()
                        .text_sm()
                        .child(input_placeholder)
                        .text_color(ChatColors::TEXT_PRIMARY),
                ),
        );

        // Send or Stop button
        if is_running {
            input_row = input_row.child(
                div()
                    .id("chat-stop-btn")
                    .px(px(12.0))
                    .py(px(8.0))
                    .rounded(px(6.0))
                    .bg(ChatColors::STOP_BG)
                    .cursor_pointer()
                    .on_click(cx.listener(
                        |this: &mut ChatView,
                         _event: &ClickEvent,
                         _window: &mut Window,
                         cx: &mut Context<ChatView>| {
                            this.model.stop_generation();
                            cx.notify();
                        },
                    ))
                    .child(
                        div()
                            .text_sm()
                            .child("Stop")
                            .text_color(ChatColors::TEXT_PRIMARY),
                    ),
            );
        } else {
            let send_opacity = if can_send { 1.0 } else { 0.4 };
            input_row = input_row.child(
                div()
                    .id("chat-send-btn")
                    .px(px(12.0))
                    .py(px(8.0))
                    .rounded(px(6.0))
                    .bg(send_bg)
                    .cursor_pointer()
                    .on_click(cx.listener(
                        |this: &mut ChatView,
                         _event: &ClickEvent,
                         _window: &mut Window,
                         cx: &mut Context<ChatView>| {
                            if this.model.handle_send_action(false).is_some() {
                                this.session_mgr.increment_message_count();
                                if this
                                    .session_mgr
                                    .active_session()
                                    .map(|s| s.message_count == 1)
                                    .unwrap_or(false)
                                {
                                    let title = truncate_title(
                                        this.model
                                            .messages
                                            .last()
                                            .map(|m| m.text.as_str())
                                            .unwrap_or(""),
                                    );
                                    this.session_mgr.set_active_title(title);
                                }
                            }
                            cx.notify();
                        },
                    ))
                    .child(
                        div()
                            .text_sm()
                            .child("Send")
                            .text_color(ChatColors::TEXT_PRIMARY)
                            .opacity(send_opacity),
                    ),
            );
        }

        // ── Full layout ──
        div()
            .id("fronda-chat")
            .track_focus(&self.focus_handle.clone())
            .on_key_down(cx.listener(Self::handle_key_down))
            .flex()
            .flex_col()
            .size_full()
            .bg(ChatColors::BACKGROUND)
            .child(self.render_tab_bar(cx))
            .child(div().h(px(1.0)).bg(ChatColors::INPUT_BORDER).w_full())
            .child(messages_div)
            .child(picker)
            .child(input_row)
    }
}

fn truncate_title(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.len() <= 40 {
        trimmed.to_string()
    } else {
        format!("{}…", &trimmed[..39])
    }
}

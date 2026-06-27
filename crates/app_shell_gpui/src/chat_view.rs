//! Chat/Agent panel gpui view — renders the chat interface with full interaction.
//!
//! Implements CHAT-001 through CHAT-010.
//! Requires the `desktop-app` feature (gpui).

use app_contract::chat_model::{ChatMessage, ChatPanelModel, ChatRole, MessageStatus};
use app_contract::mention_picker::{MentionCandidate, MentionCategory, MentionPickerState};
use app_contract::session_manager::SessionManager;
use crate::theme::{Accent, Background, BorderColors, FontSize, Layout, Radius, Spacing, Text};
use gpui::{
    div, prelude::*, px, App, ClickEvent, Context, FocusHandle, Focusable, Hsla,
    InteractiveElement, KeyDownEvent, ParentElement, Render, Styled, Window,
};

/// Role label for display.
fn role_label(role: &ChatRole) -> &'static str {
    match role {
        ChatRole::User => "You",
        ChatRole::Assistant => "Assistant",
        ChatRole::System => "System",
    }
}

/// Starter prompt entries shown in empty state (Swift: 7 preset action buttons).
const STARTER_PROMPTS: &[(&str, &str)] = &[
    ("✦", "Generate an AI video"),
    ("✦", "Generate B-roll"),
    ("◧", "Create a letterbox opening"),
    ("Cc", "Add captions to my timeline"),
    ("♪", "Create a voiceover"),
    ("♫", "Generate music and sync to my timeline"),
    ("⊞", "Organize my media into structured folders"),
];

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
            .items_end()
            .px(px(Spacing::MD_LG))
            .pt(px(Spacing::XS))
            .gap(px(Spacing::LG))
            .bg(Background::SURFACE)
            .border_b_1()
            .border_color(BorderColors::SUBTLE);

        for (i, session) in self.session_mgr.sessions.iter().enumerate() {
            let is_active = i == active_idx;
            let title = session.title.clone();

            let tab = div()
                .id(gpui::SharedString::from(format!("chat-tab-{i}")))
                .flex()
                .flex_row()
                .items_center()
                .pb(px(Spacing::XS))
                .gap(px(Spacing::XS))
                .cursor_pointer()
                // Bottom underline when active (no fill, just underline — matches Swift)
                .border_b(px(if is_active { 1.5 } else { 0.0 }))
                .border_color(Text::PRIMARY)
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
                        .text_color(if is_active { Text::PRIMARY } else { Text::MUTED })
                        .text_size(px(FontSize::SM))
                        .child(title),
                );

            bar = bar.child(tab);
        }

        // New tab + button
        bar = bar.child(
            div()
                .id("chat-new-tab")
                .flex()
                .items_center()
                .justify_center()
                .pb(px(Spacing::XS))
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
                        .text_size(px(FontSize::MD_LG))
                        .text_color(Text::MUTED)
                        .child("+"),
                ),
        );

        if count > 1 {
            let close_tab_idx = active_idx;
            bar = bar.child(
                div().flex_1().flex().justify_end().child(
                    div()
                        .id("chat-close-tab")
                        .pb(px(Spacing::XS))
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
                                .text_size(px(FontSize::XS))
                                .text_color(Text::MUTED)
                                .child("✕"),
                        ),
                ),
            );
        }

        bar
    }

    /// Message layout matching Swift AgentMessageView:
    ///   - User:      right-aligned bubble, white@Opacity.faint (0.08), Radius.lg
    ///   - Assistant: left-aligned text, no fill
    fn render_message(msg: &ChatMessage) -> impl IntoElement {
        let status_icon = match &msg.status {
            MessageStatus::Failed(_) => " ⚠",
            MessageStatus::Sending => " ⋯",
            _ => "",
        };

        let text = msg.text.clone() + status_icon;
        let is_user = matches!(msg.role, ChatRole::User);

        div()
            .id(gpui::SharedString::from(format!(
                "chat-msg-{}",
                role_label(&msg.role)
            )))
            .flex()
            .flex_row()
            .w_full()
            .px(px(Spacing::LG_XL))
            .mb(px(Spacing::XL))
            // Push user messages to the right (Swift: HStack { Spacer(minLength:48) ... })
            .when(is_user, |el| {
                el.child(div().flex_1().min_w(px(48.0)))
            })
            .child(
                div()
                    .text_color(Text::PRIMARY)
                    .text_size(px(FontSize::SM_MD))
                    .when(is_user, |el| {
                        el.px(px(Spacing::LG))
                            .py(px(Spacing::SM_MD))
                            .rounded(px(Radius::LG))
                            .bg(Hsla {
                                h: 0.0,
                                s: 0.0,
                                l: 1.0,
                                a: 0.08, // Opacity::FAINT
                            })
                    })
                    .child(text),
            )
    }

    /// Streaming "thinking dots" indicator (Swift: ThinkingDots — 3 circles, text.tertiary).
    fn render_thinking_indicator() -> impl IntoElement {
        div()
            .id("chat-thinking")
            .flex()
            .flex_row()
            .items_center()
            .gap(px(5.0))
            .px(px(Spacing::LG_XL))
            .pb(px(Spacing::MD))
            .children((0..3).map(|_| {
                div()
                    .w(px(5.0))
                    .h(px(5.0))
                    .rounded_full()
                    .bg(Text::TERTIARY)
            }))
    }

    /// Starter prompts shown when no messages exist.
    fn render_starter_prompts(&self) -> impl IntoElement {
        let mut col = div()
            .id("starter-prompts")
            .flex()
            .flex_col()
            .gap(px(Spacing::XS))
            .px(px(Spacing::LG_XL))
            .py(px(Spacing::MD));

        for (icon, label) in STARTER_PROMPTS {
            col = col.child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .w_full()
                    .h(px(36.0))
                    .px(px(Spacing::MD_LG))
                    .gap(px(Spacing::MD))
                    .rounded(px(Radius::SM))
                    .bg(Background::RAISED)
                    .border_1()
                    .border_color(BorderColors::SUBTLE)
                    .cursor_pointer()
                    .child(
                        div()
                            .text_color(Text::SECONDARY)
                            .text_size(px(FontSize::SM_MD))
                            .child(icon.to_string()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_color(Text::SECONDARY)
                            .text_size(px(FontSize::SM))
                            .child(label.to_string()),
                    ),
            );
        }
        col
    }

    fn render_mention_picker(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.mention_picker.visible {
            return div().id("mention-picker-hidden").size_0();
        }

        let mut category_bar = div()
            .id("mention-category-bar")
            .flex()
            .flex_row()
            .gap(px(Spacing::XXS))
            .px(px(Spacing::SM_MD))
            .py(px(Spacing::XS));

        for cat in MentionCategory::ALL {
            let is_active = *cat == self.mention_picker.active_category;
            let cat_val = *cat;

            category_bar = category_bar.child(
                div()
                    .id(gpui::SharedString::from(format!(
                        "mention-cat-{}",
                        cat.label()
                    )))
                    .px(px(Spacing::SM_MD))
                    .py(px(Spacing::XXS))
                    .rounded(px(Radius::XS_SM))
                    .bg(if is_active {
                        BorderColors::PRIMARY
                    } else {
                        Background::RAISED
                    })
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
                            .text_size(px(FontSize::XS))
                            .text_color(if is_active { Text::PRIMARY } else { Text::SECONDARY })
                            .child(cat.label().to_string()),
                    ),
            );
        }

        let mut candidate_list = div()
            .id("mention-candidate-list")
            .flex()
            .flex_col()
            .px(px(Spacing::SM))
            .pb(px(Spacing::SM))
            .gap(px(Spacing::XXS));

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
            let display_label = label.clone();
            let mention_label_capture = label.clone();
            let mention_subtitle = subtitle.clone();
            let highlight_bg = Hsla {
                h: 0.0,
                s: 0.0,
                l: 1.0,
                a: 0.06,
            };

            candidate_list = candidate_list.child(
                div()
                    .id(gpui::SharedString::from(format!("mention-{i}")))
                    .flex()
                    .flex_row()
                    .items_center()
                    .px(px(Spacing::SM_MD))
                    .py(px(Spacing::XS))
                    .rounded(px(Radius::XS_SM))
                    .bg(if is_highlighted {
                        highlight_bg
                    } else {
                        Background::RAISED
                    })
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
                            .text_size(px(FontSize::SM))
                            .text_color(Text::PRIMARY)
                            .child(display_label),
                    )
                    .child(
                        div()
                            .text_size(px(FontSize::XS))
                            .text_color(Text::MUTED)
                            .child(mention_subtitle),
                    ),
            );
        }

        div()
            .id("mention-picker")
            .flex()
            .flex_col()
            .bg(Background::RAISED)
            .border_1()
            .border_color(BorderColors::PRIMARY)
            .rounded(px(Radius::SM))
            .mb(px(Spacing::XS))
            .mx(px(Spacing::SM_MD))
            .child(category_bar)
            .child(div().h(px(1.0)).bg(BorderColors::SUBTLE).w_full())
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
        let is_empty = self.model.messages.is_empty();

        // ── Messages area ──
        let mut messages_div = div()
            .id("chat-messages")
            .flex()
            .flex_col()
            .flex_1()
            .overflow_y_scroll()
            .py(px(Spacing::MD));

        if is_empty {
            messages_div = messages_div.child(self.render_starter_prompts());
        } else {
            for msg in &self.model.messages {
                messages_div = messages_div.child(Self::render_message(msg));
            }
        }

        // Thinking dots shown while streaming (Swift: ThinkingDots when isStreaming)
        if self.model.is_agent_running {
            messages_div = messages_div.child(Self::render_thinking_indicator());
        }

        // ── Mention picker ──
        let picker = self.render_mention_picker(cx);

        // ── Input box ──
        let can_send = self.model.can_send();
        let is_running = self.model.is_agent_running;
        let input_text = if self.model.input.is_empty() {
            "Ask, or type @ to reference media".to_string()
        } else {
            self.model.input.text.clone()
        };
        let is_placeholder = self.model.input.is_empty();

        let send_btn = if is_running {
            div()
                .id("chat-stop-btn")
                .w(px(28.0))
                .h(px(28.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded_full()
                .bg(BorderColors::PRIMARY)
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
                        .text_size(px(FontSize::SM))
                        .text_color(Text::PRIMARY)
                        .child("◼"),
                )
        } else {
            div()
                .id("chat-send-btn")
                .w(px(28.0))
                .h(px(28.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded_full()
                .bg(if can_send {
                    Accent::PRIMARY
                } else {
                    Background::PROMINENT
                })
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
                        .text_size(px(FontSize::SM))
                        .text_color(if can_send { Background::BASE } else { Text::MUTED })
                        .child("▲"),
                )
        };

        let input_footer = div()
            .id("chat-input-footer")
            .flex()
            .flex_col()
            .px(px(Spacing::MD_LG))
            .pb(px(Spacing::MD_LG))
            .pt(px(Spacing::XS))
            .bg(Background::SURFACE)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .rounded(px(Radius::XL))
                    .border_1()
                    .border_color(BorderColors::SUBTLE)
                    .bg(Background::SURFACE)
                    // Text input area
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .min_h(px(32.0))
                            .px(px(Spacing::MD_LG))
                            .pt(px(Spacing::SM_MD))
                            .pb(px(Spacing::XS))
                            .child(
                                div()
                                    .flex_1()
                                    .text_size(px(FontSize::SM_MD))
                                    .text_color(if is_placeholder {
                                        Text::MUTED
                                    } else {
                                        Text::PRIMARY
                                    })
                                    .child(input_text),
                            ),
                    )
                    // Bottom bar: model info + send button
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .px(px(Spacing::MD_LG))
                            .pb(px(Spacing::SM_MD))
                            .pt(px(Spacing::XXS))
                            .child(
                                div()
                                    .flex_1()
                                    .text_color(Text::MUTED)
                                    .text_size(px(FontSize::XS))
                                    .child("claude-sonnet"),
                            )
                            .child(send_btn),
                    ),
            );

        // ── Full layout ──
        div()
            .id("fronda-chat")
            .track_focus(&self.focus_handle.clone())
            .on_key_down(cx.listener(Self::handle_key_down))
            .flex()
            .flex_col()
            .size_full()
            .bg(Background::SURFACE)
            .child(self.render_tab_bar(cx))
            .child(messages_div)
            .child(picker)
            .child(input_footer)
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

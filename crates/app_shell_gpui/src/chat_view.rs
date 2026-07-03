//! Chat/Agent panel gpui view — renders the chat interface with full interaction.
//!
//! Implements CHAT-001 through CHAT-010.
//! Requires the `desktop-app` feature (gpui).

use crate::theme::{Accent, Background, BorderColors, FontSize, Radius, Spacing, Text};
use app_contract::chat_model::{
    ChatMessage, ChatPanelModel, ChatRole, MessageStatus, ToolCallStatus,
};
use app_contract::mention_picker::{MentionCandidate, MentionCategory, MentionPickerState};
use app_contract::session_manager::SessionManager;
use gpui::{
    div, prelude::*, px, Animation, AnimationExt as _, App, ClickEvent, Context, FocusHandle,
    Focusable, Hsla, InteractiveElement, KeyDownEvent, ParentElement, Render, Styled, Window,
};
use std::collections::HashSet;
use std::time::Duration;

const AVAILABLE_MODELS: &[&str] = &["claude-opus-4-8", "claude-sonnet-5", "claude-haiku-4-5"];

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
    /// Index into AVAILABLE_MODELS (Swift: editor.agentService.selectedModel).
    selected_model_idx: usize,
    /// Whether the model picker dropdown is visible.
    model_picker_open: bool,
    /// Whether the chat history popover is visible (Swift: showHistory).
    history_open: bool,
    /// Set of tool-row keys that are expanded: "{msg_idx}-{tool_idx}".
    expanded_tool_rows: HashSet<String>,
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
            selected_model_idx: 1, // claude-sonnet-5 as default
            model_picker_open: false,
            history_open: false,
            expanded_tool_rows: HashSet::new(),
        }
    }

    /// Run an agent turn for `user_text` against the Anthropic API, then append
    /// the reply. The tool loop runs on the background executor; the shared
    /// project executor is locked only per tool call, never across the HTTP
    /// round-trips, so the UI never blocks on the network. Requires
    /// `ANTHROPIC_API_KEY` in the environment.
    fn spawn_agent_turn(&mut self, user_text: String, cx: &mut Context<Self>) {
        let api_key = std::env::var("ANTHROPIC_API_KEY").unwrap_or_default();
        if api_key.trim().is_empty() {
            self.model
                .fail_agent_turn("Set ANTHROPIC_API_KEY to use the agent.".into());
            cx.notify();
            return;
        }
        let model_id = AVAILABLE_MODELS
            .get(self.selected_model_idx)
            .copied()
            .unwrap_or(AVAILABLE_MODELS[0])
            .to_string();
        let executor = crate::editor_state_hub::EditorStateHub::global().executor();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let mut transport = crate::anthropic_transport::AnthropicTransport::new(
                        crate::anthropic_transport::AnthropicConfig::new(api_key),
                    )?;
                    let tools = agent_contract::all_tools();
                    // Include any installed skills in the system prompt so the
                    // agent knows they exist (read via the read_skill tool).
                    let system = {
                        let guard = executor.lock().unwrap();
                        agent_contract::system_instruction_with_skills(guard.skills())
                    };
                    agent_contract::run_agent_turn(
                        &mut transport,
                        |name, args| executor.lock().unwrap().execute(name, args),
                        &model_id,
                        8192,
                        &system,
                        &tools,
                        &user_text,
                        16,
                    )
                })
                .await;
            let _ = this.update(cx, |view, cx| {
                match result {
                    Ok(outcome) => {
                        let tool_calls =
                            crate::agent_bridge::tool_calls_from_records(&outcome.tool_calls);
                        view.model.complete_agent_turn(outcome.final_text, tool_calls)
                    }
                    Err(err) => view.model.fail_agent_turn(err),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn handle_key_down(
        &mut self,
        event: &KeyDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event.keystroke.key.as_str() {
            "enter" => {
                if let Some(agent_text) = self.model.handle_send_action(self.shift_held) {
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
                    self.spawn_agent_turn(agent_text, cx);
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

    /// Tab bar — matches Swift AgentPanelView.floatingTabBar.
    /// Layout: [session tabs w/ close × per tab] [+] [spacer] [⏱ history]
    fn render_tab_bar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let active_idx = self.session_mgr.active_index;
        let history_open = self.history_open;
        let sessions: Vec<(usize, String, bool)> = self
            .session_mgr
            .sessions
            .iter()
            .enumerate()
            .map(|(i, s)| (i, s.title.clone(), i == active_idx))
            .collect();
        let multi = sessions.len() > 1;

        let mut bar = div()
            .id("chat-tab-bar")
            .flex()
            .flex_row()
            .items_center()
            .px(px(Spacing::SM_MD))
            .h(px(crate::theme::Layout::PANEL_HEADER_HEIGHT))
            .gap(px(Spacing::XXS))
            .bg(Background::SURFACE)
            .border_b_1()
            .border_color(BorderColors::SUBTLE);

        // Session tabs — each with optional × close button
        for (i, title, is_active) in sessions {
            let mut tab = div()
                .id(gpui::SharedString::from(format!("chat-tab-{i}")))
                .flex()
                .flex_row()
                .items_center()
                .h_full()
                .px(px(Spacing::SM))
                .gap(px(Spacing::XS))
                .cursor_pointer()
                .border_b(px(if is_active { 1.5 } else { 0.0 }))
                .border_color(Text::PRIMARY)
                .on_click(cx.listener(
                    move |this: &mut ChatView,
                          _: &ClickEvent,
                          _: &mut Window,
                          cx: &mut Context<ChatView>| {
                        this.session_mgr.select_tab(i);
                        cx.notify();
                    },
                ))
                .child(
                    div()
                        .text_color(if is_active {
                            Text::PRIMARY
                        } else {
                            Text::MUTED
                        })
                        .text_size(px(FontSize::SM))
                        .font_weight(if is_active {
                            gpui::FontWeight::MEDIUM
                        } else {
                            gpui::FontWeight::NORMAL
                        })
                        .child(title),
                );

            if multi {
                tab = tab.child(
                    div()
                        .id(gpui::SharedString::from(format!("chat-tab-close-{i}")))
                        .w(px(12.0))
                        .h(px(12.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(Text::MUTED)
                        .text_size(px(FontSize::XXS))
                        .cursor_pointer()
                        .on_click(cx.listener(
                            move |this: &mut ChatView,
                                  _: &ClickEvent,
                                  _: &mut Window,
                                  cx: &mut Context<ChatView>| {
                                this.session_mgr.close_tab(i);
                                cx.notify();
                            },
                        ))
                        .child("×"),
                );
            }

            bar = bar.child(tab);
        }

        // + new tab button
        bar = bar.child(
            div()
                .id("chat-new-tab")
                .w(px(20.0))
                .h(px(20.0))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .rounded(px(Radius::XS))
                .text_color(Text::TERTIARY)
                .text_size(px(FontSize::MD))
                .on_click(cx.listener(
                    |this: &mut ChatView,
                     _: &ClickEvent,
                     _: &mut Window,
                     cx: &mut Context<ChatView>| {
                        this.session_mgr.new_tab();
                        cx.notify();
                    },
                ))
                .child("+"),
        );

        // Spacer
        bar = bar.child(div().flex_1());

        // History button (Swift: historyButton, clock.arrow.circlepath)
        bar = bar.child(
            div()
                .id("chat-history-btn")
                .w(px(20.0))
                .h(px(20.0))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .rounded(px(Radius::XS))
                .bg(if history_open {
                    Hsla {
                        h: 0.0,
                        s: 0.0,
                        l: 1.0,
                        a: 0.08,
                    }
                } else {
                    Hsla {
                        h: 0.0,
                        s: 0.0,
                        l: 0.0,
                        a: 0.0,
                    }
                })
                .text_color(if history_open {
                    Text::PRIMARY
                } else {
                    Text::TERTIARY
                })
                .text_size(px(FontSize::XS))
                .on_click(cx.listener(
                    |this: &mut ChatView,
                     _: &ClickEvent,
                     _: &mut Window,
                     cx: &mut Context<ChatView>| {
                        this.history_open = !this.history_open;
                        cx.notify();
                    },
                ))
                .child("⏱"),
        );

        bar
    }

    /// Chat history popover — rendered as an absolute overlay when history_open = true.
    fn render_history_popover(&self) -> impl IntoElement {
        let session_entries: Vec<(String, String, bool)> = self
            .session_mgr
            .sessions
            .iter()
            .enumerate()
            .map(|(i, s)| {
                (
                    s.title.clone(),
                    "now".to_string(),
                    i == self.session_mgr.active_index,
                )
            })
            .collect();

        let mut list = div()
            .id("chat-history-popover")
            .absolute()
            .top(px(crate::theme::Layout::PANEL_HEADER_HEIGHT + 2.0))
            .right(px(Spacing::SM_MD))
            .w(px(260.0))
            .bg(Background::RAISED)
            .border_1()
            .border_color(BorderColors::PRIMARY)
            .rounded(px(Radius::MD))
            .overflow_hidden()
            .flex()
            .flex_col()
            .max_h(px(300.0))
            .overflow_y_scroll();

        if session_entries.is_empty() {
            list = list.child(
                div()
                    .p(px(Spacing::MD))
                    .text_color(Text::MUTED)
                    .text_size(px(FontSize::XS))
                    .child("No conversations yet"),
            );
        } else {
            for (title, time, is_current) in session_entries {
                let active_bg = Hsla {
                    h: Accent::PRIMARY.h,
                    s: Accent::PRIMARY.s,
                    l: Accent::PRIMARY.l,
                    a: 0.15,
                };
                list = list.child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(Spacing::SM_MD))
                        .w_full()
                        .px(px(Spacing::MD))
                        .py(px(6.0))
                        .cursor_pointer()
                        .bg(if is_current {
                            active_bg
                        } else {
                            Background::BASE
                        })
                        .child(
                            div()
                                .flex_col()
                                .flex()
                                .flex_1()
                                .gap(px(Spacing::XXS))
                                .child(
                                    div()
                                        .text_color(Text::PRIMARY)
                                        .text_size(px(FontSize::XS))
                                        .child(title),
                                )
                                .child(
                                    div()
                                        .text_color(Text::TERTIARY)
                                        .text_size(px(9.0))
                                        .child(time),
                                ),
                        )
                        .when(!is_current, |el| {
                            el.child(
                                div()
                                    .text_color(Text::MUTED)
                                    .text_size(px(FontSize::XS))
                                    .child("🗑"),
                            )
                        }),
                );
            }
        }

        list
    }

    /// Message layout matching Swift AgentMessageView:
    ///   - User:      right-aligned bubble, white@Opacity.faint (0.08), Radius.lg
    ///   - Assistant: left-aligned text + ToolRunRow items + copy button
    fn render_message(
        &self,
        idx: usize,
        msg: &ChatMessage,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_sending = matches!(&msg.status, MessageStatus::Sending);
        let failed_suffix = match &msg.status {
            MessageStatus::Failed(_) => " ⚠",
            _ => "",
        };
        let text = msg.text.clone() + failed_suffix;
        let is_user = matches!(msg.role, ChatRole::User);
        let tool_calls = msg.tool_calls.clone();

        let mut body = div()
            .flex()
            .flex_col()
            .gap(px(Spacing::SM))
            .text_color(Text::PRIMARY)
            .text_size(px(FontSize::SM_MD));

        // Text block — with inline markdown rendering (bold, code, headers, line breaks)
        if !msg.text.is_empty() {
            let text_content = render_md_text(&text, FontSize::SM_MD).into_any_element();
            body = body.child(
                div()
                    .when(is_user, |el| {
                        el.px(px(Spacing::LG))
                            .py(px(Spacing::SM_MD))
                            .rounded(px(Radius::LG))
                            .bg(Hsla {
                                h: 0.0,
                                s: 0.0,
                                l: 1.0,
                                a: 0.08,
                            })
                    })
                    .child(text_content),
            );
        }

        // Animated thinking dots for Sending status (Swift: ThinkingDotsView)
        if is_sending && !is_user {
            body = body.child(thinking_dots());
        }

        // Tool call rows (Swift: ToolRunRow) — assistant only, collapsible
        if !is_user {
            for (ti, tc) in tool_calls.iter().enumerate() {
                let key = format!("{idx}-{ti}");
                let is_expanded = self.expanded_tool_rows.contains(&key);
                let key_click = key.clone();
                let status_glyph = match tc.status {
                    ToolCallStatus::Running => "⋯",
                    ToolCallStatus::Done => "✓",
                    ToolCallStatus::Failed => "✕",
                };
                let status_color = match tc.status {
                    ToolCallStatus::Running => Text::MUTED,
                    ToolCallStatus::Done => Text::TERTIARY,
                    ToolCallStatus::Failed => gpui::Hsla {
                        h: 0.0,
                        s: 0.85,
                        l: 0.55,
                        a: 1.0,
                    },
                };
                let name = tc.name.clone();
                let chevron = if is_expanded { "▾" } else { "▸" };

                let row_header = div()
                    .id(gpui::SharedString::from(format!("tool-row-{idx}-{ti}")))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(Spacing::SM))
                    .px(px(Spacing::SM_MD))
                    .py(px(Spacing::XS))
                    .rounded(px(Radius::SM))
                    .bg(Hsla {
                        h: 0.0,
                        s: 0.0,
                        l: 1.0,
                        a: 0.04,
                    })
                    .cursor_pointer()
                    .on_click(cx.listener(
                        move |this: &mut ChatView,
                              _: &ClickEvent,
                              _: &mut Window,
                              cx: &mut Context<ChatView>| {
                            if this.expanded_tool_rows.contains(&key_click) {
                                this.expanded_tool_rows.remove(&key_click);
                            } else {
                                this.expanded_tool_rows.insert(key_click.clone());
                            }
                            cx.notify();
                        },
                    ))
                    .child(
                        div()
                            .text_color(status_color)
                            .text_size(px(FontSize::SM))
                            .child(status_glyph),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_color(Text::TERTIARY)
                            .text_size(px(FontSize::SM))
                            .child(name),
                    )
                    .child(
                        div()
                            .text_color(Text::MUTED)
                            .text_size(px(FontSize::XS))
                            .child(chevron),
                    );

                let mut tool_wrap = div()
                    .id(gpui::SharedString::from(format!("tool-wrap-{idx}-{ti}")))
                    .flex()
                    .flex_col()
                    .rounded(px(Radius::SM))
                    .overflow_hidden()
                    .child(row_header);

                if is_expanded {
                    let mut detail = div()
                        .px(px(Spacing::SM_MD))
                        .py(px(Spacing::XS))
                        .flex()
                        .flex_col()
                        .gap(px(Spacing::XS))
                        .bg(Hsla {
                            h: 0.0,
                            s: 0.0,
                            l: 1.0,
                            a: 0.02,
                        });
                    // Input args (Swift: argsSection with pretty-printed JSON)
                    if let Some(ref args) = tc.input_json {
                        detail = detail
                            .child(
                                div()
                                    .text_color(Text::MUTED)
                                    .text_size(px(FontSize::XXS))
                                    .child("INPUT"),
                            )
                            .child(
                                div()
                                    .px(px(Spacing::SM))
                                    .py(px(Spacing::XS))
                                    .rounded(px(Radius::XS))
                                    .bg(Hsla {
                                        h: 0.0,
                                        s: 0.0,
                                        l: 0.0,
                                        a: 0.3,
                                    })
                                    .text_color(Text::TERTIARY)
                                    .text_size(px(FontSize::XS))
                                    .child(args.clone()),
                            );
                    }
                    // Output (Swift: resultSection)
                    if let Some(ref result) = tc.result_text {
                        detail = detail
                            .child(
                                div()
                                    .text_color(Text::MUTED)
                                    .text_size(px(FontSize::XXS))
                                    .child("OUTPUT"),
                            )
                            .child(
                                div()
                                    .px(px(Spacing::SM))
                                    .py(px(Spacing::XS))
                                    .rounded(px(Radius::XS))
                                    .bg(Hsla {
                                        h: 0.0,
                                        s: 0.0,
                                        l: 0.0,
                                        a: 0.3,
                                    })
                                    .text_color(Text::SECONDARY)
                                    .text_size(px(FontSize::XS))
                                    .child(result.clone()),
                            );
                    } else if tc.input_json.is_none() {
                        detail = detail.child(
                            div()
                                .text_color(Text::MUTED)
                                .text_size(px(FontSize::XS))
                                .child("(no output)"),
                        );
                    }
                    tool_wrap = tool_wrap.child(detail);
                }

                body = body.child(tool_wrap);
            }

            // Copy button below assistant text (Swift: CopyMessageButton, visible on hover)
            if !msg.text.is_empty() {
                body = body.child(
                    div()
                        .id(gpui::SharedString::from(format!("chat-copy-{idx}")))
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(Spacing::XS))
                        .cursor_pointer()
                        .on_click(cx.listener(|_, _, _, _| { /* copy to clipboard */ }))
                        .child(
                            div()
                                .text_color(Text::MUTED)
                                .text_size(px(FontSize::XS))
                                .child("⎘"),
                        )
                        .child(
                            div()
                                .text_color(Text::MUTED)
                                .text_size(px(FontSize::XS))
                                .child("Copy"),
                        ),
                );
            }
        }

        div()
            .id(gpui::SharedString::from(format!("chat-msg-{idx}")))
            .flex()
            .flex_row()
            .w_full()
            .px(px(Spacing::LG_XL))
            .mb(px(Spacing::XL))
            .when(is_user, |el| el.child(div().flex_1().min_w(px(48.0))))
            .child(body)
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
            .child(thinking_dots())
    }

    /// Starter prompts shown when no messages exist.
    fn render_starter_prompts(&self) -> impl IntoElement {
        let mut col = div()
            .id("starter-prompts")
            .flex()
            .flex_col()
            .gap(px(Spacing::XS))
            .px(px(Spacing::LG_XL))
            .py(px(Spacing::MD))
            // Heading: "Ask anything, or start with:" (matches Swift empty-state header)
            .child(
                div()
                    .pb(px(Spacing::XS))
                    .text_size(px(FontSize::SM_MD))
                    .text_color(Text::SECONDARY)
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .child("Ask anything, or start with:"),
            );

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
                            .text_color(Text::TERTIARY) // icon: tertiary (matches Swift .tertiaryColor)
                            .text_size(px(FontSize::SM_MD))
                            .child(icon.to_string()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_color(Text::PRIMARY) // label: primary (matches Swift .primaryColor)
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
                            .text_color(if is_active {
                                Text::PRIMARY
                            } else {
                                Text::SECONDARY
                            })
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
            let messages_snapshot: Vec<ChatMessage> = self.model.messages.clone();
            for (idx, msg) in messages_snapshot.iter().enumerate() {
                messages_div = messages_div.child(self.render_message(idx, msg, cx));
            }
        }

        // Thinking dots shown while streaming (Swift: ThinkingDots when isStreaming)
        if self.model.is_agent_running {
            messages_div = messages_div.child(Self::render_thinking_indicator());
        }

        // ── Model picker dropdown (Swift: ModelPickerMenu) ──
        let model_picker_open = self.model_picker_open;
        let selected_model_idx = self.selected_model_idx;

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
                        if let Some(agent_text) = this.model.handle_send_action(false) {
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
                            this.spawn_agent_turn(agent_text, cx);
                        }
                        cx.notify();
                    },
                ))
                .child(
                    div()
                        .text_size(px(FontSize::SM))
                        .text_color(if can_send {
                            Background::BASE
                        } else {
                            Text::MUTED
                        })
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
                    // Bottom bar: model picker + send button
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .px(px(Spacing::MD_LG))
                            .pb(px(Spacing::SM_MD))
                            .pt(px(Spacing::XXS))
                            .child(
                                // Model picker button (Swift: ModelPickerButton)
                                div()
                                    .id("model-picker-btn")
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(Spacing::XXS))
                                    .cursor_pointer()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.model_picker_open = !this.model_picker_open;
                                        cx.notify();
                                    }))
                                    .child(
                                        div()
                                            .text_color(Text::MUTED)
                                            .text_size(px(FontSize::XS))
                                            .child(AVAILABLE_MODELS[self.selected_model_idx]),
                                    )
                                    .child(
                                        div()
                                            .text_color(Text::MUTED)
                                            .text_size(px(FontSize::XXS))
                                            .child("▾"),
                                    ),
                            )
                            .child(div().flex_1())
                            .child(send_btn),
                    ),
            );

        // Build model dropdown if open
        let mut model_dropdown = div()
            .id("model-picker-dropdown-wrap")
            .absolute()
            .bottom(px(4.0))
            .left(px(Spacing::MD_LG))
            .bg(Background::RAISED)
            .border_1()
            .border_color(BorderColors::SUBTLE)
            .rounded(px(Radius::SM))
            .flex()
            .flex_col()
            .py(px(Spacing::XS));
        for (mi, model_name) in AVAILABLE_MODELS.iter().enumerate() {
            let is_selected = mi == selected_model_idx;
            model_dropdown = model_dropdown.child(
                div()
                    .id(gpui::SharedString::from(format!("model-opt-{mi}")))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(Spacing::SM))
                    .px(px(Spacing::MD))
                    .py(px(Spacing::XS))
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.selected_model_idx = mi;
                        this.model_picker_open = false;
                        cx.notify();
                    }))
                    .child(
                        div()
                            .text_color(if is_selected {
                                Accent::PRIMARY
                            } else {
                                Hsla {
                                    h: 0.0,
                                    s: 0.0,
                                    l: 1.0,
                                    a: 0.0,
                                }
                            })
                            .text_size(px(FontSize::XS))
                            .child("✓"),
                    )
                    .child(
                        div()
                            .text_color(if is_selected {
                                Text::PRIMARY
                            } else {
                                Text::SECONDARY
                            })
                            .text_size(px(FontSize::SM))
                            .child(*model_name),
                    ),
            );
        }

        // Wrap the input footer in a relative container so the dropdown can float above it
        let footer_container = div()
            .id("chat-footer-wrap")
            .relative()
            .when(model_picker_open, |el| el.child(model_dropdown))
            .child(input_footer);

        // Tab bar height for padding compensation (matches Layout::PANEL_HEADER_HEIGHT).
        let tab_h = crate::theme::Layout::PANEL_HEADER_HEIGHT;
        let history_open = self.history_open;
        let tab_bar = self.render_tab_bar(cx);

        // Messages area with top padding so content isn't hidden under the floating tab bar.
        let padded_messages = messages_div.pt(px(tab_h));

        // ZStack-style: messages behind, tab bar floating on top (Swift: ZStack(alignment:.top)).
        let messages_zone = div()
            .id("chat-messages-zone")
            .flex()
            .flex_col()
            .flex_1()
            .relative()
            .child(padded_messages)
            .child(
                div()
                    .id("chat-tab-bar-float")
                    .absolute()
                    .top_0()
                    .left_0()
                    .w_full()
                    .child(tab_bar),
            )
            .when(history_open, |el| el.child(self.render_history_popover()));

        // ── Full layout ──
        div()
            .id("fronda-chat")
            .track_focus(&self.focus_handle.clone())
            .on_key_down(cx.listener(Self::handle_key_down))
            .flex()
            .flex_col()
            .size_full()
            .bg(Background::SURFACE)
            .child(messages_zone)
            .child(picker)
            .child(footer_container)
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

/// Basic markdown renderer — handles: # headers, **bold**, `code`, blank-line paragraphs.
/// gpui can't mix inline styles in a single text node, so this returns a flex-col of lines.
fn render_md_text(text: &str, base_size: f32) -> impl IntoElement {
    let mut col = div().flex().flex_col().gap(px(4.0));
    let mut in_code_block = false;
    let mut code_block_lines: Vec<String> = Vec::new();

    let flush_code_block = |lines: &mut Vec<String>| -> gpui::AnyElement {
        let block = lines.join("\n");
        lines.clear();
        div()
            .px(px(8.0))
            .py(px(6.0))
            .rounded(px(4.0))
            .bg(gpui::Hsla {
                h: 0.0,
                s: 0.0,
                l: 0.0,
                a: 0.35,
            })
            .text_color(Text::SECONDARY)
            .text_size(px(base_size - 1.0))
            .child(block)
            .into_any_element()
    };

    for raw_line in text.lines() {
        // Code fence toggle
        if raw_line.trim_start().starts_with("```") {
            if in_code_block {
                in_code_block = false;
                col = col.child(flush_code_block(&mut code_block_lines));
            } else {
                in_code_block = true;
            }
            continue;
        }
        if in_code_block {
            code_block_lines.push(raw_line.to_string());
            continue;
        }

        // Heading
        let (line_text, is_h1, is_h2) = if raw_line.starts_with("# ") {
            (&raw_line[2..], true, false)
        } else if raw_line.starts_with("## ") {
            (&raw_line[3..], false, true)
        } else {
            (raw_line, false, false)
        };

        let font_size = if is_h1 {
            base_size + 4.0
        } else if is_h2 {
            base_size + 2.0
        } else {
            base_size
        };
        let weight = if is_h1 || is_h2 {
            gpui::FontWeight::BOLD
        } else {
            gpui::FontWeight::NORMAL
        };

        // Inline segments: split on **bold** and `code`
        let segments = parse_inline(line_text);
        let mut row = div()
            .flex()
            .flex_row()
            .flex_wrap()
            .items_baseline()
            .gap(px(0.0));
        for seg in segments {
            row = match seg {
                InlineSeg::Plain(s) => row.child(
                    div()
                        .text_color(Text::PRIMARY)
                        .text_size(px(font_size))
                        .font_weight(weight)
                        .child(s),
                ),
                InlineSeg::Bold(s) => row.child(
                    div()
                        .text_color(Text::PRIMARY)
                        .text_size(px(font_size))
                        .font_weight(gpui::FontWeight::BOLD)
                        .child(s),
                ),
                InlineSeg::Code(s) => row.child(
                    div()
                        .px(px(3.0))
                        .rounded(px(3.0))
                        .bg(gpui::Hsla {
                            h: 0.0,
                            s: 0.0,
                            l: 1.0,
                            a: 0.08,
                        })
                        .text_color(Text::SECONDARY)
                        .text_size(px(font_size - 1.0))
                        .child(s),
                ),
            };
        }
        col = col.child(row);
    }
    // Flush unclosed code block
    if in_code_block && !code_block_lines.is_empty() {
        col = col.child(flush_code_block(&mut code_block_lines));
    }
    col
}

enum InlineSeg {
    Plain(String),
    Bold(String),
    Code(String),
}

fn parse_inline(s: &str) -> Vec<InlineSeg> {
    let mut segs = Vec::new();
    let mut rest = s;
    while !rest.is_empty() {
        if let Some(bold_start) = rest.find("**") {
            let code_start = rest.find('`').unwrap_or(usize::MAX);
            if code_start < bold_start {
                // code comes first
                if code_start > 0 {
                    segs.push(InlineSeg::Plain(rest[..code_start].to_string()));
                }
                let inner = &rest[code_start + 1..];
                if let Some(code_end) = inner.find('`') {
                    segs.push(InlineSeg::Code(inner[..code_end].to_string()));
                    rest = &inner[code_end + 1..];
                } else {
                    segs.push(InlineSeg::Plain(rest.to_string()));
                    break;
                }
            } else {
                // bold comes first
                if bold_start > 0 {
                    segs.push(InlineSeg::Plain(rest[..bold_start].to_string()));
                }
                let inner = &rest[bold_start + 2..];
                if let Some(bold_end) = inner.find("**") {
                    segs.push(InlineSeg::Bold(inner[..bold_end].to_string()));
                    rest = &inner[bold_end + 2..];
                } else {
                    segs.push(InlineSeg::Plain(rest.to_string()));
                    break;
                }
            }
        } else if let Some(code_start) = rest.find('`') {
            if code_start > 0 {
                segs.push(InlineSeg::Plain(rest[..code_start].to_string()));
            }
            let inner = &rest[code_start + 1..];
            if let Some(code_end) = inner.find('`') {
                segs.push(InlineSeg::Code(inner[..code_end].to_string()));
                rest = &inner[code_end + 1..];
            } else {
                segs.push(InlineSeg::Plain(rest.to_string()));
                break;
            }
        } else {
            segs.push(InlineSeg::Plain(rest.to_string()));
            break;
        }
    }
    segs
}

/// Three animated pulsing dots (Swift: ThinkingDots) — staggered opacity loop at 900ms.
fn thinking_dots() -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(3.0))
        .children((0u32..3).map(|i| {
            div()
                .w(px(5.0))
                .h(px(5.0))
                .rounded_full()
                .bg(Text::TERTIARY)
                .with_animation(
                    format!("thinking-dot-{i}"),
                    Animation::new(Duration::from_millis(900)).repeat(),
                    move |el, delta| {
                        let phase = (delta + i as f32 / 3.0) % 1.0;
                        let a: f32 = if phase < 1.0 / 3.0 { 1.0 } else { 0.25 };
                        el.opacity(a)
                    },
                )
        }))
}

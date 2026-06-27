//! AI Edit tab content — matches Swift AIEditTab.
//!
//! Layout (top to bottom):
//!   • Scope section (toggles: Replace clip source / Use trimmed portion)
//!   • AI Enhance section (collapsible): Upscale / Edit / Rerun / Create Video
//!   • AI Audio section (collapsible, video clips only): Music / SFX

use crate::theme::{Accent, Background, BorderColors, FontSize, Spacing, Text};
use gpui::{
    div, prelude::*, px, App, ClickEvent, Context, FocusHandle, Focusable, IntoElement,
    ParentElement, Render, Styled, Window,
};

#[derive(Debug, Clone)]
pub struct AiEditTabState {
    pub enhance_expanded: bool,
    pub audio_expanded: bool,
    pub replace_clip_source: bool,
    pub use_trimmed_portion: bool,
    pub place_audio_on_timeline: bool,
    pub is_video: bool,
}

impl Default for AiEditTabState {
    fn default() -> Self {
        Self {
            enhance_expanded: true,
            audio_expanded: true,
            replace_clip_source: false,
            use_trimmed_portion: true,
            place_audio_on_timeline: true,
            is_video: true,
        }
    }
}

pub struct AiEditTabView {
    pub state: AiEditTabState,
    focus_handle: FocusHandle,
}

impl AiEditTabView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            state: AiEditTabState::default(),
            focus_handle: cx.focus_handle(),
        }
    }
}

impl Focusable for AiEditTabView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

fn section_header_static(label: &str) -> impl IntoElement {
    div()
        .text_color(Text::MUTED)
        .text_size(px(FontSize::XXS))
        .w_full()
        .child(label.to_uppercase())
}

fn section_header_collapsible(label: &str, expanded: bool) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(Spacing::XS))
        .w_full()
        .child(
            div()
                .text_color(Text::MUTED)
                .text_size(px(FontSize::XXS))
                .child(if expanded { "▾" } else { "▸" }),
        )
        .child(
            div()
                .text_color(Text::MUTED)
                .text_size(px(FontSize::XXS))
                .child(label.to_uppercase()),
        )
}

fn toggle_row(icon: &str, label: &str, is_on: bool) -> impl IntoElement {
    let pill_bg = if is_on { Accent::PRIMARY } else { Text::MUTED };
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(Spacing::SM))
        .w_full()
        .child(
            div()
                .w(px(20.0))
                .text_color(if is_on { Accent::PRIMARY } else { Text::TERTIARY })
                .text_size(px(FontSize::SM))
                .child(icon.to_string()),
        )
        .child(
            div()
                .flex_1()
                .text_color(Text::SECONDARY)
                .text_size(px(FontSize::SM))
                .child(label.to_string()),
        )
        .child(
            div()
                .w(px(28.0))
                .h(px(16.0))
                .rounded_full()
                .bg(pill_bg)
                .flex()
                .items_center()
                .when(is_on, |el| el.justify_end())
                .px(px(2.0))
                .child(
                    div()
                        .w(px(12.0))
                        .h(px(12.0))
                        .rounded_full()
                        .bg(Background::BASE),
                ),
        )
}

fn action_row(icon: &str, title: &str, description: &str, trigger: &str, enabled: bool) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .items_start()
        .gap(px(Spacing::SM))
        .w_full()
        .child(
            div()
                .w(px(20.0))
                .pt(px(2.0))
                .text_color(if enabled { Text::SECONDARY } else { Text::MUTED })
                .text_size(px(FontSize::MD))
                .child(icon.to_string()),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .gap(px(Spacing::XXS))
                .child(
                    div()
                        .text_color(if enabled { Text::PRIMARY } else { Text::MUTED })
                        .text_size(px(FontSize::SM))
                        .child(title.to_string()),
                )
                .child(
                    div()
                        .text_color(Text::TERTIARY)
                        .text_size(px(FontSize::XS))
                        .child(description.to_string()),
                ),
        )
        .child(
            div()
                .px(px(Spacing::SM))
                .py(px(Spacing::XXS))
                .rounded_full()
                .border_1()
                .border_color(if enabled { BorderColors::PRIMARY } else { BorderColors::SUBTLE })
                .text_color(if enabled { Text::SECONDARY } else { Text::MUTED })
                .text_size(px(FontSize::XS))
                .child(trigger.to_string()),
        )
}

impl Render for AiEditTabView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let enhance_exp = self.state.enhance_expanded;
        let audio_exp = self.state.audio_expanded;
        let replace = self.state.replace_clip_source;
        let trimmed = self.state.use_trimmed_portion;
        let place_audio = self.state.place_audio_on_timeline;
        let is_video = self.state.is_video;

        div()
            .track_focus(&self.focus_handle.clone())
            .id("ai-edit-scroll")
            .flex()
            .flex_col()
            .w_full()
            .overflow_y_scroll()
            .px(px(Spacing::LG))
            .py(px(Spacing::MD))
            .gap(px(Spacing::XL))
            // Scope section
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(Spacing::SM_MD))
                    .child(section_header_static("Scope"))
                    .child(toggle_row("↩", "Replace clip source", replace))
                    .child(toggle_row("✂", "Use trimmed portion only", trimmed)),
            )
            // AI Enhance section (collapsible)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(Spacing::SM_MD))
                    .child(
                        div()
                            .id("btn-enhance-toggle")
                            .w_full()
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _: &mut Window, cx| {
                                this.state.enhance_expanded = !this.state.enhance_expanded;
                                cx.notify();
                            }))
                            .child(section_header_collapsible("AI Enhance", enhance_exp)),
                    )
                    .when(enhance_exp, |el| {
                        el.child(action_row("✦", "Upscale", "Enhance resolution with AI", "Upscale", true))
                          .child(action_row("★", "Edit", "Transform with a prompt or motion reference", "Edit", true))
                          .child(action_row("↺", "Rerun", "Regenerate with the same parameters", "Rerun", true))
                          .when(is_video, |el2| {
                              el2.child(action_row("▷", "Create Video", "Use as first frame or reference", "Create", true))
                          })
                    }),
            )
            // AI Audio section (video only, collapsible)
            .when(is_video, |el| {
                el.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(Spacing::SM_MD))
                        .child(
                            div()
                                .id("btn-audio-toggle")
                                .w_full()
                                .cursor_pointer()
                                .on_click(cx.listener(|this, _: &ClickEvent, _: &mut Window, cx| {
                                    this.state.audio_expanded = !this.state.audio_expanded;
                                    cx.notify();
                                }))
                                .child(section_header_collapsible("AI Audio", audio_exp)),
                        )
                        .when(audio_exp, |el| {
                            el.child(toggle_row("↗", "Place on timeline", place_audio))
                              .child(action_row("♪", "Music", "Generate background music from video", "Generate", true))
                              .child(action_row("~", "Sound Effects", "Generate sound effects from video", "Generate", true))
                        }),
                )
            })
    }
}

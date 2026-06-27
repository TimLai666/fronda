/// Preview panel gpui view — canvas + scrub bar + transport controls.
///
/// Matches PreviewContainerView.swift layout.

use crate::preview_model::PlaybackState;
use crate::theme::{Accent, Background, BorderColors, FontSize, Layout, Spacing, Text};
use gpui::{
    div, prelude::*, px, App, Context, FocusHandle, Focusable, IntoElement, InteractiveElement,
    ParentElement, Render, Styled, Window,
};

pub struct PreviewView {
    pub state: PlaybackState,
    focus_handle: FocusHandle,
}

impl PreviewView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            state: PlaybackState::new(),
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn toggle_play(&mut self, cx: &mut Context<Self>) {
        self.state.toggle_play();
        cx.notify();
    }

    pub fn go_to_start(&mut self, cx: &mut Context<Self>) {
        self.state.go_to_start();
        cx.notify();
    }

    pub fn go_to_end(&mut self, cx: &mut Context<Self>) {
        self.state.go_to_end();
        cx.notify();
    }

    pub fn step_backward(&mut self, cx: &mut Context<Self>) {
        self.state.step_backward();
        cx.notify();
    }

    pub fn step_forward(&mut self, cx: &mut Context<Self>) {
        self.state.step_forward();
        cx.notify();
    }
}

impl Focusable for PreviewView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

/// Transport button: 32×28px matching Swift frame(width:32, height:28).
fn transport_btn(
    id: &str,
    glyph: &str,
    highlight: bool,
) -> impl IntoElement {
    div()
        .id(id.to_string())
        .w(px(32.0))
        .h(px(28.0))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .rounded(px(4.0))
        .text_color(if highlight { Text::PRIMARY } else { Text::SECONDARY })
        .text_size(px(FontSize::MD))
        .child(glyph.to_string())
}

impl Render for PreviewView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_playing = self.state.is_playing;
        let fraction = self.state.playhead_fraction();
        let current_tc = self.state.format_timecode();
        let total_tc = self.state.format_total();

        div()
            .id("preview-panel")
            .flex()
            .flex_col()
            .size_full()
            .bg(Background::BASE)
            // ── Header tab bar ──
            .child(
                div()
                    .id("preview-header")
                    .flex()
                    .flex_row()
                    .items_center()
                    .w_full()
                    .h(px(Layout::PANEL_HEADER_HEIGHT))
                    .px(px(Spacing::MD))
                    .gap(px(Spacing::XS))
                    .bg(Background::RAISED)
                    .border_b_1()
                    .border_color(BorderColors::PRIMARY)
                    // Nav chevrons
                    .child(
                        div()
                            .w(px(18.0))
                            .h(px(22.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(Text::MUTED)
                            .text_size(px(FontSize::SM))
                            .child("<"),
                    )
                    .child(
                        div()
                            .w(px(18.0))
                            .h(px(22.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(Text::MUTED)
                            .text_size(px(FontSize::SM))
                            .child(">"),
                    )
                    // Active tab label with bottom underline
                    .child(
                        div()
                            .px(px(Spacing::XS))
                            .pb(px(2.0))
                            .border_b(px(1.5))
                            .border_color(Text::PRIMARY)
                            .text_color(Text::PRIMARY)
                            .text_size(px(FontSize::SM))
                            .child("Timeline"),
                    ),
            )
            // ── Canvas area ──
            .child(
                div()
                    .id("preview-canvas")
                    .flex()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .w_full()
                    .bg(Background::BASE)
                    .child(
                        div()
                            .text_color(Text::MUTED)
                            .text_size(px(FontSize::SM))
                            .child("Preview"),
                    ),
            )
            // ── Scrub bar: 12px height matching Swift ──
            .child(
                div()
                    .id("preview-scrub")
                    .relative()
                    .w_full()
                    .h(px(12.0))
                    .bg(BorderColors::SUBTLE)
                    .cursor_pointer()
                    // Progress fill
                    .child(
                        div()
                            .absolute()
                            .top_0()
                            .left_0()
                            .h_full()
                            .w(px((fraction as f32) * Layout::PREVIEW_MIN_WIDTH))
                            .bg(BorderColors::DIVIDER),
                    )
                    // Thumb indicator
                    .child(
                        div()
                            .absolute()
                            .top(px(3.0))
                            .left(px((fraction as f32) * Layout::PREVIEW_MIN_WIDTH - 3.0))
                            .w(px(6.0))
                            .h(px(6.0))
                            .rounded_full()
                            .bg(Text::PRIMARY),
                    ),
            )
            // ── Transport bar: 36px height matching Swift ──
            .child(
                div()
                    .id("preview-transport")
                    .flex()
                    .flex_row()
                    .items_center()
                    .w_full()
                    .h(px(36.0))
                    .px(px(Spacing::MD))
                    .bg(Background::RAISED)
                    .border_t_1()
                    .border_color(BorderColors::PRIMARY)
                    // Left: timecode — orange accent for current, tertiary for separator and total
                    .child(
                        div()
                            .flex()
                            .flex_1()
                            .flex_row()
                            .items_center()
                            .gap(px(2.0))
                            .child(
                                div()
                                    .text_color(Accent::TIMECODE)
                                    .text_size(px(FontSize::SM))
                                    .child(current_tc),
                            )
                            .child(
                                div()
                                    .text_color(Text::TERTIARY)
                                    .text_size(px(FontSize::SM))
                                    .child("/"),
                            )
                            .child(
                                div()
                                    .text_color(Text::SECONDARY)
                                    .text_size(px(FontSize::SM))
                                    .child(total_tc),
                            ),
                    )
                    // Center: 5 transport buttons
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(Spacing::XS))
                            .child(
                                transport_btn("btn-go-start", "|<", false)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.go_to_start(cx);
                                    })),
                            )
                            .child(
                                transport_btn("btn-step-back", "<<", false)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.step_backward(cx);
                                    })),
                            )
                            .child(
                                transport_btn("btn-play", if is_playing { "⏸" } else { "▶" }, true)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.toggle_play(cx);
                                    })),
                            )
                            .child(
                                transport_btn("btn-step-fwd", ">>", false)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.step_forward(cx);
                                    })),
                            )
                            .child(
                                transport_btn("btn-go-end", ">|", false)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.go_to_end(cx);
                                    })),
                            ),
                    )
                    // Right: zoom / aspect info
                    .child(
                        div()
                            .flex()
                            .flex_1()
                            .flex_row()
                            .justify_end()
                            .items_center()
                            .gap(px(Spacing::SM))
                            .child(
                                div()
                                    .text_color(Text::MUTED)
                                    .text_size(px(FontSize::XS))
                                    .child("16:9"),
                            )
                            .child(
                                div()
                                    .text_color(Text::MUTED)
                                    .text_size(px(FontSize::XS))
                                    .child("Fit"),
                            ),
                    ),
            )
    }
}

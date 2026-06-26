/// Preview panel gpui view — canvas + scrub bar + transport controls.
///
/// Matches PreviewContainerView.swift layout.

use crate::preview_model::PlaybackState;
use crate::theme::{Background, BorderColors, FontSize, Layout, Spacing, Text};
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

impl Render for PreviewView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let timecode = self.state.format_timecode();
        let is_playing = self.state.is_playing;
        let fraction = self.state.playhead_fraction();

        div()
            .id("preview-panel")
            .flex()
            .flex_col()
            .size_full()
            .bg(Background::BASE)
            // ── Panel header (tab bar) ──
            .child(
                div()
                    .id("preview-header")
                    .flex()
                    .flex_row()
                    .items_center()
                    .w_full()
                    .h(px(Layout::PANEL_HEADER_HEIGHT))
                    .px(px(Spacing::MD))
                    .bg(Background::RAISED)
                    .border_b_1()
                    .border_color(BorderColors::PRIMARY)
                    .child(
                        div()
                            .px(px(Spacing::SM))
                            .py(px(Spacing::XXS))
                            .rounded(px(4.0))
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
            // ── Scrub bar ──
            .child(
                div()
                    .id("preview-scrub")
                    .relative()
                    .w_full()
                    .h(px(Spacing::XS))
                    .bg(BorderColors::SUBTLE)
                    .child(
                        div()
                            .absolute()
                            .top_0()
                            .left_0()
                            .h_full()
                            .w(px((fraction * Layout::PREVIEW_MIN_WIDTH as f64) as f32))
                            .bg(BorderColors::DIVIDER),
                    ),
            )
            // ── Transport bar ──
            .child(
                div()
                    .id("preview-transport")
                    .flex()
                    .flex_row()
                    .items_center()
                    .w_full()
                    .h(px(Spacing::XXL + Spacing::LG_XL))
                    .px(px(Spacing::MD))
                    .bg(Background::RAISED)
                    .border_t_1()
                    .border_color(BorderColors::PRIMARY)
                    // Left: timecode
                    .child(
                        div()
                            .flex()
                            .flex_1()
                            .items_center()
                            .child(
                                div()
                                    .text_color(Text::TERTIARY)
                                    .text_size(px(FontSize::XS))
                                    .child(timecode),
                            ),
                    )
                    // Center: navigation buttons
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(Spacing::SM))
                            .child(
                                div()
                                    .id("btn-go-start")
                                    .w(px(Spacing::XL_XXL))
                                    .h(px(Spacing::XL_XXL))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_pointer()
                                    .text_color(Text::SECONDARY)
                                    .text_size(px(FontSize::SM))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.go_to_start(cx);
                                    }))
                                    .child("|<"),
                            )
                            .child(
                                div()
                                    .id("btn-step-back")
                                    .w(px(Spacing::XL_XXL))
                                    .h(px(Spacing::XL_XXL))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_pointer()
                                    .text_color(Text::SECONDARY)
                                    .text_size(px(FontSize::SM))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.step_backward(cx);
                                    }))
                                    .child("<<"),
                            )
                            .child(
                                div()
                                    .id("btn-play")
                                    .w(px(Spacing::XL_XXL))
                                    .h(px(Spacing::XL_XXL))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_pointer()
                                    .text_color(Text::PRIMARY)
                                    .text_size(px(FontSize::MD))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.toggle_play(cx);
                                    }))
                                    .child(if is_playing { "⏸" } else { "▶" }),
                            )
                            .child(
                                div()
                                    .id("btn-step-fwd")
                                    .w(px(Spacing::XL_XXL))
                                    .h(px(Spacing::XL_XXL))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_pointer()
                                    .text_color(Text::SECONDARY)
                                    .text_size(px(FontSize::SM))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.step_forward(cx);
                                    }))
                                    .child(">>"),
                            )
                            .child(
                                div()
                                    .id("btn-go-end")
                                    .w(px(Spacing::XL_XXL))
                                    .h(px(Spacing::XL_XXL))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .cursor_pointer()
                                    .text_color(Text::SECONDARY)
                                    .text_size(px(FontSize::SM))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.go_to_end(cx);
                                    }))
                                    .child(">|"),
                            ),
                    )
                    // Right: zoom level placeholder
                    .child(
                        div()
                            .flex()
                            .flex_1()
                            .justify_end()
                            .items_center()
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

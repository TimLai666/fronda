/// Preview panel gpui view — canvas + scrub bar + transport controls.
///
/// Matches PreviewContainerView.swift layout.
/// TransformOverlayView and CropOverlayView are layered on top of the canvas.

use crate::crop_overlay_view::CropOverlayView;
use crate::preview_model::PlaybackState;
use crate::theme::{Accent, Background, BorderColors, FontSize, Layout, Spacing, Text};
use crate::transform_overlay_view::TransformOverlayView;
use gpui::{
    div, prelude::*, px, App, Context, Entity, FocusHandle, Focusable, IntoElement,
    InteractiveElement, ParentElement, Render, Styled, Window,
};

/// Canvas overlay state — mirrors Swift PreviewView offline/generating/failed states.
#[derive(Debug, Clone, PartialEq)]
pub enum CanvasOverlay {
    None,
    /// Media file is offline (not found on disk).
    Offline,
    /// AI generation in progress.
    Generating { progress_pct: u8 },
    /// Generation or render failed.
    Failed { message: String },
}

pub struct PreviewView {
    pub state: PlaybackState,
    pub show_transform_overlay: bool,
    pub show_crop_overlay: bool,
    pub canvas_overlay: CanvasOverlay,
    transform_overlay: Entity<TransformOverlayView>,
    crop_overlay: Entity<CropOverlayView>,
    focus_handle: FocusHandle,
}

impl PreviewView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            state: PlaybackState::new(),
            show_transform_overlay: false,
            show_crop_overlay: false,
            canvas_overlay: CanvasOverlay::None,
            transform_overlay: cx.new(|cx| TransformOverlayView::new(cx)),
            crop_overlay: cx.new(|cx| CropOverlayView::new(cx)),
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

fn transport_btn(id: &str, glyph: &str, highlight: bool) -> gpui::Stateful<gpui::Div> {
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
        let show_transform = self.show_transform_overlay;
        let show_crop = self.show_crop_overlay;
        let canvas_overlay = self.canvas_overlay.clone();

        let transform_entity = self.transform_overlay.clone();
        let crop_entity = self.crop_overlay.clone();

        div()
            .id("preview-panel")
            .flex()
            .flex_col()
            .size_full()
            .bg(Background::BASE)
            // Header tab bar
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
                    .child(
                        div()
                            .id("preview-back")
                            .w(px(18.0))
                            .h(px(22.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .text_color(Text::MUTED)
                            .text_size(px(FontSize::SM))
                            .on_click(cx.listener(|_, _, _, _| {}))
                            .child("<"),
                    )
                    .child(
                        div()
                            .id("preview-fwd")
                            .w(px(18.0))
                            .h(px(22.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .text_color(Text::MUTED)
                            .text_size(px(FontSize::SM))
                            .on_click(cx.listener(|_, _, _, _| {}))
                            .child(">"),
                    )
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
            // Canvas area (relative so overlays can stack absolutely)
            .child(
                div()
                    .id("preview-canvas")
                    .flex()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .w_full()
                    .relative()
                    .bg(Background::BASE)
                    // Placeholder text — hidden when an overlay is shown
                    .when(canvas_overlay == CanvasOverlay::None, |el| {
                        el.child(
                            div()
                                .text_color(Text::MUTED)
                                .text_size(px(FontSize::SM))
                                .child("Preview"),
                        )
                    })
                    // Offline overlay (Swift: "Media Offline" message)
                    .when(canvas_overlay == CanvasOverlay::Offline, |el| {
                        el.child(
                            div()
                                .flex()
                                .flex_col()
                                .items_center()
                                .gap(px(Spacing::SM))
                                .child(
                                    div()
                                        .text_color(Text::MUTED)
                                        .text_size(px(FontSize::MD_LG))
                                        .child("Media Offline"),
                                )
                                .child(
                                    div()
                                        .text_color(Text::MUTED)
                                        .text_size(px(FontSize::SM))
                                        .child("File not found on disk"),
                                ),
                        )
                    })
                    // Generating overlay (Swift: generation progress spinner)
                    .when(matches!(canvas_overlay, CanvasOverlay::Generating { .. }), |el| {
                        let pct = if let CanvasOverlay::Generating { progress_pct } = &canvas_overlay { *progress_pct } else { 0 };
                        el.child(
                            div()
                                .flex()
                                .flex_col()
                                .items_center()
                                .gap(px(Spacing::MD))
                                .child(
                                    div()
                                        .text_color(Accent::PRIMARY)
                                        .text_size(px(FontSize::DISPLAY))
                                        .child("✦"),
                                )
                                .child(
                                    div()
                                        .text_color(Text::SECONDARY)
                                        .text_size(px(FontSize::SM))
                                        .child(format!("Generating… {}%", pct)),
                                ),
                        )
                    })
                    // Failed overlay (Swift: error badge)
                    .when(matches!(canvas_overlay, CanvasOverlay::Failed { .. }), |el| {
                        let msg = if let CanvasOverlay::Failed { message } = &canvas_overlay { message.clone() } else { String::new() };
                        el.child(
                            div()
                                .flex()
                                .flex_col()
                                .items_center()
                                .gap(px(Spacing::SM))
                                .child(
                                    div()
                                        .text_color(gpui::Hsla { h: 0.0, s: 0.85, l: 0.55, a: 1.0 })
                                        .text_size(px(FontSize::MD_LG))
                                        .child("Generation Failed"),
                                )
                                .child(
                                    div()
                                        .text_color(Text::MUTED)
                                        .text_size(px(FontSize::SM))
                                        .child(msg),
                                ),
                        )
                    })
                    // Transform overlay — shown when select tool + clip selected
                    .when(show_transform, |el| {
                        el.child(
                            div()
                                .absolute()
                                .top_0()
                                .left_0()
                                .size_full()
                                .child(transform_entity),
                        )
                    })
                    // Crop overlay — shown when crop tool + clip selected
                    .when(show_crop, |el| {
                        el.child(
                            div()
                                .absolute()
                                .top_0()
                                .left_0()
                                .size_full()
                                .child(crop_entity),
                        )
                    }),
            )
            // Scrub bar
            .child(
                div()
                    .id("preview-scrub")
                    .relative()
                    .w_full()
                    .h(px(12.0))
                    .bg(BorderColors::SUBTLE)
                    .cursor_pointer()
                    .child(
                        div()
                            .absolute()
                            .top_0()
                            .left_0()
                            .h_full()
                            .w(px((fraction as f32) * Layout::PREVIEW_MIN_WIDTH))
                            .bg(BorderColors::DIVIDER),
                    )
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
            // Transport bar
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
                                transport_btn("btn-play", if is_playing { "||" } else { ">" }, true)
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

//! AI Generation panel gpui view — embedded inside the Media panel's media tab.
//!
//! Covers the Swift GenerationView: type picker, reference tiles, prompt, generate button.
//! Uses GenerationPanel theme constants for all sizing.

use crate::theme::{
    Accent, Background, BorderColors, FontSize, GenerationPanel, Radius, Spacing, Text,
};
use gpui::{
    div, prelude::*, px, App, ClickEvent, Context, FocusHandle, Focusable, Hsla,
    InteractiveElement, ParentElement, Render, Styled, Window,
};

/// AI generation type (matches Swift GenerationType).
#[derive(Debug, Clone, PartialEq, Copy)]
pub enum GenerationType {
    Video,
    Image,
    Audio,
}

impl GenerationType {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Video => "Video",
            Self::Image => "Image",
            Self::Audio => "Audio",
        }
    }

    pub fn all() -> &'static [Self] {
        &[Self::Video, Self::Image, Self::Audio]
    }
}

/// State for the generation panel.
#[derive(Debug, Clone)]
pub struct GenerationState {
    pub selected_type: GenerationType,
    pub prompt: String,
    pub is_generating: bool,
}

impl Default for GenerationState {
    fn default() -> Self {
        Self {
            selected_type: GenerationType::Video,
            prompt: String::new(),
            is_generating: false,
        }
    }
}

/// gpui Generation panel view, embedded in the Media panel.
pub struct GenerationView {
    pub state: GenerationState,
    focus_handle: FocusHandle,
}

impl GenerationView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            state: GenerationState::default(),
            focus_handle: cx.focus_handle(),
        }
    }
}

impl Focusable for GenerationView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

/// A reference media tile (first frame, last frame, or image reference).
fn ref_tile(label: &str) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .w(px(GenerationPanel::REFERENCE_TILE_WIDTH))
        .h(px(GenerationPanel::REFERENCE_TILE_HEIGHT))
        .rounded(px(Radius::SM))
        .border_1()
        .border_color(BorderColors::SUBTLE)
        .bg(Background::RAISED)
        .gap(px(Spacing::XXS))
        .cursor_pointer()
        .child(
            div()
                .text_color(Text::MUTED)
                .text_size(px(FontSize::MD))
                .child("+"),
        )
        .child(
            div()
                .text_color(Text::MUTED)
                .text_size(px(FontSize::XXS))
                .child(label.to_string()),
        )
}

impl Render for GenerationView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = self.state.selected_type;
        let prompt_text = if self.state.prompt.is_empty() {
            "Describe what to generate…".to_string()
        } else {
            self.state.prompt.clone()
        };
        let is_placeholder = self.state.prompt.is_empty();
        let is_generating = self.state.is_generating;

        // Active tab bg matches HoverHighlight(isActive: true)
        let active_tab_bg: Hsla = Hsla {
            h: 0.0,
            s: 0.0,
            l: 1.0,
            a: 0.10,
        };

        div()
            .id("generation-panel")
            .track_focus(&self.focus_handle.clone())
            .flex()
            .flex_col()
            .size_full()
            .bg(Background::SURFACE)
            // ── Type picker tabs ──
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .px(px(Spacing::SM_MD))
                    .py(px(Spacing::XS))
                    .gap(px(Spacing::XXS))
                    .border_b_1()
                    .border_color(BorderColors::SUBTLE)
                    .children(GenerationType::all().iter().map(|gen_type| {
                        let is_active = *gen_type == selected;
                        let gt = *gen_type;
                        div()
                            .id(gpui::SharedString::from(format!(
                                "gen-type-{}",
                                gen_type.label()
                            )))
                            .px(px(Spacing::SM_MD))
                            .h(px(24.0))
                            .flex()
                            .items_center()
                            .rounded(px(Radius::SM))
                            .cursor_pointer()
                            .bg(if is_active {
                                active_tab_bg
                            } else {
                                Background::SURFACE
                            })
                            .on_click(cx.listener(
                                move |this: &mut GenerationView,
                                      _event: &ClickEvent,
                                      _window: &mut Window,
                                      cx: &mut Context<GenerationView>| {
                                    this.state.selected_type = gt;
                                    cx.notify();
                                },
                            ))
                            .child(
                                div()
                                    .text_size(px(FontSize::SM))
                                    .text_color(if is_active {
                                        Text::PRIMARY
                                    } else {
                                        Text::TERTIARY
                                    })
                                    .child(gen_type.label()),
                            )
                    })),
            )
            // ── Reference tiles area (first frame, last frame) ──
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(Spacing::SM))
                    .px(px(Spacing::MD))
                    .py(px(Spacing::SM_MD))
                    .min_h(px(GenerationPanel::MEDIA_AREA_MIN_HEIGHT))
                    .when(selected == GenerationType::Video, |el| {
                        el.child(ref_tile("First Frame"))
                            .child(ref_tile("Last Frame"))
                    })
                    .when(selected == GenerationType::Image, |el| {
                        el.child(ref_tile("Reference"))
                    })
                    .when(selected == GenerationType::Audio, |el| {
                        el.child(ref_tile("Video Source"))
                    }),
            )
            // ── Prompt input ──
            .child(
                div()
                    .flex()
                    .flex_col()
                    .mx(px(Spacing::MD))
                    .mb(px(Spacing::SM_MD))
                    .rounded(px(Radius::MD))
                    .border_1()
                    .border_color(BorderColors::SUBTLE)
                    .bg(Background::RAISED)
                    .min_h(px(GenerationPanel::PROMPT_MIN_HEIGHT))
                    .child(
                        div()
                            .flex_1()
                            .px(px(Spacing::SM_MD))
                            .pt(px(Spacing::SM_MD))
                            .pb(px(Spacing::XS))
                            .text_size(px(FontSize::SM))
                            .text_color(if is_placeholder {
                                Text::MUTED
                            } else {
                                Text::PRIMARY
                            })
                            .child(prompt_text),
                    )
                    // Footer row: model badge + generate button
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .px(px(Spacing::SM_MD))
                            .pb(px(Spacing::SM_MD))
                            .pt(px(Spacing::XXS))
                            .child(
                                div()
                                    .flex_1()
                                    .text_color(Text::MUTED)
                                    .text_size(px(FontSize::XXS))
                                    .child(match selected {
                                        GenerationType::Video => "Sora · 5s · 1080p",
                                        GenerationType::Image => "Flux Pro",
                                        GenerationType::Audio => "Udio · 30s",
                                    }),
                            )
                            .child(
                                div()
                                    .id("btn-generate")
                                    .w(px(28.0))
                                    .h(px(28.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded_full()
                                    .cursor_pointer()
                                    .bg(if is_generating {
                                        BorderColors::PRIMARY
                                    } else {
                                        Accent::PRIMARY
                                    })
                                    .on_click(cx.listener(
                                        |this: &mut GenerationView,
                                         _event: &ClickEvent,
                                         _window: &mut Window,
                                         cx: &mut Context<GenerationView>| {
                                            this.state.is_generating = !this.state.is_generating;
                                            cx.notify();
                                        },
                                    ))
                                    .child(
                                        div()
                                            .text_size(px(FontSize::SM_MD))
                                            .text_color(if is_generating {
                                                Text::PRIMARY
                                            } else {
                                                Background::BASE
                                            })
                                            .child(if is_generating { "◼" } else { "✦" }),
                                    ),
                            ),
                    ),
            )
    }
}

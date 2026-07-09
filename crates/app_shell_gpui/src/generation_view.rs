//! AI Generation panel gpui view — embedded inside the Media panel's media tab.
//!
//! Covers the Swift GenerationView: type picker, reference tiles, prompt, generate button.
//! Uses GenerationPanel theme constants for all sizing.

use crate::text_area::{TextArea, TextAreaEvent};
use crate::theme::{
    Accent, Background, BorderColors, FontSize, GenerationPanel, Opacity, Radius, Spacing, Text,
    TrackColor,
};
use gpui::{
    div, prelude::*, px, svg, App, ClickEvent, Context, Entity, FocusHandle, Focusable, Hsla,
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

    pub fn icon_path(&self) -> &'static str {
        match self {
            Self::Video => "icons/video.svg",
            Self::Image => "icons/photo.svg",
            Self::Audio => "icons/waveform.svg",
        }
    }

    pub fn accent_color(&self) -> Hsla {
        match self {
            Self::Video => TrackColor::VIDEO,
            Self::Image => TrackColor::IMAGE,
            Self::Audio => TrackColor::AUDIO,
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
    /// Credits remaining (None = signed out / unknown).
    pub credits_remaining: Option<u32>,
    /// GEN-4: Video ref mode — true = First/Last frames, false = single Reference.
    pub use_first_last: bool,
    /// GEN-5: whether the model picker dropdown is open.
    pub show_model_picker: bool,
    /// Whether the credit top-off popover is open (CreditActionsPopover in Swift).
    pub show_credit_popover: bool,
    /// Whether the user is on a paid plan (affects popover content).
    pub is_paid_plan: bool,
}

impl Default for GenerationState {
    fn default() -> Self {
        Self {
            selected_type: GenerationType::Video,
            prompt: String::new(),
            is_generating: false,
            credits_remaining: Some(1_250),
            use_first_last: true,
            show_model_picker: false,
            show_credit_popover: false,
            is_paid_plan: false,
        }
    }
}

/// gpui Generation panel view, embedded in the Media panel.
pub struct GenerationView {
    pub state: GenerationState,
    focus_handle: FocusHandle,
    /// Multiline prompt editor (IME-capable); `state.prompt` mirrors it.
    prompt_area: Entity<TextArea>,
}

impl GenerationView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let prompt_area = cx.new(|cx| {
            TextArea::new(cx, "Describe what to generate…")
                .with_min_lines(3)
                .with_max_lines(8)
        });
        cx.subscribe(&prompt_area, |this, area, event, cx| {
            if matches!(event, TextAreaEvent::Edited) {
                this.state.prompt = area.read(cx).text().to_string();
                cx.notify();
            }
        })
        .detach();
        Self {
            state: GenerationState::default(),
            focus_handle: cx.focus_handle(),
            prompt_area,
        }
    }
}

impl Focusable for GenerationView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

/// A reference media tile (first frame, last frame, or image reference).
/// Label is displayed *above* the tile (matches Swift layout: Text(label) before the tile RoundedRectangle).
fn ref_tile(label: &str) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(Spacing::XXS))
        .child(
            div()
                .text_color(Text::TERTIARY)
                .text_size(px(FontSize::XXS))
                .child(label.to_string()),
        )
        .child(
            div()
                .w(px(GenerationPanel::REFERENCE_TILE_WIDTH))
                .h(px(GenerationPanel::REFERENCE_TILE_HEIGHT))
                .rounded(px(Radius::SM))
                .border_1()
                .border_color(BorderColors::SUBTLE)
                .bg(Background::RAISED)
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .child(
                    div()
                        .text_color(Text::MUTED)
                        .text_size(px(FontSize::MD))
                        .child("+"),
                ),
        )
}

impl Render for GenerationView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = self.state.selected_type;
        let use_first_last = self.state.use_first_last;
        let show_model_picker = self.state.show_model_picker;
        let is_generating = self.state.is_generating;
        let credits = self.state.credits_remaining;

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
            // aiGradientDark approximation: white(0.06)→white(0.11) gradient; avg ≈ SURFACE
            .bg(Background::SURFACE)
            .rounded(px(Radius::LG))
            .overflow_hidden()
            // ── Resize handle (Swift: resizeHandle — 24×2 capsule, white@soft, cursor ns-resize) ──
            .child(
                div()
                    .id("gen-resize-handle")
                    .flex()
                    .items_center()
                    .justify_center()
                    .w_full()
                    .h(px(Spacing::MD))
                    .cursor_ns_resize()
                    .child(
                        div()
                            .w(px(24.0))
                            .h(px(2.0))
                            .rounded_full()
                            .bg(Hsla { h: 0.0, s: 0.0, l: 1.0, a: Opacity::SOFT }),
                    ),
            )
            // ── Header: type tabs (left) + credit chip + activity + close (right) ──
            // Matches Swift: typeTabs · Spacer · CreditSummaryView(.compact) · ProjectActivityButton · xmark
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .px(px(Spacing::SM))
                    .py(px(Spacing::XS))
                    .gap(px(Spacing::XXS))
                    .border_b_1()
                    .border_color(BorderColors::SUBTLE)
                    // Type tabs — wrapped in pill container (Swift: typeTabs HStack with bg+strokeBorder)
                    .child({
                        let mut pill = div()
                            .id("gen-type-pill")
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(Spacing::XXS))
                            .px(px(Spacing::XXS))
                            .py(px(Spacing::XXS))
                            .rounded(px(Radius::SM))
                            .bg(Hsla { h: 0.0, s: 0.0, l: 1.0, a: Opacity::SUBTLE })
                            .border_1()
                            .border_color(Hsla { h: 0.0, s: 0.0, l: 1.0, a: Opacity::FAINT });
                        for gen_type in GenerationType::all() {
                            let is_active = *gen_type == selected;
                            let gt = *gen_type;
                            let icon_path = gen_type.icon_path();
                            let accent = gen_type.accent_color();
                            let icon_color = if is_active { accent } else { Text::TERTIARY };
                            let text_color = if is_active { Text::PRIMARY } else { Text::TERTIARY };
                            pill = pill.child(
                                div()
                                    .id(gpui::SharedString::from(format!(
                                        "gen-type-{}",
                                        gen_type.label()
                                    )))
                                    .px(px(Spacing::SM_MD))
                                    .h(px(22.0))
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(Spacing::XS))
                                    .rounded(px(Radius::XS_SM))
                                    .cursor_pointer()
                                    .bg(if is_active { active_tab_bg } else { Hsla { h: 0.0, s: 0.0, l: 0.0, a: 0.0 } })
                                    .on_click(cx.listener(
                                        move |this: &mut GenerationView,
                                              _event: &ClickEvent,
                                              _window: &mut Window,
                                              cx: &mut Context<GenerationView>| {
                                            this.state.selected_type = gt;
                                            cx.notify();
                                        },
                                    ))
                                    .child(svg().path(icon_path).w(px(11.0)).h(px(11.0)).text_color(icon_color))
                                    .child(
                                        div()
                                            .text_size(px(FontSize::SM))
                                            .text_color(text_color)
                                            .child(gen_type.label()),
                                    )
                            );
                        }
                        pill
                    })
                    // Spacer
                    .child(div().flex_1())
                    // Credit chip (CreditSummaryView.compact) — only when credits available
                    .when_some(credits, |el, c| {
                        let show_popover = self.state.show_credit_popover;
                        let is_paid = self.state.is_paid_plan;
                        el.child(
                            div()
                                .relative()
                                .child(
                                    div()
                                        .id("credit-chip")
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .gap(px(Spacing::XS))
                                        .px(px(Spacing::SM))
                                        .py(px(Spacing::XXS))
                                        .rounded_full()
                                        .border_1()
                                        .border_color(BorderColors::SUBTLE)
                                        .cursor_pointer()
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            this.state.show_credit_popover = !this.state.show_credit_popover;
                                            cx.notify();
                                        }))
                                        .child(
                                            div()
                                                .text_color(Accent::PRIMARY)
                                                .text_size(px(FontSize::SM))
                                                .child("$"),
                                        )
                                        .child(
                                            div()
                                                .text_color(Accent::PRIMARY)
                                                .text_size(px(FontSize::XS))
                                                .child(format!("{c}")),
                                        ),
                                )
                                // CreditActionsPopover — anchored below chip
                                .when(show_popover, |el| {
                                    el.child(credit_popover(is_paid, cx))
                                })
                        )
                    })
                    // Project activity icon button
                    .child(
                        div()
                            .id("btn-gen-activity")
                            .w(px(22.0))
                            .h(px(22.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(Radius::XS))
                            .cursor_pointer()
                            .text_color(Text::TERTIARY)
                            .text_size(px(FontSize::XS))
                            .child("≡"),
                    )
                    // Close button (xmark)
                    .child(
                        div()
                            .id("btn-gen-close")
                            .w(px(22.0))
                            .h(px(22.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(Radius::XS))
                            .cursor_pointer()
                            .text_color(Text::TERTIARY)
                            .text_size(px(FontSize::XXS))
                            .child("✕"),
                    ),
            )
            // ── Reference tiles area ──
            .child(
                div()
                    .flex()
                    .flex_col()
                    .px(px(Spacing::MD))
                    .py(px(Spacing::SM_MD))
                    .gap(px(Spacing::XS))
                    .min_h(px(GenerationPanel::MEDIA_AREA_MIN_HEIGHT))
                    // GEN-4: segmented toggle for Video type (First/Last vs Reference)
                    .when(selected == GenerationType::Video, |el| {
                        let seg_bg: Hsla = Hsla { h: 0.0, s: 0.0, l: 1.0, a: 0.06 };
                        let active_seg: Hsla = Hsla { h: 0.0, s: 0.0, l: 1.0, a: 0.14 };
                        el.child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(1.0))
                                .rounded(px(Radius::XS_SM))
                                .bg(seg_bg)
                                .p(px(1.0))
                                .child(
                                    div()
                                        .id("gen-seg-first-last")
                                        .px(px(Spacing::SM))
                                        .h(px(20.0))
                                        .flex()
                                        .items_center()
                                        .rounded(px(Radius::XS))
                                        .cursor_pointer()
                                        .bg(if use_first_last { active_seg } else { Hsla { h:0.0,s:0.0,l:0.0,a:0.0 } })
                                        .text_size(px(FontSize::XXS))
                                        .text_color(if use_first_last { Text::PRIMARY } else { Text::MUTED })
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.state.use_first_last = true;
                                            cx.notify();
                                        }))
                                        .child("First / Last"),
                                )
                                .child(
                                    div()
                                        .id("gen-seg-reference")
                                        .px(px(Spacing::SM))
                                        .h(px(20.0))
                                        .flex()
                                        .items_center()
                                        .rounded(px(Radius::XS))
                                        .cursor_pointer()
                                        .bg(if !use_first_last { active_seg } else { Hsla { h:0.0,s:0.0,l:0.0,a:0.0 } })
                                        .text_size(px(FontSize::XXS))
                                        .text_color(if !use_first_last { Text::PRIMARY } else { Text::MUTED })
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.state.use_first_last = false;
                                            cx.notify();
                                        }))
                                        .child("Reference"),
                                ),
                        )
                    })
                    // Tile row
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(Spacing::SM))
                            .when(selected == GenerationType::Video && use_first_last, |el| {
                                el.child(ref_tile("First Frame"))
                                    .child(ref_tile("Last Frame"))
                            })
                            .when(selected == GenerationType::Video && !use_first_last, |el| {
                                el.child(ref_tile("Reference"))
                            })
                            .when(selected == GenerationType::Image, |el| {
                                el.child(ref_tile("Reference"))
                            })
                            .when(selected == GenerationType::Audio, |el| {
                                el.child(ref_tile("Video Source"))
                            }),
                    ),
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
                            .id("gen-prompt-input")
                            .flex_1()
                            .px(px(Spacing::SM_MD))
                            .pt(px(Spacing::SM_MD))
                            .pb(px(Spacing::XS))
                            .text_size(px(FontSize::SM))
                            .text_color(Text::PRIMARY)
                            .cursor_text()
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                window.focus(&this.prompt_area.focus_handle(cx), cx);
                                cx.notify();
                            }))
                            .child(self.prompt_area.clone()),
                    )
                    // Footer row: gear + model picker + generate button
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .px(px(Spacing::SM_MD))
                            .pb(px(Spacing::SM_MD))
                            .pt(px(Spacing::XXS))
                            .gap(px(Spacing::XS))
                            // GEN-6: Settings gear button
                            .child(
                                div()
                                    .id("btn-gen-settings")
                                    .w(px(20.0))
                                    .h(px(20.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(px(Radius::XS))
                                    .cursor_pointer()
                                    .text_color(Text::MUTED)
                                    .text_size(px(FontSize::SM))
                                    .on_click(cx.listener(|_, _, _, _| {}))
                                    .child("⚙"),
                            )
                            // GEN-5: Model picker button — tappable label + chevron
                            .child(
                                div()
                                    .id("btn-gen-model-picker")
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(2.0))
                                    .cursor_pointer()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.state.show_model_picker = !this.state.show_model_picker;
                                        cx.notify();
                                    }))
                                    .child(
                                        div()
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
                                            .text_color(Text::MUTED)
                                            .text_size(px(FontSize::XXS))
                                            .child("⌄"),
                                    ),
                            )
                            .child(div().flex_1())
                            // Generate button
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
                    )
                    // GEN-5: Model picker dropdown — appears inside the prompt box when open
                    .when(show_model_picker, |el| {
                        el.child(
                            div()
                                .id("gen-model-picker-dropdown")
                                .border_t_1()
                                .border_color(BorderColors::SUBTLE)
                                .flex()
                                .flex_col()
                                .overflow_hidden()
                                .child(model_picker_row("Sora 1.5", "5s · 1080p"))
                                .child(model_picker_row("Sora 1.0", "5s · 720p"))
                                .child(model_picker_row("Runway Gen-3", "10s · 720p")),
                        )
                    }),
            )
    }
}

/// Credit top-off popover (CreditActionsPopover in Swift).
/// Paid users: dollar-amount input + Buy button (TopOffField).
/// Free users: upgrade prompt.
fn credit_popover(is_paid: bool, cx: &mut Context<GenerationView>) -> impl IntoElement {
    div()
        .id("credit-popover")
        .absolute()
        .bottom(px(28.0))
        .right(px(0.0))
        .w(px(220.0))
        .rounded(px(crate::theme::Radius::MD))
        .bg(crate::theme::Background::RAISED)
        .border_1()
        .border_color(crate::theme::BorderColors::PRIMARY)
        .shadow_lg()
        .p(px(crate::theme::Spacing::MD))
        .flex()
        .flex_col()
        .gap(px(crate::theme::Spacing::SM))
        // Dismiss on outside click is handled by toggling show_credit_popover
        .on_click(|_, _, _| {}) // absorb click so parent doesn't close it
        .when(is_paid, |el| {
            // TopOffField: amount input + Buy button
            el.child(
                div()
                    .text_color(crate::theme::Text::PRIMARY)
                    .text_size(px(FontSize::SM))
                    .child("Add credits"),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(crate::theme::Spacing::XS))
                    .child(
                        div()
                            .flex_1()
                            .h(px(28.0))
                            .px(px(crate::theme::Spacing::SM))
                            .border_1()
                            .border_color(crate::theme::BorderColors::SUBTLE)
                            .rounded(px(crate::theme::Radius::SM))
                            .flex()
                            .items_center()
                            .text_color(crate::theme::Text::MUTED)
                            .text_size(px(FontSize::SM))
                            .child("$10.00"),
                    )
                    .child(
                        div()
                            .id("credit-buy-btn")
                            .px(px(crate::theme::Spacing::SM))
                            .h(px(28.0))
                            .rounded(px(crate::theme::Radius::SM))
                            .bg(crate::theme::Accent::PRIMARY)
                            .flex()
                            .items_center()
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.state.show_credit_popover = false;
                                cx.notify();
                            }))
                            .text_color(crate::theme::Background::BASE)
                            .text_size(px(FontSize::SM))
                            .child("Buy"),
                    ),
            )
        })
        .when(!is_paid, |el| {
            // Free plan: upgrade prompt
            el.child(
                div()
                    .text_color(crate::theme::Text::SECONDARY)
                    .text_size(px(FontSize::SM))
                    .child("Upgrade to add credits"),
            )
            .child(
                div()
                    .id("credit-upgrade-btn")
                    .w_full()
                    .px(px(crate::theme::Spacing::MD))
                    .py(px(crate::theme::Spacing::XS))
                    .rounded(px(crate::theme::Radius::SM))
                    .bg(crate::theme::Accent::PRIMARY)
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.state.show_credit_popover = false;
                        cx.notify();
                    }))
                    .text_color(crate::theme::Background::BASE)
                    .text_size(px(FontSize::SM))
                    .child("Account settings"),
            )
        })
}

/// A single row in the model picker dropdown.
fn model_picker_row(name: &str, detail: &str) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .px(px(Spacing::SM_MD))
        .py(px(Spacing::XS))
        .gap(px(Spacing::SM))
        .cursor_pointer()
        .child(
            div()
                .flex_1()
                .text_color(Text::PRIMARY)
                .text_size(px(FontSize::SM))
                .child(name.to_string()),
        )
        .child(
            div()
                .text_color(Text::MUTED)
                .text_size(px(FontSize::XXS))
                .child(detail.to_string()),
        )
}

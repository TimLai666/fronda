//! TourOverlay — matches Swift TourOverlay + TourController.
//!
//! Full-screen overlay rendered on top of the app window during onboarding.
//! `TourFlow` is the pure step machine (Swift TourController.makeSteps /
//! start / advance / back / end); the view renders the current step:
//!
//!   • Intro: 600px wide, title + instruction + hero area + Skip/Next
//!   • Spotlight callout: 320px wide, "Step N of M" + Skip/Back/Next
//!   • Outro: 600px wide, completion message + "Start creating"
//!
//! Spotlight cutout: the 4-band scrim + accent border is implemented
//! (`spotlight_scrim`), but no producer sets `spotlight_rect` yet — element
//! anchor bounds need a registration seam in the host views (media panel,
//! editor panes, timeline ruler), which are outside this change. Until that
//! lands, spotlight steps render a centered callout over a full scrim.

use crate::theme::{
    Accent, Background, BorderColors, BorderWidth, FontSize, Opacity, Radius, Spacing, Text,
};
use gpui::{
    div, prelude::*, px, relative, App, ClickEvent, Context, FocusHandle, Focusable, IntoElement,
    ParentElement, Render, Styled, Window,
};

/// Step kind (Swift TourStep.Kind; anchor targets are deferred until an
/// element-bounds registry exists).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TourStepKind {
    Intro,
    Spotlight,
    Outro,
}

/// One step of the onboarding tour.
#[derive(Debug, Clone, PartialEq)]
pub struct TourStep {
    pub kind: TourStepKind,
    pub title: &'static str,
    pub instruction: &'static str,
}

/// Pure tour step machine (Swift TourController).
#[derive(Debug, Clone)]
pub struct TourFlow {
    steps: Vec<TourStep>,
    index: Option<usize>,
}

impl TourFlow {
    /// The standard step list (Swift TourController.makeSteps). The gated
    /// smart-search step is omitted — the local visual model is not ported.
    pub fn standard() -> Self {
        let spotlight = |title, instruction| TourStep {
            kind: TourStepKind::Spotlight,
            title,
            instruction,
        };
        Self {
            steps: vec![
                TourStep {
                    kind: TourStepKind::Intro,
                    title: "Tutorial",
                    instruction: "Let's take a quick tour of the workspace and what you can do.",
                },
                spotlight(
                    "Media panel",
                    "This is where all your footage and assets live.",
                ),
                spotlight(
                    "Import footage",
                    "Import your footage here, or drag and drop, or copy-paste, into the media panel.",
                ),
                spotlight("Generate", "Click Generate to open the generation panel."),
                spotlight(
                    "Generation panel",
                    "Generate video, image, or audio with different models and settings. Drag assets from the media panel above into the reference frame.",
                ),
                spotlight(
                    "Preview",
                    "This is your preview panel to play a selected media or the whole timeline.",
                ),
                spotlight(
                    "Screenshot a frame",
                    "Take a screenshot of the preview and use it as a reference for generation. Particularly useful for creating AI transitions.",
                ),
                spotlight(
                    "Inspector",
                    "This is your inspector panel. Select a clip from the timeline to edit it.",
                ),
                spotlight(
                    "Timeline",
                    "Your timeline: the top half is video, the bottom half is audio. This is where you edit. Right-click a clip for some cool AI features such as upscale, edit, or generate music.",
                ),
                spotlight(
                    "Select a range",
                    "This is the timeline ruler. Shift+drag on the ruler to select a range to render. You can pick any slot to AI edit or generate music that fits that range.",
                ),
                spotlight(
                    "AI agent",
                    "Chat with your agent! It can generate content, edit clips, organize your assets, and much more. Start by signing in, or bring your own Anthropic API key.",
                ),
                TourStep {
                    kind: TourStepKind::Outro,
                    title: "You're all set",
                    instruction: "Start creating, or explore these to get the most out of Fronda.",
                },
            ],
            index: None,
        }
    }

    pub fn start(&mut self) {
        self.index = Some(0);
    }

    /// Next step, or end after the last one.
    pub fn advance(&mut self) {
        match self.index {
            Some(i) if i + 1 < self.steps.len() => self.index = Some(i + 1),
            Some(_) => self.end(),
            None => {}
        }
    }

    pub fn back(&mut self) {
        if let Some(i) = self.index {
            if i > 0 {
                self.index = Some(i - 1);
            }
        }
    }

    pub fn end(&mut self) {
        self.index = None;
    }

    pub fn is_active(&self) -> bool {
        self.index.is_some()
    }

    pub fn current(&self) -> Option<&TourStep> {
        self.index.and_then(|i| self.steps.get(i))
    }

    /// Raw step index — the intro is 0, so the first spotlight reads
    /// "Step 1 of N" (Swift: `Text("Step \(index) of \(spotlightCount)")`).
    pub fn step_number(&self) -> usize {
        self.index.unwrap_or(0)
    }

    pub fn spotlight_count(&self) -> usize {
        self.steps
            .iter()
            .filter(|s| s.kind == TourStepKind::Spotlight)
            .count()
    }
}

/// Clamp a normalized (left, top, right, bottom) spotlight rect into 0..=1
/// with non-negative extent, so malformed anchor bounds can't produce
/// negative-sized scrim bands.
pub fn clamp_spotlight_rect(rect: (f32, f32, f32, f32)) -> (f32, f32, f32, f32) {
    let l = rect.0.clamp(0.0, 1.0);
    let t = rect.1.clamp(0.0, 1.0);
    let r = rect.2.clamp(l, 1.0);
    let b = rect.3.clamp(t, 1.0);
    (l, t, r, b)
}

pub struct TourOverlayView {
    pub flow: TourFlow,
    /// Normalized (0..=1) cutout rect for the current spotlight step. No
    /// producer yet — see the module docs for the anchor-bounds blocker.
    pub spotlight_rect: Option<(f32, f32, f32, f32)>,
    focus_handle: FocusHandle,
}

impl TourOverlayView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            flow: TourFlow::standard(),
            spotlight_rect: None,
            focus_handle: cx.focus_handle(),
        }
    }

    /// Begin the tour from its first step (Welcome "Watch Tutorial").
    pub fn start(&mut self, cx: &mut Context<Self>) {
        self.flow.start();
        cx.notify();
    }
}

impl Focusable for TourOverlayView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

fn tour_button(id: &'static str, label: &str, is_primary: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .px(px(Spacing::MD_LG))
        .py(px(Spacing::SM))
        .rounded_full()
        .cursor_pointer()
        .when(is_primary, |el| {
            el.bg(Accent::PRIMARY).text_color(Background::BASE)
        })
        .when(!is_primary, |el| {
            el.border_1()
                .border_color(BorderColors::PRIMARY)
                .text_color(Text::SECONDARY)
        })
        .text_size(px(FontSize::SM))
        .child(label.to_string())
}

/// Spotlight scrim: 4 dark bands leaving a rect cutout with an accent border.
fn spotlight_scrim(rect: (f32, f32, f32, f32)) -> impl IntoElement {
    let (l, t, r, b) = clamp_spotlight_rect(rect);
    let cw = r - l;
    let ch = b - t;
    let dim = gpui::Hsla {
        h: 0.0,
        s: 0.0,
        l: 0.0,
        a: Opacity::STRONG,
    };
    div()
        .size_full()
        .relative()
        // Top band
        .child(
            div()
                .absolute()
                .top(relative(0.0))
                .left(relative(0.0))
                .w(relative(1.0))
                .h(relative(t))
                .bg(dim),
        )
        // Bottom band
        .child(
            div()
                .absolute()
                .top(relative(b))
                .left(relative(0.0))
                .w(relative(1.0))
                .h(relative(1.0 - b))
                .bg(dim),
        )
        // Left band (middle row)
        .child(
            div()
                .absolute()
                .top(relative(t))
                .left(relative(0.0))
                .w(relative(l))
                .h(relative(ch))
                .bg(dim),
        )
        // Right band (middle row)
        .child(
            div()
                .absolute()
                .top(relative(t))
                .left(relative(r))
                .w(relative(1.0 - r))
                .h(relative(ch))
                .bg(dim),
        )
        // Accent border around cutout (approximates Swift gradient border)
        .child(
            div()
                .absolute()
                .top(relative(t))
                .left(relative(l))
                .w(relative(cw))
                .h(relative(ch))
                .border(px(BorderWidth::MEDIUM))
                .border_color(Accent::PRIMARY)
                .rounded(px(Radius::SM)),
        )
}

impl TourOverlayView {
    fn intro_card(&self, step: &TourStep, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("tour-intro-card")
            .occlude()
            .flex()
            .flex_col()
            .gap(px(Spacing::MD_LG))
            .w(px(600.0))
            .rounded(px(Radius::LG))
            .bg(Background::RAISED)
            .p(px(Spacing::XL))
            .border_1()
            .border_color(BorderColors::SUBTLE)
            .child(
                div()
                    .text_color(Text::PRIMARY)
                    .text_size(px(FontSize::XL))
                    .child(step.title.to_string()),
            )
            .child(
                div()
                    .text_color(Text::SECONDARY)
                    .text_size(px(FontSize::SM_MD))
                    .child(step.instruction.to_string()),
            )
            // Hero image placeholder (Swift: tour-hero.jpg)
            .child(
                div()
                    .w_full()
                    .h(px(180.0))
                    .rounded(px(Radius::MD))
                    .bg(Background::SURFACE)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_color(Text::MUTED)
                            .text_size(px(FontSize::MD))
                            .child("[ Preview ]"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .justify_end()
                    .gap(px(Spacing::SM))
                    .child(
                        tour_button("tour-intro-skip", "Skip", false).on_click(cx.listener(
                            |this, _, _, cx| {
                                this.flow.end();
                                cx.notify();
                            },
                        )),
                    )
                    .child(
                        tour_button("tour-intro-next", "Next", true).on_click(cx.listener(
                            |this, _, _, cx| {
                                this.flow.advance();
                                cx.notify();
                            },
                        )),
                    ),
            )
    }

    fn callout_card(&self, step: &TourStep, cx: &mut Context<Self>) -> impl IntoElement {
        let number = self.flow.step_number();
        let total = self.flow.spotlight_count();
        div()
            .id("tour-callout-card")
            .occlude()
            .flex()
            .flex_col()
            .gap(px(Spacing::MD))
            .w(px(320.0))
            .rounded(px(Radius::MD))
            .bg(Background::RAISED)
            .p(px(Spacing::LG))
            .border_1()
            .border_color(BorderColors::SUBTLE)
            .child(
                div()
                    .text_color(Text::TERTIARY)
                    .text_size(px(FontSize::XS))
                    .child(format!("Step {} of {}", number, total)),
            )
            .child(
                div()
                    .text_color(Text::PRIMARY)
                    .text_size(px(FontSize::MD))
                    .child(step.title.to_string()),
            )
            .child(
                div()
                    .text_color(Text::SECONDARY)
                    .text_size(px(FontSize::SM_MD))
                    .child(step.instruction.to_string()),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .justify_end()
                    .gap(px(Spacing::SM))
                    .child(
                        tour_button("tour-skip", "Skip", false).on_click(cx.listener(
                            |this, _, _, cx| {
                                this.flow.end();
                                cx.notify();
                            },
                        )),
                    )
                    .child(
                        tour_button("tour-back", "Back", false).on_click(cx.listener(
                            |this, _, _, cx| {
                                this.flow.back();
                                cx.notify();
                            },
                        )),
                    )
                    .child(tour_button("tour-next", "Next", true).on_click(cx.listener(
                        |this, _, _, cx| {
                            this.flow.advance();
                            cx.notify();
                        },
                    ))),
            )
    }

    fn outro_card(&self, step: &TourStep, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("tour-outro-card")
            .occlude()
            .flex()
            .flex_col()
            .gap(px(Spacing::MD_LG))
            .w(px(600.0))
            .rounded(px(Radius::LG))
            .bg(Background::RAISED)
            .p(px(Spacing::XL))
            .border_1()
            .border_color(BorderColors::SUBTLE)
            .child(
                div()
                    .text_color(Text::PRIMARY)
                    .text_size(px(FontSize::XL))
                    .child(step.title.to_string()),
            )
            .child(
                div()
                    .text_color(Text::SECONDARY)
                    .text_size(px(FontSize::SM_MD))
                    .child(step.instruction.to_string()),
            )
            .child(div().flex().flex_row().justify_end().child(
                tour_button("tour-done", "Start creating", true).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.flow.end();
                        cx.notify();
                    },
                )),
            ))
    }
}

impl Render for TourOverlayView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(step) = self.flow.current().cloned() else {
            return div();
        };
        let spotlight = self.spotlight_rect;
        let is_spotlight = step.kind == TourStepKind::Spotlight;

        // Scrim: clicking it ends the tour only on spotlight steps (Swift:
        // `.onTapGesture { if isSpotlight { tour.end() } }`).
        let scrim = div()
            .id("tour-overlay")
            .track_focus(&self.focus_handle.clone())
            .size_full()
            .absolute()
            .top(px(0.0))
            .left(px(0.0))
            .flex()
            .items_center()
            .justify_center()
            .bg(gpui::Hsla {
                h: 0.0,
                s: 0.0,
                l: 0.0,
                a: Opacity::STRONG,
            })
            .when(is_spotlight, |el| {
                el.on_click(cx.listener(|this, _: &ClickEvent, _: &mut Window, cx| {
                    this.flow.end();
                    cx.notify();
                }))
            });

        let overlay = match step.kind {
            TourStepKind::Intro => scrim.child(self.intro_card(&step, cx)).into_any_element(),
            TourStepKind::Outro => scrim.child(self.outro_card(&step, cx)).into_any_element(),
            TourStepKind::Spotlight => {
                if let Some(rect) = spotlight {
                    let rect = clamp_spotlight_rect(rect);
                    // Spotlight mode: 4-band scrim with cutout + callout card
                    div()
                        .id("tour-spotlight-layer")
                        .size_full()
                        .absolute()
                        .top(px(0.0))
                        .left(px(0.0))
                        .relative()
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.flow.end();
                            cx.notify();
                        }))
                        .child(spotlight_scrim(rect))
                        // Callout card anchored at top-right of spotlight
                        .child(
                            div()
                                .absolute()
                                .top(relative(rect.1))
                                .left(relative(rect.2 + 0.01))
                                .child(self.callout_card(&step, cx)),
                        )
                        .into_any_element()
                } else {
                    scrim.child(self.callout_card(&step, cx)).into_any_element()
                }
            }
        };

        div().size_full().relative().child(overlay)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_flow_mirrors_swift_step_list() {
        let flow = TourFlow::standard();
        assert_eq!(flow.steps.len(), 12);
        assert_eq!(flow.steps[0].kind, TourStepKind::Intro);
        assert_eq!(flow.steps[11].kind, TourStepKind::Outro);
        assert_eq!(flow.spotlight_count(), 10);
        assert!(!flow.is_active(), "tour is idle until started");
        assert!(flow.current().is_none());
    }

    #[test]
    fn start_shows_intro_and_advance_walks_to_end() {
        let mut flow = TourFlow::standard();
        flow.start();
        assert_eq!(flow.current().unwrap().kind, TourStepKind::Intro);
        for _ in 0..11 {
            flow.advance();
        }
        assert_eq!(flow.current().unwrap().kind, TourStepKind::Outro);
        flow.advance();
        assert!(!flow.is_active(), "advancing past outro ends the tour");
    }

    #[test]
    fn step_numbering_matches_swift_labels() {
        let mut flow = TourFlow::standard();
        flow.start();
        flow.advance(); // first spotlight
        assert_eq!(flow.step_number(), 1, "intro is 0, first spotlight reads 1");
        assert_eq!(flow.current().unwrap().title, "Media panel");
    }

    #[test]
    fn back_stops_at_first_step_and_end_deactivates() {
        let mut flow = TourFlow::standard();
        flow.start();
        flow.back();
        assert_eq!(flow.step_number(), 0, "back at intro stays at intro");
        flow.advance();
        flow.back();
        assert_eq!(flow.current().unwrap().kind, TourStepKind::Intro);
        flow.end();
        assert!(!flow.is_active());
        flow.advance();
        assert!(!flow.is_active(), "advance on an ended tour is a no-op");
    }

    #[test]
    fn clamp_spotlight_rect_sanitizes_bounds() {
        assert_eq!(
            clamp_spotlight_rect((0.2, 0.3, 0.6, 0.7)),
            (0.2, 0.3, 0.6, 0.7)
        );
        // Out-of-range values clamp into 0..=1.
        assert_eq!(
            clamp_spotlight_rect((-0.5, -1.0, 1.5, 2.0)),
            (0.0, 0.0, 1.0, 1.0)
        );
        // Inverted rects collapse to zero extent instead of negative bands.
        let (l, t, r, b) = clamp_spotlight_rect((0.8, 0.9, 0.2, 0.1));
        assert!(r >= l && b >= t);
        assert_eq!((r - l, b - t), (0.0, 0.0));
    }
}

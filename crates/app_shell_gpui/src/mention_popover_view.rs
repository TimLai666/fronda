//! MentionPopover — matches Swift MentionPicker.
//!
//! Appears above the chat input field when the user types '@' to mention media.
//!
//! Layout:
//!   • 260px wide popover
//!   • Tab strip: All / Video / Image / Audio
//!   • Scrollable list of media asset candidates
//!   • Empty state when no matches
//!
//! Each asset row: thumbnail placeholder (32px square) + name + type badge.

use crate::theme::{Background, BorderColors, FontSize, Radius, Spacing, Text};
use gpui::{
    div, prelude::*, px, App, Context, FocusHandle, Focusable, IntoElement, ParentElement, Render,
    SharedString, Styled, Window,
};

#[derive(Debug, Clone, PartialEq, Copy)]
pub enum MentionFilterTab {
    All,
    Video,
    Image,
    Audio,
}

impl MentionFilterTab {
    pub fn label(&self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Video => "Video",
            Self::Image => "Image",
            Self::Audio => "Audio",
        }
    }

    pub fn all() -> &'static [Self] {
        &[Self::All, Self::Video, Self::Image, Self::Audio]
    }
}

#[derive(Debug, Clone)]
pub struct MentionCandidate {
    pub id: SharedString,
    pub name: SharedString,
    pub media_type: SharedString,
}

#[derive(Debug, Clone)]
pub struct MentionPopoverState {
    pub query: SharedString,
    pub active_tab: MentionFilterTab,
    pub candidates: Vec<MentionCandidate>,
}

impl Default for MentionPopoverState {
    fn default() -> Self {
        Self {
            query: SharedString::default(),
            active_tab: MentionFilterTab::All,
            candidates: vec![
                MentionCandidate {
                    id: "1".into(),
                    name: "Interview A-roll".into(),
                    media_type: "Video".into(),
                },
                MentionCandidate {
                    id: "2".into(),
                    name: "Background music".into(),
                    media_type: "Audio".into(),
                },
                MentionCandidate {
                    id: "3".into(),
                    name: "Title card".into(),
                    media_type: "Image".into(),
                },
            ],
        }
    }
}

pub struct MentionPopoverView {
    pub state: MentionPopoverState,
    focus_handle: FocusHandle,
}

impl MentionPopoverView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            state: MentionPopoverState::default(),
            focus_handle: cx.focus_handle(),
        }
    }
}

impl Focusable for MentionPopoverView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

fn tab_pill(label: &'static str, is_active: bool) -> impl IntoElement {
    div()
        .px(px(Spacing::SM_MD))
        .py(px(Spacing::XXS))
        .rounded_full()
        .cursor_pointer()
        .bg(if is_active {
            Background::RAISED
        } else {
            Background::SURFACE
        })
        .text_color(if is_active {
            Text::PRIMARY
        } else {
            Text::MUTED
        })
        .text_size(px(FontSize::XS))
        .child(label)
}

/// Color-coded thumbnail for each media type (approximates per-asset thumbnail).
fn media_thumbnail(media_type: &str) -> impl IntoElement {
    let (hue, icon) = match media_type {
        "Video" => (0.097_f32, "▶"),
        "Audio" => (0.60_f32, "♫"),
        "Image" => (0.35_f32, "⬜"),
        _ => (0.0_f32, "▣"),
    };
    div()
        .w(px(32.0))
        .h(px(24.0))
        .rounded(px(Radius::XS))
        .bg(gpui::Hsla {
            h: hue,
            s: 0.45,
            l: 0.18,
            a: 1.0,
        })
        .flex()
        .items_center()
        .justify_center()
        .flex_shrink_0()
        .text_color(gpui::Hsla {
            h: hue,
            s: 0.65,
            l: 0.65,
            a: 1.0,
        })
        .text_size(px(FontSize::XXS))
        .child(icon)
}

fn candidate_row(c: &MentionCandidate) -> impl IntoElement {
    let mt = c.media_type.as_ref();
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(Spacing::SM))
        .w_full()
        .px(px(Spacing::SM_MD))
        .py(px(Spacing::XS))
        .cursor_pointer()
        // Color-coded thumbnail (type-based hue + icon)
        .child(media_thumbnail(mt))
        // Name
        .child(
            div()
                .flex_1()
                .text_color(Text::PRIMARY)
                .text_size(px(FontSize::SM))
                .child(c.name.clone()),
        )
        // Type badge
        .child(
            div()
                .px(px(Spacing::XS))
                .py(px(2.0))
                .rounded(px(Radius::XS))
                .bg(Background::SURFACE)
                .text_color(Text::TERTIARY)
                .text_size(px(FontSize::XXS))
                .child(c.media_type.clone()),
        )
}

impl Render for MentionPopoverView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active_tab = self.state.active_tab;
        let candidates: Vec<MentionCandidate> = self
            .state
            .candidates
            .iter()
            .filter(|c| {
                active_tab == MentionFilterTab::All
                    || c.media_type.as_ref().to_lowercase() == active_tab.label().to_lowercase()
            })
            .cloned()
            .collect();

        div()
            .id("mention-scroll")
            .track_focus(&self.focus_handle.clone())
            .flex()
            .flex_col()
            .w(px(260.0))
            .rounded(px(Radius::MD))
            .bg(Background::RAISED)
            .border_1()
            .border_color(BorderColors::PRIMARY)
            .overflow_hidden()
            // Tab strip
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(Spacing::XXS))
                    .px(px(Spacing::SM))
                    .py(px(Spacing::XS))
                    .border_b_1()
                    .border_color(BorderColors::SUBTLE)
                    .children(MentionFilterTab::all().iter().map(|t| {
                        let is_active = *t == active_tab;
                        let t_copy = *t;
                        div()
                            .id(gpui::SharedString::from(format!(
                                "mention-tab-{}",
                                t.label()
                            )))
                            .cursor_pointer()
                            .on_click(cx.listener(
                                move |this, _: &gpui::ClickEvent, _: &mut Window, cx| {
                                    this.state.active_tab = t_copy;
                                    cx.notify();
                                },
                            ))
                            .child(tab_pill(t.label(), is_active))
                    })),
            )
            // Content: empty state or candidate list
            .child(if candidates.is_empty() {
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .p(px(Spacing::MD))
                    .text_color(Text::MUTED)
                    .text_size(px(FontSize::XS))
                    .child("No media matches")
                    .into_any_element()
            } else {
                div()
                    .id("mention-candidates")
                    .flex()
                    .flex_col()
                    .max_h(px(280.0))
                    .overflow_y_scroll()
                    .children(candidates.iter().map(candidate_row))
                    .into_any_element()
            })
    }
}

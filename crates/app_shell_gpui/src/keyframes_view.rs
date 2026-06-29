//! Keyframes panel — matches Swift KeyframesPanel + KeyframesLaneRow.
//!
//! Layout:
//!   18px ruler strip (tick marks)
//!   14px colored clip strip (clip name)
//!   N × 22px lane rows (one per AnimatableProperty)
//!   Single red playhead line overlay at playhead_fraction position.
//!
//! Diamond markers (◇) are placed at frame-fraction positions in each lane.

use crate::theme::{Accent, Background, BorderColors, FontSize, Spacing, Text};
use gpui::{
    div, px, relative, App, Context, FocusHandle, Focusable, InteractiveElement, IntoElement,
    ParentElement, Render, Styled, Window,
};

const NAV_H: f32 = 24.0;
const RULER_H: f32 = 18.0;
const STRIP_H: f32 = 14.0;
const ROW_H: f32 = 22.0;

#[derive(Debug, Clone)]
pub struct KeyframeLane {
    pub property_label: &'static str,
    /// Frame positions as fractions 0.0..=1.0 within the clip.
    pub frame_fractions: Vec<f32>,
}

#[derive(Debug, Clone)]
pub struct KeyframesState {
    pub clip_name: String,
    pub clip_hue: f32,
    pub lanes: Vec<KeyframeLane>,
    pub playhead_fraction: f32,
}

impl Default for KeyframesState {
    fn default() -> Self {
        Self {
            clip_name: "Video Clip".to_string(),
            clip_hue: 0.12,
            lanes: vec![
                KeyframeLane {
                    property_label: "Position",
                    frame_fractions: vec![0.0, 0.5, 1.0],
                },
                KeyframeLane {
                    property_label: "Scale",
                    frame_fractions: vec![0.0, 1.0],
                },
                KeyframeLane {
                    property_label: "Rotation",
                    frame_fractions: vec![],
                },
                KeyframeLane {
                    property_label: "Opacity",
                    frame_fractions: vec![0.0, 0.3, 0.7, 1.0],
                },
                KeyframeLane {
                    property_label: "Crop",
                    frame_fractions: vec![],
                },
            ],
            playhead_fraction: 0.25,
        }
    }
}

pub struct KeyframesView {
    pub state: KeyframesState,
    focus_handle: FocusHandle,
}

impl KeyframesView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            state: KeyframesState::default(),
            focus_handle: cx.focus_handle(),
        }
    }
}

impl Focusable for KeyframesView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

fn nav_toolbar() -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .w_full()
        .h(px(NAV_H))
        .bg(Background::RAISED)
        .border_b_1()
        .border_color(BorderColors::SUBTLE)
        .px(px(Spacing::SM))
        .gap(px(Spacing::XS))
        // Prev keyframe
        .child(
            div()
                .id("kf-nav-prev")
                .w(px(20.0))
                .h(px(20.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(crate::theme::Radius::XS))
                .cursor_pointer()
                .text_color(Text::TERTIARY)
                .text_size(px(FontSize::SM_MD))
                .child("‹"),
        )
        // Add keyframe
        .child(
            div()
                .id("kf-nav-add")
                .w(px(20.0))
                .h(px(20.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(crate::theme::Radius::XS))
                .cursor_pointer()
                .text_color(Accent::PRIMARY)
                .text_size(px(FontSize::XS))
                .child("◆"),
        )
        // Next keyframe
        .child(
            div()
                .id("kf-nav-next")
                .w(px(20.0))
                .h(px(20.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(crate::theme::Radius::XS))
                .cursor_pointer()
                .text_color(Text::TERTIARY)
                .text_size(px(FontSize::SM_MD))
                .child("›"),
        )
        .child(div().flex_1())
        // Done / close label
        .child(
            div()
                .text_color(Text::MUTED)
                .text_size(px(FontSize::XXS))
                .child("Keyframes"),
        )
}

fn ruler_strip() -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .w_full()
        .h(px(RULER_H))
        .bg(Background::RAISED)
        .px(px(4.0))
        .children((0..5u32).map(|i| {
            div()
                .flex_1()
                .text_color(Text::MUTED)
                .text_size(px(FontSize::XXS))
                .child(format!("{}s", i))
        }))
}

fn clip_strip(name: &str, hue: f32) -> impl IntoElement {
    div()
        .w_full()
        .h(px(STRIP_H))
        .bg(gpui::Hsla {
            h: hue,
            s: 0.55,
            l: 0.40,
            a: 0.50,
        })
        .flex()
        .items_center()
        .px(px(Spacing::SM))
        .child(
            div()
                .text_color(gpui::Hsla {
                    h: 0.0,
                    s: 0.0,
                    l: 1.0,
                    a: 0.95,
                })
                .text_size(px(FontSize::XXS))
                .child(name.to_string()),
        )
}

fn lane_row(lane: &KeyframeLane, hue: f32) -> impl IntoElement {
    let fracs = lane.frame_fractions.clone();
    let label = lane.property_label;
    let diamond_color = gpui::Hsla {
        h: hue,
        s: 0.60,
        l: 0.60,
        a: 1.0,
    };

    let mut track = div()
        .flex()
        .flex_row()
        .items_center()
        .flex_1()
        .h_full()
        .relative();

    for frac in &fracs {
        track = track.child(
            div()
                .absolute()
                .left(relative(*frac))
                .top(px((ROW_H - 8.0) / 2.0))
                .text_color(diamond_color)
                .text_size(px(8.0))
                .child("◇"),
        );
    }

    div()
        .flex()
        .flex_row()
        .items_center()
        .w_full()
        .h(px(ROW_H))
        .border_b_1()
        .border_color(BorderColors::SUBTLE)
        .bg(gpui::Hsla {
            h: 0.0,
            s: 0.0,
            l: 1.0,
            a: 0.03,
        })
        .child(
            div()
                .w(px(60.0))
                .px(px(Spacing::XS))
                .text_color(Text::TERTIARY)
                .text_size(px(FontSize::XXS))
                .flex_shrink_0()
                .child(label.to_string()),
        )
        .child(track)
}

impl Render for KeyframesView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let hue = self.state.clip_hue;
        let name = self.state.clip_name.clone();
        let playhead_frac = self.state.playhead_fraction;
        let lanes = self.state.lanes.clone();
        let panel_h = NAV_H + RULER_H + STRIP_H + (lanes.len() as f32 * ROW_H);

        div()
            .id("keyframes-panel")
            .track_focus(&self.focus_handle.clone())
            .flex()
            .flex_col()
            .w_full()
            .h(px(panel_h))
            .relative()
            .bg(Background::SURFACE)
            .child(nav_toolbar())
            .child(ruler_strip())
            .child(clip_strip(&name, hue))
            .children(lanes.iter().map(|lane| lane_row(lane, hue)))
            // Playhead: thin red vertical line
            .child(
                div()
                    .absolute()
                    .top(px(0.0))
                    .left(relative(playhead_frac))
                    .w(px(1.5))
                    .h(px(panel_h))
                    .bg(gpui::Hsla {
                        h: 0.0,
                        s: 0.95,
                        l: 0.55,
                        a: 1.0,
                    }),
            )
    }
}

//! Project Settings Mismatch dialog — matches Swift ProjectSettingsMismatchView.
//!
//! Shown when a clip with different FPS/resolution is added to the timeline.
//! Width: 360px. Two action buttons: "Keep Current" / "Change to Match".

use crate::theme::{Accent, Background, BorderColors, FontSize, Radius, Spacing, Text};
use gpui::{
    div, prelude::*, px, App, Context, FocusHandle, Focusable, ParentElement, Render, Styled,
    Window,
};

/// Settings mismatch data.
#[derive(Debug, Clone)]
pub struct SettingsMismatch {
    pub project_fps: u32,
    pub clip_fps: u32,
    pub project_width: u32,
    pub project_height: u32,
    pub clip_width: u32,
    pub clip_height: u32,
}

/// Settings mismatch dialog view.
pub struct SettingsMismatchView {
    pub mismatch: SettingsMismatch,
    focus_handle: FocusHandle,
}

impl SettingsMismatchView {
    pub fn new(mismatch: SettingsMismatch, cx: &mut Context<Self>) -> Self {
        Self {
            mismatch,
            focus_handle: cx.focus_handle(),
        }
    }
}

impl Focusable for SettingsMismatchView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

fn grid_row(label: &str, project_val: &str, clip_val: &str, mismatch: bool) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(Spacing::XL))
        .child(
            div()
                .w(px(80.0))
                .text_color(Text::SECONDARY)
                .text_size(px(FontSize::SM))
                .child(label.to_string()),
        )
        .child(
            div()
                .w(px(80.0))
                .text_color(Text::PRIMARY)
                .text_size(px(FontSize::SM))
                .child(project_val.to_string()),
        )
        .child(
            div()
                .text_color(if mismatch {
                    // orange for mismatch
                    gpui::Hsla {
                        h: 35.0 / 360.0,
                        s: 0.90,
                        l: 0.55,
                        a: 1.0,
                    }
                } else {
                    Text::PRIMARY
                })
                .text_size(px(FontSize::SM))
                .child(clip_val.to_string()),
        )
}

impl Render for SettingsMismatchView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let m = &self.mismatch;
        let fps_mismatch = m.clip_fps != m.project_fps;
        let res_mismatch = m.clip_width != m.project_width || m.clip_height != m.project_height;

        div()
            .id("settings-mismatch")
            .track_focus(&self.focus_handle.clone())
            .flex()
            .flex_col()
            .items_center()
            .gap(px(Spacing::XL))
            .w(px(360.0))
            .bg(Background::RAISED)
            .rounded(px(Radius::LG))
            .border_1()
            .border_color(BorderColors::PRIMARY)
            .p(px(Spacing::XL + Spacing::MD))
            // Title
            .child(
                div()
                    .text_color(Text::PRIMARY)
                    .text_size(px(FontSize::XL))
                    .child("Clip Settings Mismatch"),
            )
            // Subtitle
            .child(
                div()
                    .text_color(Text::SECONDARY)
                    .text_size(px(FontSize::SM))
                    .text_align(gpui::TextAlign::Center)
                    .child(
                        "The clip you're adding has different settings than the current project.",
                    ),
            )
            // Comparison grid
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(Spacing::SM))
                    .w_full()
                    // Header row
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap(px(Spacing::XL))
                            .child(div().w(px(80.0)))
                            .child(
                                div()
                                    .w(px(80.0))
                                    .text_color(Text::TERTIARY)
                                    .text_size(px(FontSize::XS))
                                    .child("Project"),
                            )
                            .child(
                                div()
                                    .text_color(Text::TERTIARY)
                                    .text_size(px(FontSize::XS))
                                    .child("Clip"),
                            ),
                    )
                    .child(grid_row(
                        "FPS",
                        &m.project_fps.to_string(),
                        &m.clip_fps.to_string(),
                        fps_mismatch,
                    ))
                    .child(grid_row(
                        "Resolution",
                        &format!("{} x {}", m.project_width, m.project_height),
                        &format!("{} x {}", m.clip_width, m.clip_height),
                        res_mismatch,
                    )),
            )
            // Action buttons
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(Spacing::MD))
                    // Keep Current (secondary capsule)
                    .child(
                        div()
                            .id("btn-keep-current")
                            .px(px(Spacing::LG))
                            .py(px(Spacing::SM))
                            .rounded_full()
                            .border_1()
                            .border_color(BorderColors::PRIMARY)
                            .cursor_pointer()
                            .text_color(Text::SECONDARY)
                            .text_size(px(FontSize::SM))
                            .child("Keep Current"),
                    )
                    // Change to Match (prominent capsule)
                    .child(
                        div()
                            .id("btn-change-to-match")
                            .px(px(Spacing::LG))
                            .py(px(Spacing::SM))
                            .rounded_full()
                            .bg(Accent::PRIMARY)
                            .cursor_pointer()
                            .text_color(Background::BASE)
                            .text_size(px(FontSize::SM))
                            .child("Change to Match"),
                    ),
            )
    }
}

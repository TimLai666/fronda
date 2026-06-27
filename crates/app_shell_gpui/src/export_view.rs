//! Export panel view — matches Swift ExportView layout (Issue #166).
//!
//! Layout: 860×560 sheet
//!   ├── HStack
//!   │   ├── settingsPanel  (360px wide, left)
//!   │   └── previewPanel  (flex, right)
//!   └── bottomBar (48px, footer)

#![cfg(feature = "desktop-app")]

use gpui::*;

use crate::export_model::{ExportMode, ExportViewModel};
use crate::theme::{Background, BorderColors, Spacing, Text};

/// Export sheet view.
pub struct ExportView {
    pub model: ExportViewModel,
}

impl ExportView {
    pub fn new() -> Self {
        Self {
            model: ExportViewModel::new(),
        }
    }
}

impl Render for ExportView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let stage = self.model.panel.stage.clone();
        let can_start = self.model.can_start_export();
        let mode = self.model.mode;

        div()
            .flex()
            .flex_col()
            .w(px(860.0))
            .h(px(560.0))
            .bg(Background::RAISED)
            // ── body row ──────────────────────────────────────────────────
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    // Settings panel (left, 360px)
                    .child(
                        div()
                            .w(px(360.0))
                            .h_full()
                            .flex()
                            .flex_col()
                            .border_r_1()
                            .border_color(BorderColors::PRIMARY)
                            .child(
                                // Panel header
                                div()
                                    .px(Spacing::XL)
                                    .py(Spacing::MD)
                                    .border_b_1()
                                    .border_color(BorderColors::PRIMARY)
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(Text::PRIMARY)
                                            .child("Export"),
                                    ),
                            )
                            .child(
                                // Mode picker
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(Spacing::XS)
                                    .px(Spacing::LG)
                                    .py(Spacing::MD)
                                    .children(ExportMode::all().iter().map(|m| {
                                        let selected = *m == mode;
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(Spacing::SM)
                                            .px(Spacing::SM)
                                            .py(Spacing::XS)
                                            .rounded(px(4.0))
                                            .when(selected, |el| el.bg(Background::SELECTED))
                                            .cursor_pointer()
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(if selected {
                                                        Text::PRIMARY
                                                    } else {
                                                        Text::SECONDARY
                                                    })
                                                    .child(m.label()),
                                            )
                                    })),
                            ),
                    )
                    // Preview panel (right, flex)
                    .child(
                        div()
                            .flex_1()
                            .h_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(Background::BASE)
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(Text::MUTED)
                                    .child("Preview"),
                            ),
                    ),
            )
            // ── bottom bar ───────────────────────────────────────────────
            .child(
                div()
                    .h(px(48.0))
                    .flex()
                    .items_center()
                    .justify_end()
                    .px(Spacing::LG)
                    .gap(Spacing::MD)
                    .border_t_1()
                    .border_color(BorderColors::PRIMARY)
                    .bg(Background::RAISED)
                    // Cancel button
                    .child(
                        div()
                            .px(Spacing::MD)
                            .py(Spacing::XS)
                            .rounded(px(4.0))
                            .border_1()
                            .border_color(BorderColors::SECONDARY)
                            .cursor_pointer()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(Text::SECONDARY)
                                    .child("Cancel"),
                            ),
                    )
                    // Export button
                    .child(
                        div()
                            .px(Spacing::MD)
                            .py(Spacing::XS)
                            .rounded(px(4.0))
                            .bg(if can_start {
                                Background::ACCENT
                            } else {
                                Background::OVERLAY
                            })
                            .cursor_pointer()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(Text::PRIMARY)
                                    .child("Export"),
                            ),
                    ),
            )
    }
}

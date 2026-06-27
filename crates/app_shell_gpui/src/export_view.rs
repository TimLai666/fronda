//! Export panel view — matches Swift ExportView layout (Issue #166).
//!
//! Layout: 860×560 sheet
//!   ├── HStack
//!   │   ├── settingsPanel  (360px wide, left)
//!   │   └── previewPanel  (flex, right)
//!   └── bottomBar (48px, footer)

#![cfg(feature = "desktop-app")]

use gpui::*;
use gpui::prelude::*;

use crate::export_model::{ExportMode, ExportViewModel};
use crate::theme::{Accent, Background, BorderColors, FontSize, Radius, Spacing, Text};

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
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
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
                                    .px(px(Spacing::XL))
                                    .py(px(Spacing::MD))
                                    .border_b_1()
                                    .border_color(BorderColors::PRIMARY)
                                    .child(
                                        div()
                                            .text_size(px(FontSize::SM_MD))
                                            .text_color(Text::PRIMARY)
                                            .child("Export"),
                                    ),
                            )
                            .child(
                                // Mode picker
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(Spacing::XS))
                                    .px(px(Spacing::LG))
                                    .py(px(Spacing::MD))
                                    .children(ExportMode::all().iter().map(|m| {
                                        let selected = *m == mode;
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(Spacing::SM))
                                            .px(px(Spacing::SM))
                                            .py(px(Spacing::XS))
                                            .rounded(px(Radius::XS_SM))
                                            .when(selected, |el| el.bg(BorderColors::PRIMARY))
                                            .cursor_pointer()
                                            .child(
                                                div()
                                                    .text_size(px(FontSize::SM))
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
                                    .text_size(px(FontSize::SM))
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
                    .px(px(Spacing::LG))
                    .gap(px(Spacing::MD))
                    .border_t_1()
                    .border_color(BorderColors::PRIMARY)
                    .bg(Background::RAISED)
                    // Cancel button
                    .child(
                        div()
                            .px(px(Spacing::MD))
                            .py(px(Spacing::XS))
                            .rounded(px(Radius::XS_SM))
                            .border_1()
                            .border_color(BorderColors::PRIMARY)
                            .cursor_pointer()
                            .child(
                                div()
                                    .text_size(px(FontSize::SM))
                                    .text_color(Text::SECONDARY)
                                    .child("Cancel"),
                            ),
                    )
                    // Export button
                    .child(
                        div()
                            .px(px(Spacing::MD))
                            .py(px(Spacing::XS))
                            .rounded(px(Radius::XS_SM))
                            .bg(if can_start {
                                Accent::PRIMARY
                            } else {
                                Background::PROMINENT
                            })
                            .cursor_pointer()
                            .child(
                                div()
                                    .text_size(px(FontSize::SM))
                                    .text_color(if can_start {
                                        Background::BASE
                                    } else {
                                        Text::MUTED
                                    })
                                    .child("Export"),
                            ),
                    ),
            )
    }
}

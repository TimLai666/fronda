//! Project Activity view — matches Swift ProjectActivityView.
//!
//! Shows AI generation history for the current project.
//! Width: 340px. Shown as a popover/panel.

use crate::theme::{Background, BorderColors, FontSize, Radius, Spacing, Text};
use gpui::{
    div, prelude::*, px, App, Context, FocusHandle, Focusable, IntoElement, ParentElement, Render,
    Styled, Window,
};

/// A single AI generation log entry.
#[derive(Debug, Clone)]
pub struct ActivityEntry {
    pub icon: &'static str,
    pub cost_label: String,
    pub model_name: String,
    pub relative_time: String,
}

/// State for the project activity panel.
#[derive(Debug, Clone, Default)]
pub struct ProjectActivityState {
    pub entries: Vec<ActivityEntry>,
    pub total_cost_label: String,
}

/// Project activity view entity.
pub struct ProjectActivityView {
    pub state: ProjectActivityState,
    focus_handle: FocusHandle,
}

impl ProjectActivityView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            state: ProjectActivityState::default(),
            focus_handle: cx.focus_handle(),
        }
    }
}

impl Focusable for ProjectActivityView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

fn entry_row(entry: &ActivityEntry) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(Spacing::SM))
        .w_full()
        .py(px(Spacing::XS))
        .px(px(Spacing::XXS))
        // Icon (xs, tertiary)
        .child(
            div()
                .w(px(14.0))
                .text_color(Text::TERTIARY)
                .text_size(px(FontSize::XS))
                .child(entry.icon.to_string()),
        )
        // Cost (68px, xs medium, secondary)
        .child(
            div()
                .w(px(68.0))
                .text_color(Text::SECONDARY)
                .text_size(px(FontSize::XS))
                .child(entry.cost_label.clone()),
        )
        // Model name (flex 1, truncated)
        .child(
            div()
                .flex_1()
                .text_color(Text::SECONDARY)
                .text_size(px(FontSize::XS))
                .child(entry.model_name.clone()),
        )
        // Relative time (muted, xs)
        .child(
            div()
                .text_color(Text::MUTED)
                .text_size(px(FontSize::XS))
                .child(entry.relative_time.clone()),
        )
}

impl Render for ProjectActivityView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let is_empty = self.state.entries.is_empty();
        let total_label = self.state.total_cost_label.clone();
        let entries: Vec<ActivityEntry> = self.state.entries.clone();

        div()
            .id("project-activity")
            .track_focus(&self.focus_handle.clone())
            .flex()
            .flex_col()
            .w(px(340.0))
            .bg(Background::RAISED)
            .rounded(px(Radius::MD_LG))
            .border_1()
            .border_color(BorderColors::PRIMARY)
            .p(px(Spacing::MD))
            .gap(px(Spacing::SM))
            // Header row
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .w_full()
                    .child(
                        div()
                            .flex_1()
                            .text_color(Text::PRIMARY)
                            .text_size(px(FontSize::SM))
                            .child("Project Activity"),
                    )
                    .when(!is_empty, |el| {
                        el.child(
                            div()
                                .text_color(Text::TERTIARY)
                                .text_size(px(FontSize::XS))
                                .child(format!("{} used", total_label)),
                        )
                    }),
            )
            // Content: empty state or entry list
            .child(if is_empty {
                div()
                    .text_color(Text::MUTED)
                    .text_size(px(FontSize::XS))
                    .py(px(Spacing::SM))
                    .child("No generations yet")
                    .into_any_element()
            } else {
                div()
                    .id("project-activity-scroll")
                    .flex()
                    .flex_col()
                    .gap(px(Spacing::XXS))
                    .max_h(px(420.0))
                    .overflow_y_scroll()
                    .children(entries.iter().map(|e| entry_row(e)))
                    .into_any_element()
            })
    }
}

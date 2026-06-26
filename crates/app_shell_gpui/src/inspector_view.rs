/// Inspector panel gpui view — tab bar + collapsible sections.
///
/// Matches InspectorView.swift layout.

use crate::inspector_model::{InspectorState, InspectorTab};
use crate::theme::{Background, BorderColors, FontSize, Layout, Radius, Spacing, Text};
use gpui::{
    div, prelude::*, px, App, Context, FocusHandle, Focusable, IntoElement, InteractiveElement,
    ParentElement, Render, Styled, Window,
};

pub struct InspectorView {
    pub state: InspectorState,
    focus_handle: FocusHandle,
}

impl InspectorView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            state: InspectorState::new(),
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn select_tab(&mut self, tab: InspectorTab, cx: &mut Context<Self>) {
        self.state.select_tab(tab);
        cx.notify();
    }

    pub fn toggle_transform(&mut self, cx: &mut Context<Self>) {
        self.state.toggle_transform();
        cx.notify();
    }

    pub fn toggle_volume(&mut self, cx: &mut Context<Self>) {
        self.state.toggle_volume();
        cx.notify();
    }
}

impl Focusable for InspectorView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for InspectorView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active_tab = self.state.active_tab.clone();
        let transform_expanded = self.state.transform_expanded;
        let volume_expanded = self.state.volume_expanded;

        div()
            .id("inspector-panel")
            .flex()
            .flex_col()
            .size_full()
            .bg(Background::SURFACE)
            // ── Panel header ──
            .child(
                div()
                    .id("inspector-header")
                    .flex()
                    .flex_row()
                    .items_center()
                    .w_full()
                    .h(px(Layout::PANEL_HEADER_HEIGHT))
                    .px(px(Spacing::MD))
                    .bg(Background::RAISED)
                    .border_b_1()
                    .border_color(BorderColors::PRIMARY)
                    .child(
                        div()
                            .text_color(Text::SECONDARY)
                            .text_size(px(FontSize::SM))
                            .child("Inspector"),
                    ),
            )
            // ── Tab bar ──
            .child(
                div()
                    .id("inspector-tabs")
                    .flex()
                    .flex_row()
                    .items_center()
                    .w_full()
                    .px(px(Spacing::SM))
                    .py(px(Spacing::XS))
                    .gap(px(Spacing::XS))
                    .bg(Background::SURFACE)
                    .border_b_1()
                    .border_color(BorderColors::SUBTLE)
                    .children(InspectorTab::all_tabs().iter().map(|tab| {
                        let is_active = *tab == active_tab;
                        let tab_clone = tab.clone();
                        div()
                            .id(tab.label())
                            .px(px(Spacing::SM))
                            .py(px(Spacing::XXS))
                            .rounded(px(Radius::XS_SM))
                            .cursor_pointer()
                            .bg(if is_active { BorderColors::PRIMARY } else { Background::SURFACE })
                            .text_color(if is_active { Text::PRIMARY } else { Text::MUTED })
                            .text_size(px(FontSize::XS))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.select_tab(tab_clone.clone(), cx);
                            }))
                            .child(tab.label())
                    })),
            )
            // ── Tab content area ──
            .child(
                div()
                    .id("inspector-content")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .w_full()
                    .overflow_hidden()
                    // Tab label placeholder
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .px(px(Spacing::MD))
                            .py(px(Spacing::SM))
                            .text_color(Text::MUTED)
                            .text_size(px(FontSize::SM))
                            .child(active_tab.label()),
                    )
                    // Volume section
                    .child(
                        div()
                            .id("inspector-volume-section")
                            .flex()
                            .flex_col()
                            .w_full()
                            .border_t_1()
                            .border_color(BorderColors::SUBTLE)
                            // Collapsible header
                            .child(
                                div()
                                    .id("inspector-volume-header")
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .w_full()
                                    .px(px(Spacing::MD))
                                    .h(px(Spacing::XXL))
                                    .cursor_pointer()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.toggle_volume(cx);
                                    }))
                                    .child(
                                        div()
                                            .text_color(Text::SECONDARY)
                                            .text_size(px(FontSize::SM))
                                            .child(if volume_expanded { "▾ Volume" } else { "▸ Volume" }),
                                    ),
                            )
                            // Volume slider placeholder (visible when expanded)
                            .when(volume_expanded, |el| {
                                el.child(
                                    div()
                                        .id("inspector-volume-content")
                                        .flex()
                                        .items_center()
                                        .px(px(Spacing::MD))
                                        .py(px(Spacing::SM))
                                        .child(
                                            div()
                                                .w_full()
                                                .h(px(Spacing::XS))
                                                .rounded(px(Radius::XS))
                                                .bg(BorderColors::PRIMARY),
                                        ),
                                )
                            }),
                    )
                    // Transform section
                    .child(
                        div()
                            .id("inspector-transform-section")
                            .flex()
                            .flex_col()
                            .w_full()
                            .border_t_1()
                            .border_color(BorderColors::SUBTLE)
                            // Collapsible header
                            .child(
                                div()
                                    .id("inspector-transform-header")
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .w_full()
                                    .px(px(Spacing::MD))
                                    .h(px(Spacing::XXL))
                                    .cursor_pointer()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.toggle_transform(cx);
                                    }))
                                    .child(
                                        div()
                                            .text_color(Text::SECONDARY)
                                            .text_size(px(FontSize::SM))
                                            .child(if transform_expanded { "▾ Transform" } else { "▸ Transform" }),
                                    ),
                            )
                            // Transform content placeholder (visible when expanded)
                            .when(transform_expanded, |el| {
                                el.child(
                                    div()
                                        .id("inspector-transform-content")
                                        .px(px(Spacing::MD))
                                        .py(px(Spacing::SM))
                                        .child(
                                            div()
                                                .text_color(Text::MUTED)
                                                .text_size(px(FontSize::XS))
                                                .child("Position · Scale · Rotation"),
                                        ),
                                )
                            }),
                    ),
            )
    }
}

/// Inspector panel gpui view — tab bar + collapsible sections.
///
/// Matches InspectorView.swift layout with real property rows.

use crate::inspector_model::{InspectorState, InspectorTab};
use crate::theme::{
    Accent, Background, BorderColors, FontSize, Layout, Radius, Spacing, Text,
};
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

/// A label/value property row matching Swift inspector rows.
fn prop_row(label: &str, value: &str) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .w_full()
        .px(px(Spacing::LG))
        .h(px(22.0))
        .child(
            div()
                .flex_1()
                .text_color(Text::SECONDARY)
                .text_size(px(FontSize::XS))
                .child(label.to_string()),
        )
        .child(
            div()
                .text_color(Text::TERTIARY)
                .text_size(px(FontSize::XS))
                .child(value.to_string()),
        )
}

/// Collapsible section header: uppercase xxs/muted text + expand chevron.
fn section_header(label: &str, expanded: bool) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .w_full()
        .px(px(Spacing::LG))
        .h(px(28.0))
        .border_t_1()
        .border_color(BorderColors::SUBTLE)
        .cursor_pointer()
        .child(
            div()
                .flex_1()
                .text_color(Text::MUTED)
                .text_size(px(FontSize::XXS))
                .child(label.to_uppercase()),
        )
        .child(
            div()
                .text_color(Text::MUTED)
                .text_size(px(FontSize::XS))
                .child(if expanded { "▾" } else { "▸" }),
        )
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
            // ── Tab bar: underline style, not fill ──
            .child(
                div()
                    .id("inspector-tabs")
                    .flex()
                    .flex_row()
                    .items_end()
                    .w_full()
                    .px(px(Spacing::LG))
                    .pt(px(Spacing::XS))
                    .gap(px(Spacing::MD_LG))
                    .bg(Background::SURFACE)
                    .border_b_1()
                    .border_color(BorderColors::SUBTLE)
                    .children(InspectorTab::all_tabs().iter().map(|tab| {
                        let is_active = *tab == active_tab;
                        let tab_clone = tab.clone();
                        div()
                            .id(tab.label())
                            .pb(px(Spacing::XS))
                            .cursor_pointer()
                            .text_color(if is_active { Text::PRIMARY } else { Text::MUTED })
                            .text_size(px(FontSize::XS))
                            .border_b(px(if is_active { 1.5 } else { 0.0 }))
                            .border_color(Text::PRIMARY)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.select_tab(tab_clone.clone(), cx);
                            }))
                            .child(tab.label())
                    })),
            )
            // ── Scrollable content ──
            .child(
                div()
                    .id("inspector-content")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .w_full()
                    .overflow_y_scroll()
                    // Volume section
                    .child(
                        div()
                            .id("section-volume")
                            .flex()
                            .flex_col()
                            .w_full()
                            .child(
                                div()
                                    .w_full()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.toggle_volume(cx);
                                    }))
                                    .child(section_header("Volume", volume_expanded)),
                            )
                            .when(volume_expanded, |el| {
                                el.child(prop_row("Volume", "100%"))
                                    .child(prop_row("Fade In", "0.0s"))
                                    .child(prop_row("Fade Out", "0.0s"))
                            }),
                    )
                    // Transform section
                    .child(
                        div()
                            .id("section-transform")
                            .flex()
                            .flex_col()
                            .w_full()
                            .child(
                                div()
                                    .w_full()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.toggle_transform(cx);
                                    }))
                                    .child(section_header("Transform", transform_expanded)),
                            )
                            .when(transform_expanded, |el| {
                                el.child(prop_row("Position X", "0.0"))
                                    .child(prop_row("Position Y", "0.0"))
                                    .child(prop_row("Scale", "100%"))
                                    .child(prop_row("Rotation", "0°"))
                                    .child(prop_row("Opacity", "100%"))
                            }),
                    )
                    // Blending section
                    .child(
                        div()
                            .id("section-blend")
                            .flex()
                            .flex_col()
                            .w_full()
                            .child(section_header("Blending", false)),
                    )
                    // Speed section
                    .child(
                        div()
                            .id("section-speed")
                            .flex()
                            .flex_col()
                            .w_full()
                            .child(section_header("Speed", false)),
                    )
                    // Project metadata section
                    .child(
                        div()
                            .id("section-project")
                            .flex()
                            .flex_col()
                            .w_full()
                            .child(section_header("Project", true))
                            .child(prop_row("Name", "Untitled"))
                            .child(prop_row("Resolution", "1920×1080"))
                            .child(prop_row("Frame Rate", "30 fps"))
                            .child(prop_row("Duration", "0:20")),
                    )
                    // Format section
                    .child(
                        div()
                            .id("section-format")
                            .flex()
                            .flex_col()
                            .w_full()
                            .child(section_header("Format", true))
                            .child(prop_row("Aspect Ratio", "16:9"))
                            .child(prop_row("Color Space", "sRGB")),
                    )
                    // AI Edit footer badge
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_end()
                            .w_full()
                            .px(px(Spacing::LG))
                            .py(px(Spacing::MD))
                            .child(
                                div()
                                    .px(px(Spacing::SM))
                                    .py(px(Spacing::XXS))
                                    .rounded(px(Radius::XS_SM))
                                    .border_1()
                                    .border_color(BorderColors::SUBTLE)
                                    .text_size(px(FontSize::XXS))
                                    .text_color(Accent::PRIMARY)
                                    .child("AI EDIT"),
                            ),
                    ),
            )
    }
}

/// Inspector panel gpui view — matches Swift InspectorView.swift exactly.
///
/// Two display modes:
///   • no_clip (default when editor opens): Project + Format metadata rows, no tab bar
///   • clip_selected: tab bar (Text / Video / Audio / AI Edit) + tab content
///
/// Tab labels: active = SM medium + PRIMARY + underline; inactive = SM regular + TERTIARY.
/// AI Edit tab uses Accent::PRIMARY instead of PRIMARY (Swift uses aiGradient).
/// Prop rows: label XS TERTIARY left, value XS SECONDARY right.

use crate::inspector_model::{InspectorState, InspectorTab};
use crate::theme::{
    Accent, Background, BorderColors, FontSize, Layout, Spacing, Text,
};
use gpui::{
    div, prelude::*, px, App, Context, FocusHandle, Focusable, IntoElement, InteractiveElement,
    ParentElement, Render, Styled, Window,
};

pub struct InspectorView {
    pub state: InspectorState,
    /// When true, render clip-inspector (tabs). When false, render project metadata.
    pub has_clip_selected: bool,
    focus_handle: FocusHandle,
}

impl InspectorView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            state: InspectorState::new(),
            has_clip_selected: false,
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

/// Label/value row — Swift plainMetadataRow: label XS TERTIARY left, value XS SECONDARY right.
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
                .text_color(Text::TERTIARY)
                .text_size(px(FontSize::XS))
                .child(label.to_string()),
        )
        .child(
            div()
                .text_color(Text::SECONDARY)
                .text_size(px(FontSize::XS))
                .child(value.to_string()),
        )
}

/// Section header: uppercase XXS MUTED label + chevron. Matches Swift metadataSection titles.
fn section_header(label: &str, expanded: bool) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .w_full()
        .px(px(Spacing::LG))
        .h(px(28.0))
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

/// "No clip selected" content: Project (Name, Path) + Format (Resolution, fps, ratio, duration).
/// Matches Swift InspectorView.projectMetadataContent.
fn project_metadata_content() -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .w_full()
        // Project section
        .child(
            div()
                .flex()
                .flex_col()
                .w_full()
                .pt(px(Spacing::MD))
                .gap(px(Spacing::XXS))
                .child(section_header("Project", true))
                .child(prop_row("Name", "Untitled"))
                .child(prop_row("Path", "~/Movies/Untitled.palmier")),
        )
        // Format section
        .child(
            div()
                .flex()
                .flex_col()
                .w_full()
                .pt(px(Spacing::SM))
                .gap(px(Spacing::XXS))
                .child(section_header("Format", true))
                .child(prop_row("Resolution", "1920 × 1080"))
                .child(prop_row("Frame Rate", "30 fps"))
                .child(prop_row("Aspect Ratio", "16:9"))
                .child(prop_row("Duration", "0:20")),
        )
}

/// Volume + Fade rows — used in Video and Audio tabs.
fn levels_section(volume_expanded: bool) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .w_full()
        .child(section_header("Levels", volume_expanded))
        .when(volume_expanded, |el| {
            el.child(prop_row("Volume", "100%"))
                .child(prop_row("Fade In", "0.0 s"))
                .child(prop_row("Fade Out", "0.0 s"))
        })
}

/// Transform rows — used in Video tab.
fn transform_section(transform_expanded: bool) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .w_full()
        .child(section_header("Transform", transform_expanded))
        .when(transform_expanded, |el| {
            el.child(prop_row("Position", "0, 0"))
                .child(prop_row("Scale", "100%"))
                .child(prop_row("Rotation", "0°"))
                .child(prop_row("Opacity", "100%"))
                .child(prop_row("Crop", "None"))
                .child(prop_row("Flip", "None"))
        })
}

/// Speed row — used in Video and Audio tabs.
fn speed_section() -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .w_full()
        .child(section_header("Playback", true))
        .child(prop_row("Speed", "100%"))
}

/// Keyframes footer toggle — always at bottom of Video/Audio tabs.
fn keyframes_toggle_bar() -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .justify_end()
        .w_full()
        .px(px(Spacing::LG))
        .py(px(Spacing::XS))
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(Spacing::XS))
                .px(px(Spacing::SM_MD))
                .py(px(Spacing::XS))
                .text_color(Text::TERTIARY)
                .text_size(px(FontSize::XS))
                .cursor_pointer()
                .child("◇ Keyframes"),
        )
}

/// Video tab content: Transform + Speed + Keyframes toggle.
fn video_tab_content(transform_expanded: bool, volume_expanded: bool) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .w_full()
        .child(levels_section(volume_expanded))
        .child(transform_section(transform_expanded))
        .child(speed_section())
        .child(keyframes_toggle_bar())
}

/// Audio tab content: Levels + Speed.
fn audio_tab_content(volume_expanded: bool) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .w_full()
        .child(levels_section(volume_expanded))
        .child(speed_section())
        .child(keyframes_toggle_bar())
}

/// Text tab placeholder.
fn text_tab_content() -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .w_full()
        .pt(px(Spacing::MD))
        .child(section_header("Content", true))
        .child(prop_row("Text", "Title"))
        .child(prop_row("Font", "System"))
        .child(prop_row("Size", "48"))
        .child(prop_row("Color", "White"))
        .child(prop_row("Alignment", "Center"))
}

/// AI Edit tab placeholder.
fn ai_edit_tab_content() -> impl IntoElement {
    div()
        .flex()
        .flex_1()
        .items_center()
        .justify_center()
        .text_color(Text::MUTED)
        .text_size(px(FontSize::SM))
        .child("Select a clip to use AI Edit")
}

impl Render for InspectorView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active_tab = self.state.active_tab.clone();
        let transform_expanded = self.state.transform_expanded;
        let volume_expanded = self.state.volume_expanded;
        let has_clip = self.has_clip_selected;

        let header_title = if has_clip { "Inspector" } else { "Timeline" };

        div()
            .id("inspector-panel")
            .flex()
            .flex_col()
            .size_full()
            .bg(Background::SURFACE)
            // ── Panel header: icon + title ──
            .child(
                div()
                    .id("inspector-header")
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(Spacing::XS))
                    .w_full()
                    .h(px(Layout::PANEL_HEADER_HEIGHT))
                    .px(px(Spacing::LG))
                    .bg(Background::RAISED)
                    .border_b_1()
                    .border_color(BorderColors::PRIMARY)
                    .child(
                        div()
                            .text_color(Text::TERTIARY)
                            .text_size(px(FontSize::XS))
                            .child(if has_clip { "⚙" } else { "ℹ" }),
                    )
                    .child(
                        div()
                            .text_color(Text::SECONDARY)
                            .text_size(px(FontSize::SM))
                            .child(header_title),
                    ),
            )
            // ── Conditional body ──
            .child(
                div()
                    .id("inspector-scroll")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .w_full()
                    .overflow_y_scroll()
                    .when(!has_clip, |el| {
                        // No clip: Project + Format metadata only
                        el.child(project_metadata_content())
                    })
                    .when(has_clip, |el| {
                        // Clip selected: tab bar + tab content
                        el
                            // Tab bar (underline style, Swift genericTabBar)
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
                                        let is_ai = *tab == InspectorTab::AiEdit;
                                        let tab_clone = tab.clone();
                                        div()
                                            .id(tab.label())
                                            .pb(px(Spacing::XS))
                                            .cursor_pointer()
                                            .text_color(if is_ai {
                                                Accent::PRIMARY
                                            } else if is_active {
                                                Text::PRIMARY
                                            } else {
                                                Text::TERTIARY
                                            })
                                            .text_size(px(FontSize::SM))
                                            // Underline indicator (Swift: Rectangle height=BorderWidth.medium)
                                            .border_b(px(if is_active { 1.5 } else { 0.0 }))
                                            .border_color(if is_ai {
                                                Accent::PRIMARY
                                            } else {
                                                Text::PRIMARY
                                            })
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.select_tab(tab_clone.clone(), cx);
                                            }))
                                            .child(tab.label())
                                    })),
                            )
                            // Tab content
                            .child(match active_tab {
                                InspectorTab::Video => {
                                    video_tab_content(transform_expanded, volume_expanded)
                                        .into_any_element()
                                }
                                InspectorTab::Audio => {
                                    audio_tab_content(volume_expanded).into_any_element()
                                }
                                InspectorTab::Text => text_tab_content().into_any_element(),
                                InspectorTab::AiEdit => ai_edit_tab_content().into_any_element(),
                            })
                    }),
            )
    }
}

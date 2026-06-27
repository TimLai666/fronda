/// Inspector panel gpui view — matches Swift InspectorView.swift exactly.
///
/// Two display modes:
///   • no_clip: Project + Format metadata rows, no tab bar
///   • clip_selected: tab bar (Text / Video / Audio / AI Edit) + tab content
///
/// AI Edit tab wires to AiEditTabView entity; keyframes toggle shows KeyframesView.

use crate::ai_edit_tab_view::AiEditTabView;
use crate::inspector_model::{InspectorState, InspectorTab};
use crate::keyframes_view::KeyframesView;
use crate::theme::{
    Accent, Background, BorderColors, FontSize, Layout, Spacing, Text,
};
use gpui::{
    div, prelude::*, px, App, Context, Entity, FocusHandle, Focusable, IntoElement,
    InteractiveElement, ParentElement, Render, Styled, Window,
};

pub struct InspectorView {
    pub state: InspectorState,
    pub has_clip_selected: bool,
    ai_edit_view: Entity<AiEditTabView>,
    keyframes_view: Entity<KeyframesView>,
    focus_handle: FocusHandle,
}

impl InspectorView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            state: InspectorState::new(),
            has_clip_selected: false,
            ai_edit_view: cx.new(|cx| AiEditTabView::new(cx)),
            keyframes_view: cx.new(|cx| KeyframesView::new(cx)),
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

    pub fn toggle_keyframes(&mut self, cx: &mut Context<Self>) {
        self.state.toggle_keyframes();
        cx.notify();
    }
}

impl Focusable for InspectorView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

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
                .child(if expanded { "v" } else { ">" }),
        )
}

fn project_metadata_content() -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .w_full()
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
        .child(
            div()
                .flex()
                .flex_col()
                .w_full()
                .pt(px(Spacing::SM))
                .gap(px(Spacing::XXS))
                .child(section_header("Format", true))
                .child(prop_row("Resolution", "1920 x 1080"))
                .child(prop_row("Frame Rate", "30 fps"))
                .child(prop_row("Aspect Ratio", "16:9"))
                .child(prop_row("Duration", "0:20")),
        )
}

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

fn transform_section(transform_expanded: bool) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .w_full()
        .child(section_header("Transform", transform_expanded))
        .when(transform_expanded, |el| {
            el.child(prop_row("Position", "0, 0"))
                .child(prop_row("Scale", "100%"))
                .child(prop_row("Rotation", "0 deg"))
                .child(prop_row("Opacity", "100%"))
                .child(prop_row("Crop", "None"))
                .child(prop_row("Flip", "None"))
        })
}

fn speed_section() -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .w_full()
        .child(section_header("Playback", true))
        .child(prop_row("Speed", "100%"))
}

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

fn keyframes_btn(id: &str, active: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id.to_string())
        .flex()
        .flex_row()
        .items_center()
        .gap(px(Spacing::XS))
        .px(px(Spacing::SM_MD))
        .py(px(Spacing::XS))
        .text_color(if active { Text::PRIMARY } else { Text::TERTIARY })
        .text_size(px(FontSize::XS))
        .cursor_pointer()
        .child("Keyframes")
}

impl Render for InspectorView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active_tab = self.state.active_tab.clone();
        let transform_expanded = self.state.transform_expanded;
        let volume_expanded = self.state.volume_expanded;
        let kf_visible = self.state.keyframes_visible;
        let has_clip = self.has_clip_selected;

        let title = if has_clip { "Inspector" } else { "Timeline" };
        let icon = if has_clip { "G" } else { "i" };

        let ai_edit_entity = self.ai_edit_view.clone();
        let kf_entity = self.keyframes_view.clone();

        div()
            .id("inspector-panel")
            .flex()
            .flex_col()
            .size_full()
            .bg(Background::SURFACE)
            // Header
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
                            .child(icon),
                    )
                    .child(
                        div()
                            .text_color(Text::SECONDARY)
                            .text_size(px(FontSize::SM))
                            .child(title),
                    ),
            )
            // Body
            .child(
                div()
                    .id("inspector-scroll")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .w_full()
                    .overflow_y_scroll()
                    .when(!has_clip, |el| el.child(project_metadata_content()))
                    .when(has_clip, |el| {
                        el
                            // Tab bar
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
                                            .border_b(px(if is_active { 1.5 } else { 0.0 }))
                                            .border_color(if is_ai { Accent::PRIMARY } else { Text::PRIMARY })
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.select_tab(tab_clone.clone(), cx);
                                            }))
                                            .child(tab.label())
                                    })),
                            )
                            // Tab content
                            .child(match active_tab {
                                InspectorTab::Video => {
                                    div()
                                        .flex()
                                        .flex_col()
                                        .w_full()
                                        .child(levels_section(volume_expanded))
                                        .child(transform_section(transform_expanded))
                                        .child(speed_section())
                                        .child(
                                            div()
                                                .flex()
                                                .flex_row()
                                                .justify_end()
                                                .w_full()
                                                .px(px(Spacing::LG))
                                                .py(px(Spacing::XS))
                                                .child(
                                                    keyframes_btn("kf-toggle-video", kf_visible)
                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                            this.toggle_keyframes(cx);
                                                        })),
                                                ),
                                        )
                                        .when(kf_visible, |el| {
                                            el.child(
                                                div()
                                                    .w_full()
                                                    .border_t_1()
                                                    .border_color(BorderColors::SUBTLE)
                                                    .child(kf_entity.clone()),
                                            )
                                        })
                                        .into_any_element()
                                }
                                InspectorTab::Audio => {
                                    div()
                                        .flex()
                                        .flex_col()
                                        .w_full()
                                        .child(levels_section(volume_expanded))
                                        .child(speed_section())
                                        .child(
                                            div()
                                                .flex()
                                                .flex_row()
                                                .justify_end()
                                                .w_full()
                                                .px(px(Spacing::LG))
                                                .py(px(Spacing::XS))
                                                .child(
                                                    keyframes_btn("kf-toggle-audio", kf_visible)
                                                        .on_click(cx.listener(|this, _, _, cx| {
                                                            this.toggle_keyframes(cx);
                                                        })),
                                                ),
                                        )
                                        .when(kf_visible, |el| {
                                            el.child(
                                                div()
                                                    .w_full()
                                                    .border_t_1()
                                                    .border_color(BorderColors::SUBTLE)
                                                    .child(kf_entity.clone()),
                                            )
                                        })
                                        .into_any_element()
                                }
                                InspectorTab::Text => text_tab_content().into_any_element(),
                                InspectorTab::AiEdit => ai_edit_entity.clone().into_any_element(),
                            })
                    }),
            )
    }
}

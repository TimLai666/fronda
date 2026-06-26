//! Media panel gpui view — left tab rail + content area.
//!
//! Covers UIX-011 (panel widths), THM-017 (tab rail width formula),
//! and the MediaPanelView from 07-ui-port-spec.md.

use crate::media_panel_model::{MediaPanelState, MediaPanelTab};
use crate::theme::{Background, BorderColors, IconSize, MediaPanel, Radius, Spacing, Text};
use gpui::{
    div, prelude::*, px, App, Context, FocusHandle, Focusable, IntoElement, InteractiveElement,
    ParentElement, Render, Styled, Window,
};

/// Media panel gpui entity.
pub struct MediaPanelView {
    pub state: MediaPanelState,
    focus_handle: FocusHandle,
}

impl MediaPanelView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            state: MediaPanelState::new(),
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn select_tab(&mut self, tab: MediaPanelTab, cx: &mut Context<Self>) {
        self.state.select_tab(tab);
        cx.notify();
    }
}

impl Focusable for MediaPanelView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for MediaPanelView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active = self.state.active_tab.clone();
        let media_active = active == MediaPanelTab::Media;
        let captions_active = active == MediaPanelTab::Captions;
        let music_active = active == MediaPanelTab::Music;

        let btn_size = IconSize::MD_LG; // 24px icon button size

        div()
            .id("media-panel")
            .flex()
            .flex_row()
            .size_full()
            .bg(Background::SURFACE)
            // ── Left tab rail ──
            .child(
                div()
                    .id("tab-rail-container")
                    .flex()
                    .flex_row()
                    .h_full()
                    // Rail content
                    .child(
                        div()
                            .id("tab-rail")
                            .flex()
                            .flex_col()
                            .items_center()
                            .w(px(MediaPanel::TAB_RAIL_WIDTH))
                            .h_full()
                            .pt(px(Spacing::SM))
                            .pb(px(Spacing::SM))
                            .gap(px(Spacing::XS))
                            .bg(Background::RAISED)
                            // Media tab button
                            .child(
                                div()
                                    .id("tab-media")
                                    .w(px(btn_size))
                                    .h(px(btn_size))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(px(Radius::SM))
                                    .cursor_pointer()
                                    .bg(if media_active { BorderColors::PRIMARY } else { Background::RAISED })
                                    .text_color(if media_active { Text::PRIMARY } else { Text::TERTIARY })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.select_tab(MediaPanelTab::Media, cx);
                                    }))
                                    .child("M"),
                            )
                            // Captions tab button
                            .child(
                                div()
                                    .id("tab-captions")
                                    .w(px(btn_size))
                                    .h(px(btn_size))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(px(Radius::SM))
                                    .cursor_pointer()
                                    .bg(if captions_active { BorderColors::PRIMARY } else { Background::RAISED })
                                    .text_color(if captions_active { Text::PRIMARY } else { Text::TERTIARY })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.select_tab(MediaPanelTab::Captions, cx);
                                    }))
                                    .child("C"),
                            )
                            // Music tab button
                            .child(
                                div()
                                    .id("tab-music")
                                    .w(px(btn_size))
                                    .h(px(btn_size))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(px(Radius::SM))
                                    .cursor_pointer()
                                    .bg(if music_active { BorderColors::PRIMARY } else { Background::RAISED })
                                    .text_color(if music_active { Text::PRIMARY } else { Text::TERTIARY })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.select_tab(MediaPanelTab::Music, cx);
                                    }))
                                    .child("♪"),
                            ),
                    )
                    // Right hairline border separator
                    .child(
                        div()
                            .w(px(1.0))
                            .h_full()
                            .bg(BorderColors::PRIMARY),
                    ),
            )
            // ── Tab content area ──
            .child(
                div()
                    .id("tab-content")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .h_full()
                    .bg(Background::SURFACE)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .size_full()
                            .text_color(Text::MUTED)
                            .child(match &active {
                                MediaPanelTab::Media => "Media",
                                MediaPanelTab::Captions => "Captions",
                                MediaPanelTab::Music => "Music",
                            }),
                    ),
            )
    }
}

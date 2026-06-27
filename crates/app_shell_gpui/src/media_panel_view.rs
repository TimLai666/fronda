//! Media panel gpui view — left tab rail + content area.
//!
//! Covers UIX-011 (panel widths), THM-017 (tab rail width formula),
//! and the MediaPanelView from 07-ui-port-spec.md.

use crate::media_panel_model::{MediaPanelState, MediaPanelTab};
use crate::theme::{Background, BorderColors, FontSize, IconSize, Layout, MediaPanel, Radius, Spacing, Text};
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

/// Tab button: 26px square (Swift: IconSize.lg = 26) with active indicator strip.
fn tab_btn(id: &str, label: &str, is_active: bool) -> impl IntoElement {
    let btn_size = IconSize::LG; // 26px — matches Swift
    // Active bg: white@6% (Opacity::HINT), inactive: transparent
    let bg = if is_active {
        gpui::Hsla { h: 0.0, s: 0.0, l: 1.0, a: 0.06 }
    } else {
        Background::RAISED
    };
    div()
        .id(id.to_string())
        .relative()
        .w(px(btn_size))
        .h(px(btn_size))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(Radius::SM))
        .cursor_pointer()
        .bg(bg)
        .text_color(if is_active { Text::PRIMARY } else { Text::TERTIARY })
        .text_size(px(FontSize::SM_MD))
        // Active indicator: 2×18px capsule on left edge (Swift: Capsule().frame(width:2,height:18))
        .when(is_active, |el| {
            el.child(
                div()
                    .absolute()
                    .left_0()
                    .top(px((btn_size - 18.0) / 2.0))
                    .w(px(2.0))
                    .h(px(18.0))
                    .rounded_full()
                    .bg(BorderColors::PRIMARY),
            )
        })
        .child(label.to_string())
}

/// Media content tab: search bar + drop zone.
fn media_tab_content() -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .size_full()
        // Search bar
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .w_full()
                .h(px(28.0))
                .px(px(Spacing::SM))
                .bg(Background::RAISED)
                .border_b_1()
                .border_color(BorderColors::SUBTLE)
                .child(
                    div()
                        .text_color(Text::MUTED)
                        .text_size(px(FontSize::SM))
                        .child("⌕ Search media…"),
                ),
        )
        // Drop zone
        .child(
            div()
                .flex()
                .flex_1()
                .items_center()
                .justify_center()
                .flex_col()
                .gap(px(Spacing::SM))
                .text_color(Text::MUTED)
                .text_size(px(FontSize::SM))
                .child("Drop media here")
                .child(
                    div()
                        .text_size(px(FontSize::XS))
                        .text_color(Text::MUTED)
                        .child("or click Import"),
                ),
        )
        // Import button
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_center()
                .w_full()
                .h(px(36.0))
                .border_t_1()
                .border_color(BorderColors::SUBTLE)
                .bg(Background::RAISED)
                .child(
                    div()
                        .px(px(Spacing::MD))
                        .py(px(Spacing::XS))
                        .rounded(px(Radius::SM))
                        .border_1()
                        .border_color(BorderColors::PRIMARY)
                        .text_color(Text::SECONDARY)
                        .text_size(px(FontSize::SM))
                        .cursor_pointer()
                        .child("Import Media"),
                ),
        )
}

/// Captions tab: empty with placeholder.
fn captions_tab_content() -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .size_full()
        // Header with add button
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .w_full()
                .h(px(Layout::PANEL_HEADER_HEIGHT))
                .px(px(Spacing::MD))
                .bg(Background::RAISED)
                .border_b_1()
                .border_color(BorderColors::SUBTLE)
                .child(
                    div()
                        .flex_1()
                        .text_color(Text::SECONDARY)
                        .text_size(px(FontSize::SM))
                        .child("Captions"),
                )
                .child(
                    div()
                        .text_color(Text::MUTED)
                        .text_size(px(FontSize::MD))
                        .cursor_pointer()
                        .child("+"),
                ),
        )
        // Empty state
        .child(
            div()
                .flex()
                .flex_1()
                .items_center()
                .justify_center()
                .text_color(Text::MUTED)
                .text_size(px(FontSize::SM))
                .child("No captions"),
        )
}

/// Music tab: empty with placeholder.
fn music_tab_content() -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .size_full()
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .w_full()
                .h(px(Layout::PANEL_HEADER_HEIGHT))
                .px(px(Spacing::MD))
                .bg(Background::RAISED)
                .border_b_1()
                .border_color(BorderColors::SUBTLE)
                .child(
                    div()
                        .text_color(Text::SECONDARY)
                        .text_size(px(FontSize::SM))
                        .child("Music"),
                ),
        )
        .child(
            div()
                .flex()
                .flex_1()
                .items_center()
                .justify_center()
                .text_color(Text::MUTED)
                .text_size(px(FontSize::SM))
                .child("No music tracks"),
        )
}

impl Render for MediaPanelView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active = self.state.active_tab.clone();
        let media_active = active == MediaPanelTab::Media;
        let captions_active = active == MediaPanelTab::Captions;
        let music_active = active == MediaPanelTab::Music;

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
                            .child(
                                tab_btn("tab-media", "M", media_active)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.select_tab(MediaPanelTab::Media, cx);
                                    })),
                            )
                            .child(
                                tab_btn("tab-captions", "C", captions_active)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.select_tab(MediaPanelTab::Captions, cx);
                                    })),
                            )
                            .child(
                                tab_btn("tab-music", "♪", music_active)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.select_tab(MediaPanelTab::Music, cx);
                                    })),
                            ),
                    )
                    // Hairline border separator
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
                    .child(match active {
                        MediaPanelTab::Media => media_tab_content().into_any_element(),
                        MediaPanelTab::Captions => captions_tab_content().into_any_element(),
                        MediaPanelTab::Music => music_tab_content().into_any_element(),
                    }),
            )
    }
}

//! Media panel gpui view — left tab rail + content area.
//!
//! Covers UIX-011 (panel widths), THM-017 (tab rail width formula),
//! and the MediaPanelView from 07-ui-port-spec.md.

use crate::generation_view::GenerationView;
use crate::media_panel_model::{MediaPanelState, MediaPanelTab};
use crate::theme::{Accent, Background, BorderColors, FontSize, IconSize, Layout, MediaPanel, Radius, Spacing, Text};
use gpui::{
    div, prelude::*, px, App, Context, Entity, FocusHandle, Focusable, IntoElement,
    InteractiveElement, ParentElement, Render, Styled, Window,
};

/// Media panel gpui entity.
pub struct MediaPanelView {
    pub state: MediaPanelState,
    focus_handle: FocusHandle,
    /// AI generation panel embedded in the media tab (Swift: GenerationView).
    pub generation: Entity<GenerationView>,
}

impl MediaPanelView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let gen = cx.new(|cx| GenerationView::new(cx));
        Self {
            state: MediaPanelState::new(),
            focus_handle: cx.focus_handle(),
            generation: gen,
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

/// Tab button: 26px square (Swift: IconSize.lg = 26).
///
/// Active bg = white@10% (Opacity::SOFT), matching Swift HoverHighlight(isActive: true).
/// No left-edge capsule — Swift uses rounded-rect fill only.
fn tab_btn(id: &str, label: &str, is_active: bool) -> gpui::Stateful<gpui::Div> {
    let btn_size = IconSize::LG; // 26px
    let bg = if is_active {
        // white@10% matches Swift Opacity.soft (isActive, !isHovered)
        gpui::Hsla { h: 0.0, s: 0.0, l: 1.0, a: 0.10 }
    } else {
        gpui::Hsla { h: 0.0, s: 0.0, l: 0.0, a: 0.0 } // clear
    };
    div()
        .id(id.to_string())
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
        .child(label.to_string())
}

/// Media library empty state — shown when no assets exist.
#[allow(dead_code)]
fn media_empty_state() -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .flex_1()
        .items_center()
        .justify_center()
        .gap(px(Spacing::SM))
        .child(
            div()
                .text_color(Text::MUTED)
                .text_size(px(FontSize::SM))
                .child("Drop media here"),
        )
        .child(
            div()
                .text_color(Text::MUTED)
                .text_size(px(FontSize::XS))
                .child("or click Import"),
        )
}

/// Demo media tile — 80×60 thumbnail + name strip (matches Swift AssetThumbnailView).
fn demo_tile(id: &str, icon: &str, name: &str, hue: f32) -> impl IntoElement {
    div()
        .id(id.to_string())
        .flex()
        .flex_col()
        .w(px(80.0))
        .cursor_pointer()
        .child(
            div()
                .w(px(80.0))
                .h(px(60.0))
                .rounded(px(Radius::XS_SM))
                .bg(gpui::Hsla { h: hue, s: 0.35, l: 0.18, a: 1.0 })
                .flex()
                .items_center()
                .justify_center()
                .text_color(gpui::Hsla { h: hue, s: 0.60, l: 0.65, a: 1.0 })
                .text_size(px(FontSize::LG))
                .child(icon.to_string()),
        )
        .child(
            div()
                .w(px(80.0))
                .pt(px(Spacing::XXS))
                .text_color(Text::SECONDARY)
                .text_size(px(FontSize::XS))
                .overflow_hidden()
                .child(name.to_string()),
        )
}

/// Demo asset grid — flex-wrap tile grid matching Swift LazyVGrid.
fn media_demo_grid() -> impl IntoElement {
    div()
        .id("media-grid-scroll")
        .flex_1()
        .overflow_y_scroll()
        .child(
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .gap(px(Spacing::SM_MD))
                .p(px(Spacing::SM_MD))
                .child(demo_tile("tile-0", "▶", "interview.mp4",  0.60))
                .child(demo_tile("tile-1", "▶", "b-roll.mp4",     0.75))
                .child(demo_tile("tile-2", "♪", "music.wav",      0.83))
                .child(demo_tile("tile-3", "⬜", "title-card.png", 0.55))
                .child(demo_tile("tile-4", "▶", "drone.mp4",      0.35))
                .child(demo_tile("tile-5", "▶", "closeup.mp4",    0.06))
        )
}

/// Media toolbar row — matches Swift MediaTab.actionsRow + searchControlsRow.
fn media_toolbar() -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(Spacing::XS))
        .px(px(Spacing::SM))
        .pt(px(Spacing::SM))
        .pb(px(Spacing::XS))
        .bg(Background::SURFACE)
        .border_b_1()
        .border_color(BorderColors::SUBTLE)
        // Actions row: Import + Generate
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(Spacing::XS))
                .h(px(Layout::PANEL_HEADER_HEIGHT))
                // Import button
                .child(
                    div()
                        .id("btn-import-media")
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(Spacing::XS))
                        .px(px(Spacing::SM))
                        .h(px(24.0))
                        .rounded(px(Radius::SM))
                        .border_1()
                        .border_color(BorderColors::SUBTLE)
                        .cursor_pointer()
                        .text_color(Text::SECONDARY)
                        .text_size(px(FontSize::SM))
                        .child("+ Import"),
                )
                // Generate button (filled, AI gradient approximated as Accent::PRIMARY)
                .child(
                    div()
                        .id("btn-generate-media")
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(Spacing::XS))
                        .px(px(Spacing::SM))
                        .h(px(24.0))
                        .rounded(px(Radius::SM))
                        .bg(Accent::PRIMARY)
                        .cursor_pointer()
                        .text_color(Background::BASE)
                        .text_size(px(FontSize::SM))
                        .child("✦ Generate"),
                ),
        )
        // Search + display controls row
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(Spacing::XS))
                .h(px(Layout::PANEL_HEADER_HEIGHT))
                // Search field
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .flex_1()
                        .px(px(Spacing::SM))
                        .h(px(22.0))
                        .rounded(px(Radius::SM))
                        .border_1()
                        .border_color(BorderColors::SUBTLE)
                        .bg(Background::RAISED)
                        .text_color(Text::MUTED)
                        .text_size(px(FontSize::SM))
                        .child("⌕ Search"),
                )
                // View mode icon button
                .child(
                    div()
                        .w(px(22.0))
                        .h(px(22.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .text_color(Text::TERTIARY)
                        .text_size(px(FontSize::SM))
                        .child("⊞"),
                )
                // Sort icon button
                .child(
                    div()
                        .w(px(22.0))
                        .h(px(22.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .text_color(Text::TERTIARY)
                        .text_size(px(FontSize::SM))
                        .child("↕"),
                ),
        )
        // Breadcrumb / context bar
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(Spacing::XS))
                .h(px(20.0))
                .text_color(Text::MUTED)
                .text_size(px(FontSize::XS))
                .child("Library"),
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
        let generation_entity = self.generation.clone();

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
                        MediaPanelTab::Media => div()
                            .flex()
                            .flex_col()
                            .size_full()
                            // Toolbar at top (Import + Generate + Search + View controls)
                            .child(media_toolbar())
                            // Library grid (demo tiles; real assets would populate this)
                            .child(media_demo_grid())
                            // GenerationView anchored to BOTTOM with padding (Swift: .padding(.horizontal, sm).padding(.bottom, sm))
                            .child(
                                div()
                                    .px(px(crate::theme::Spacing::SM))
                                    .pb(px(crate::theme::Spacing::SM))
                                    .child(generation_entity),
                            )
                            .into_any_element(),
                        MediaPanelTab::Captions => captions_tab_content().into_any_element(),
                        MediaPanelTab::Music => music_tab_content().into_any_element(),
                    }),
            )
    }
}

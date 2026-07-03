//! Media panel gpui view — left tab rail + content area.
//!
//! Covers UIX-011 (panel widths), THM-017 (tab rail width formula),
//! and the MediaPanelView from 07-ui-port-spec.md.

use crate::generation_view::GenerationView;
use crate::media_panel_model::{MediaPanelState, MediaPanelTab};
use crate::theme::{
    Accent, Background, BorderColors, FontSize, IconSize, Layout, MediaPanel, Radius, Spacing, Text,
};
use gpui::{
    div, prelude::*, px, App, Context, Entity, FocusHandle, Focusable, InteractiveElement,
    IntoElement, ParentElement, Render, SharedString, Styled, Window,
};

/// Simple tooltip capsule for tab buttons.
struct TabTooltip {
    label: SharedString,
}

impl Render for TabTooltip {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px(px(Spacing::SM))
            .py(px(Spacing::XXS))
            .rounded(px(Radius::SM))
            .bg(Background::PROMINENT)
            .text_color(Text::PRIMARY)
            .text_size(px(FontSize::XS))
            .child(self.label.clone())
    }
}

/// Media panel gpui entity.
pub struct MediaPanelView {
    pub state: MediaPanelState,
    focus_handle: FocusHandle,
    /// AI generation panel embedded in the media tab (Swift: GenerationView).
    pub generation: Entity<GenerationView>,
    /// Last seen shared-state revision; manifest changes rebuild the grid.
    state_revision: u64,
}

impl MediaPanelView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let gen = cx.new(|cx| GenerationView::new(cx));
        let mut view = Self {
            state: MediaPanelState::new(),
            focus_handle: cx.focus_handle(),
            generation: gen,
            state_revision: u64::MAX,
        };
        view.sync_from_shared_state();
        view
    }

    /// Rebuild grid data from the shared manifest when the revision moved.
    fn sync_from_shared_state(&mut self) -> bool {
        let hub = crate::editor_state_hub::EditorStateHub::global();
        let revision = hub.revision();
        if revision == self.state_revision {
            return false;
        }
        self.state_revision = revision;
        let executor = hub.executor();
        let Ok(exec) = executor.lock() else {
            return false;
        };
        let root = hub.project_root();
        self.state
            .sync_from_manifest(exec.media_manifest(), root.as_deref());
        true
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
/// Active: white@10% bg + 2.5px left-edge capsule in BorderColors::PRIMARY
/// (Swift: HoverHighlight(isActive) + Capsule overlay on leading edge).
fn tab_btn(id: &str, label: &str, is_active: bool) -> gpui::Stateful<gpui::Div> {
    let btn_size = IconSize::LG; // 26px
    let bg = if is_active {
        gpui::Hsla {
            h: 0.0,
            s: 0.0,
            l: 1.0,
            a: 0.10,
        }
    } else {
        gpui::Hsla {
            h: 0.0,
            s: 0.0,
            l: 0.0,
            a: 0.0,
        }
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
        .text_color(if is_active {
            Text::PRIMARY
        } else {
            Text::TERTIARY
        })
        .text_size(px(FontSize::SM_MD))
        .child(label.to_string())
        // Left-edge accent capsule (Swift: Capsule overlay at topLeading)
        .when(is_active, |el| {
            el.child(
                div()
                    .absolute()
                    .left_0()
                    .top(px(5.0))
                    .w(px(2.5))
                    .h(px(16.0))
                    .rounded_full()
                    .bg(BorderColors::PRIMARY),
            )
        })
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
                .bg(gpui::Hsla {
                    h: hue,
                    s: 0.35,
                    l: 0.18,
                    a: 1.0,
                })
                .flex()
                .items_center()
                .justify_center()
                .text_color(gpui::Hsla {
                    h: hue,
                    s: 0.60,
                    l: 0.65,
                    a: 1.0,
                })
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

/// Media tile rendering the actual image file (80x60, cover).
fn image_tile(id: &str, name: &str, path: std::path::PathBuf) -> impl IntoElement {
    div()
        .id(format!("tile-{id}"))
        .flex()
        .flex_col()
        .w(px(80.0))
        .cursor_pointer()
        .child(
            div()
                .w(px(80.0))
                .h(px(60.0))
                .rounded(px(Radius::XS_SM))
                .overflow_hidden()
                .child(
                    gpui::img(path)
                        .size_full()
                        .object_fit(gpui::ObjectFit::Cover),
                ),
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

/// Library grid driven by the shared manifest entries.
fn media_grid(items: &[crate::media_panel_model::MediaItem]) -> impl IntoElement {
    let mut grid = div()
        .flex()
        .flex_row()
        .flex_wrap()
        .gap(px(Spacing::SM_MD))
        .p(px(Spacing::SM_MD));
    for item in items {
        let image = match item.kind {
            core_model::ClipType::Image => item.source_path.clone(),
            core_model::ClipType::Video => item
                .source_path
                .as_deref()
                .and_then(crate::video_thumbnails::request_thumbnail),
            _ => None,
        };
        if let Some(path) = image {
            grid = grid.child(image_tile(&item.id, &item.name, path));
        } else {
            grid = grid.child(demo_tile(
                &format!("tile-{}", item.id),
                crate::media_panel_model::tile_icon(&item.kind),
                &item.name,
                crate::media_panel_model::tile_hue(&item.id),
            ));
        }
    }
    div()
        .id("media-grid-scroll")
        .flex_1()
        .overflow_y_scroll()
        .child(grid)
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

fn section_label(text: &str) -> impl IntoElement {
    div()
        .text_color(Text::MUTED)
        .text_size(px(FontSize::XXS))
        .child(text.to_uppercase())
}

fn row_value(label: &str, value: &str) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .h(px(28.0))
        .px(px(Spacing::MD_LG))
        .child(
            div()
                .flex_1()
                .text_color(Text::TERTIARY)
                .text_size(px(FontSize::SM))
                .child(label.to_string()),
        )
        .child(
            div()
                .text_color(Text::SECONDARY)
                .text_size(px(FontSize::SM))
                .child(value.to_string()),
        )
}

fn generate_btn(id: &str) -> impl IntoElement {
    use crate::theme::Accent;
    div()
        .id(id.to_string())
        .w_full()
        .h(px(32.0))
        .rounded(px(crate::theme::Radius::SM))
        .bg(Accent::PRIMARY)
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .text_color(Background::BASE)
        .text_size(px(FontSize::SM))
        .child("Generate")
}

/// Captions tab: Source, Style, and Placement sections + Generate button.
fn captions_tab_content() -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .size_full()
        .bg(Background::SURFACE)
        .child(
            div()
                .id("captions-scroll")
                .flex()
                .flex_col()
                .flex_1()
                .overflow_y_scroll()
                .px(px(Spacing::LG_XL))
                .py(px(Spacing::MD))
                .gap(px(Spacing::LG))
                // Source section
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(Spacing::XS))
                        .child(section_label("Source"))
                        .child(row_value("Input", "Auto")),
                )
                // Style section
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(Spacing::XS))
                        .child(section_label("Style"))
                        .child(row_value("Font Size", "36"))
                        .child(row_value("Case", "Auto"))
                        .child(row_value("Censor Profanity", "Off")),
                )
                // Placement
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(Spacing::XS))
                        .child(section_label("Placement"))
                        .child(row_value("Position", "Bottom Center")),
                ),
        )
        // Generate bar at bottom (matches Swift generateBar)
        .child(
            div()
                .flex()
                .flex_col()
                .px(px(Spacing::LG_XL))
                .py(px(Spacing::SM_MD))
                .border_t_1()
                .border_color(BorderColors::SUBTLE)
                .bg(Background::RAISED)
                .child(generate_btn("btn-gen-captions")),
        )
}

/// Music tab: Source, Model, Prompt, Duration + Generate button.
fn music_tab_content() -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .size_full()
        .bg(Background::SURFACE)
        .child(
            div()
                .id("music-scroll")
                .flex()
                .flex_col()
                .flex_1()
                .overflow_y_scroll()
                .px(px(Spacing::LG_XL))
                .py(px(Spacing::MD))
                .gap(px(Spacing::LG))
                // Source section
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(Spacing::XS))
                        .child(section_label("Source"))
                        .child(row_value("Input", "Video to Music"))
                        .child(row_value("Video", "Whole timeline")),
                )
                // Model
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(Spacing::XS))
                        .child(section_label("Model"))
                        .child(row_value("Model", "ElevenLabs Music ⌄")),
                )
                // Prompt area
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(Spacing::XS))
                        .child(section_label("Prompt"))
                        .child(
                            div()
                                .h(px(80.0))
                                .rounded(px(crate::theme::Radius::SM))
                                .border_1()
                                .border_color(BorderColors::SUBTLE)
                                .bg(Background::RAISED)
                                .px(px(Spacing::SM_MD))
                                .py(px(Spacing::SM))
                                .text_color(Text::MUTED)
                                .text_size(px(FontSize::SM))
                                .child("Describe the music…"),
                        ),
                ),
        )
        // Generate bar at bottom
        .child(
            div()
                .flex()
                .flex_col()
                .px(px(Spacing::LG_XL))
                .py(px(Spacing::SM_MD))
                .border_t_1()
                .border_color(BorderColors::SUBTLE)
                .bg(Background::RAISED)
                .child(generate_btn("btn-gen-music")),
        )
}

impl Render for MediaPanelView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.sync_from_shared_state() {
            cx.notify();
        }
        let media_items = self.state.items.clone();
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
                                    }))
                                    .tooltip(|_, cx| {
                                        cx.new(|_| TabTooltip {
                                            label: "Media".into(),
                                        })
                                        .into()
                                    }),
                            )
                            .child(
                                tab_btn("tab-captions", "C", captions_active)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.select_tab(MediaPanelTab::Captions, cx);
                                    }))
                                    .tooltip(|_, cx| {
                                        cx.new(|_| TabTooltip {
                                            label: "Captions".into(),
                                        })
                                        .into()
                                    }),
                            )
                            .child(
                                tab_btn("tab-music", "♪", music_active)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.select_tab(MediaPanelTab::Music, cx);
                                    }))
                                    .tooltip(|_, cx| {
                                        cx.new(|_| TabTooltip {
                                            label: "Music".into(),
                                        })
                                        .into()
                                    }),
                            ),
                    )
                    // Hairline border separator
                    .child(div().w(px(1.0)).h_full().bg(BorderColors::PRIMARY)),
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
                            .child(media_grid(&media_items))
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

/// Preview panel gpui view — canvas + scrub bar + transport controls.
///
/// Matches PreviewContainerView.swift layout.
/// TransformOverlayView and CropOverlayView are layered on top of the canvas.
use crate::crop_overlay_view::CropOverlayView;
use crate::preview_guides::{ViewerGuide, ViewerGuideState};
use crate::preview_model::PlaybackState;
use crate::theme::{
    Accent, Background, BorderColors, FontSize, Layout, Opacity, Radius, Spacing, Text,
};
use crate::transform_overlay_view::TransformOverlayView;
use gpui::{
    div, prelude::*, px, svg, App, Context, Entity, FocusHandle, Focusable, InteractiveElement,
    IntoElement, ParentElement, Render, Styled, Window,
};

/// Canvas overlay state — mirrors Swift PreviewView offline/generating/failed states.
#[derive(Debug, Clone, PartialEq)]
pub enum CanvasOverlay {
    None,
    Offline,
    Generating { progress_pct: u8 },
    Failed { message: String },
}

/// A single open tab in the preview header — mirrors Swift PreviewTab.
#[derive(Debug, Clone, PartialEq)]
pub enum PreviewTabItem {
    Timeline,
    MediaAsset { name: String },
}

impl PreviewTabItem {
    pub fn display_name(&self) -> &str {
        match self {
            Self::Timeline => "Timeline",
            Self::MediaAsset { name } => name.as_str(),
        }
    }

    pub fn is_closeable(&self) -> bool {
        !matches!(self, Self::Timeline)
    }
}

pub struct PreviewView {
    pub state: PlaybackState,
    pub show_transform_overlay: bool,
    pub show_crop_overlay: bool,
    pub canvas_overlay: CanvasOverlay,
    /// Open tabs (Swift: editor.previewTabs). Timeline is always index 0.
    pub preview_tabs: Vec<PreviewTabItem>,
    pub active_tab_idx: usize,
    /// Active viewer guides (safe zones, format bars). Matches Swift ViewerGuideState.
    pub guide_state: ViewerGuideState,
    /// Whether the guides dropdown menu is open.
    pub show_guide_menu: bool,
    transform_overlay: Entity<TransformOverlayView>,
    crop_overlay: Entity<CropOverlayView>,
    focus_handle: FocusHandle,
    /// Cache PNG of the last composited preview frame, shown on the canvas.
    frame_png: Option<std::path::PathBuf>,
    /// (project revision, frame) currently rendered or in flight — avoids
    /// re-compositing the same frame every render.
    rendered_key: Option<(u64, i64)>,
}

impl PreviewView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            state: PlaybackState::new(),
            show_transform_overlay: false,
            show_crop_overlay: false,
            canvas_overlay: CanvasOverlay::None,
            preview_tabs: vec![PreviewTabItem::Timeline],
            active_tab_idx: 0,
            guide_state: ViewerGuideState::new(),
            show_guide_menu: false,
            transform_overlay: cx.new(|cx| TransformOverlayView::new(cx)),
            crop_overlay: cx.new(|cx| CropOverlayView::new(cx)),
            focus_handle: cx.focus_handle(),
            frame_png: None,
            rendered_key: None,
        }
    }

    /// If the current playhead frame hasn't been composited yet, kick a
    /// background render of it to a cache PNG and show it when ready. Skipped
    /// during playback (per-frame decode would be too slow) — the last rendered
    /// frame stays on screen. Compose + decode + encode run off the UI thread,
    /// so this never blocks rendering.
    fn ensure_preview_frame(&mut self, cx: &mut Context<Self>) {
        if self.state.is_playing || self.active_tab_idx != 0 {
            return;
        }
        let hub = crate::editor_state_hub::EditorStateHub::global();
        let key = (hub.revision(), self.state.active_frame);
        if self.rendered_key == Some(key) {
            return;
        }
        self.rendered_key = Some(key);
        let frame = self.state.active_frame;
        let (revision, _) = key;
        cx.spawn(async move |this, cx| {
            let rendered = cx
                .background_executor()
                .spawn(async move {
                    let hub = crate::editor_state_hub::EditorStateHub::global();
                    let (timeline, manifest, root) = {
                        let exec = hub.executor();
                        let guard = exec.lock().unwrap();
                        (
                            guard.timeline().clone(),
                            guard.media_manifest().clone(),
                            hub.project_root().unwrap_or_else(|| {
                                std::env::temp_dir().join("fronda-preview")
                            }),
                        )
                    };
                    let cache_dir =
                        crate::project_registry_store::fronda_config_dir().join("preview");
                    let out = crate::preview_render::preview_cache_path(&cache_dir, revision, frame);
                    if out.is_file() {
                        return Some(out);
                    }
                    crate::preview_render::render_frame_png(
                        &timeline, &manifest, &root, frame, &out,
                    )
                    .ok()
                    .map(|()| out)
                })
                .await;
            if let Some(path) = rendered {
                let _ = this.update(cx, |view, cx| {
                    view.frame_png = Some(path);
                    cx.notify();
                });
            }
        })
        .detach();
    }

    /// Open a media asset tab (Swift: openMediaAssetTab). Selects it immediately.
    pub fn open_media_tab(&mut self, name: String, cx: &mut Context<Self>) {
        let tab = PreviewTabItem::MediaAsset { name };
        if let Some(idx) = self.preview_tabs.iter().position(|t| t == &tab) {
            self.active_tab_idx = idx;
        } else {
            self.preview_tabs.push(tab);
            self.active_tab_idx = self.preview_tabs.len() - 1;
        }
        cx.notify();
    }

    pub fn close_tab(&mut self, idx: usize, cx: &mut Context<Self>) {
        if idx == 0 {
            return;
        } // Timeline is never closeable
        self.preview_tabs.remove(idx);
        if self.active_tab_idx >= self.preview_tabs.len() {
            self.active_tab_idx = self.preview_tabs.len().saturating_sub(1);
        }
        cx.notify();
    }

    pub fn select_tab(&mut self, idx: usize, cx: &mut Context<Self>) {
        if idx < self.preview_tabs.len() {
            self.active_tab_idx = idx;
            cx.notify();
        }
    }

    pub fn go_back(&mut self, cx: &mut Context<Self>) {
        if self.active_tab_idx > 0 {
            self.active_tab_idx -= 1;
            cx.notify();
        }
    }

    pub fn go_forward(&mut self, cx: &mut Context<Self>) {
        if self.active_tab_idx + 1 < self.preview_tabs.len() {
            self.active_tab_idx += 1;
            cx.notify();
        }
    }

    pub fn toggle_play(&mut self, cx: &mut Context<Self>) {
        self.state.toggle_play();
        cx.notify();
    }

    pub fn go_to_start(&mut self, cx: &mut Context<Self>) {
        self.state.go_to_start();
        cx.notify();
    }

    pub fn go_to_end(&mut self, cx: &mut Context<Self>) {
        self.state.go_to_end();
        cx.notify();
    }

    pub fn step_backward(&mut self, cx: &mut Context<Self>) {
        self.state.step_backward();
        cx.notify();
    }

    pub fn step_forward(&mut self, cx: &mut Context<Self>) {
        self.state.step_forward();
        cx.notify();
    }
}

impl Focusable for PreviewView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

/// Renders safe-zone / format-bar guide overlays over the canvas.
/// Matches Swift ViewerGuideOverlay rendering (border rects for safe zones,
/// solid bands for format reference bars).
fn viewer_guide_overlay(guides: &[ViewerGuide]) -> impl IntoElement {
    const GUIDE_COLOR: gpui::Hsla = gpui::Hsla {
        h: 0.0,
        s: 0.0,
        l: 1.0,
        a: 0.40,
    };
    const BAR_COLOR: gpui::Hsla = gpui::Hsla {
        h: 0.0,
        s: 0.0,
        l: 0.0,
        a: 0.50,
    };

    let mut root = div()
        .id("guide-overlay")
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .overflow_hidden();

    for guide in guides {
        match guide {
            ViewerGuide::ActionSafe => {
                root = root.child(
                    div()
                        .absolute()
                        .top(px(10.0))
                        .left(px(10.0))
                        .right(px(10.0))
                        .bottom(px(10.0))
                        .border_1()
                        .border_color(GUIDE_COLOR),
                );
            }
            ViewerGuide::TitleSafe => {
                root = root.child(
                    div()
                        .absolute()
                        .top(px(20.0))
                        .left(px(20.0))
                        .right(px(20.0))
                        .bottom(px(20.0))
                        .border_1()
                        .border_color(GUIDE_COLOR),
                );
            }
            ViewerGuide::Center => {
                root = root
                    .child(
                        div()
                            .absolute()
                            .top_0()
                            .bottom_0()
                            .left(gpui::relative(0.5))
                            .w(px(1.0))
                            .bg(GUIDE_COLOR),
                    )
                    .child(
                        div()
                            .absolute()
                            .left_0()
                            .right_0()
                            .top(gpui::relative(0.5))
                            .h(px(1.0))
                            .bg(GUIDE_COLOR),
                    );
            }
            ViewerGuide::Wide => {
                root = root
                    .child(
                        div()
                            .absolute()
                            .top_0()
                            .left_0()
                            .right_0()
                            .h(px(40.0))
                            .bg(BAR_COLOR),
                    )
                    .child(
                        div()
                            .absolute()
                            .bottom_0()
                            .left_0()
                            .right_0()
                            .h(px(40.0))
                            .bg(BAR_COLOR),
                    );
            }
            ViewerGuide::Square => {
                root = root
                    .child(
                        div()
                            .absolute()
                            .top_0()
                            .bottom_0()
                            .left_0()
                            .w(px(30.0))
                            .bg(BAR_COLOR),
                    )
                    .child(
                        div()
                            .absolute()
                            .top_0()
                            .bottom_0()
                            .right_0()
                            .w(px(30.0))
                            .bg(BAR_COLOR),
                    );
            }
            ViewerGuide::Portrait => {
                root = root
                    .child(
                        div()
                            .absolute()
                            .top_0()
                            .bottom_0()
                            .left_0()
                            .w(px(80.0))
                            .bg(BAR_COLOR),
                    )
                    .child(
                        div()
                            .absolute()
                            .top_0()
                            .bottom_0()
                            .right_0()
                            .w(px(80.0))
                            .bg(BAR_COLOR),
                    );
            }
            ViewerGuide::Scope => {
                root = root.child(
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .bottom(px(12.0))
                        .h(px(1.0))
                        .bg(GUIDE_COLOR),
                );
            }
        }
    }
    root
}

fn transport_btn_svg(
    id: &str,
    icon_path: &'static str,
    highlight: bool,
) -> gpui::Stateful<gpui::Div> {
    let color = if highlight {
        Text::PRIMARY
    } else {
        Text::SECONDARY
    };
    div()
        .id(id.to_string())
        .w(px(32.0))
        .h(px(28.0))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .rounded(px(4.0))
        .child(
            svg()
                .path(icon_path)
                .w(px(14.0))
                .h(px(14.0))
                .text_color(color),
        )
}

fn settings_badge(id: &str, label: &str) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id.to_string())
        .px(px(Spacing::XS))
        .py(px(1.0))
        .rounded(px(Radius::XS_SM))
        .bg(Background::RAISED)
        .border_1()
        .border_color(BorderColors::SUBTLE)
        .cursor_pointer()
        .child(
            div()
                .text_color(Text::SECONDARY)
                .text_size(px(FontSize::XS))
                .child(label.to_string()),
        )
}

impl Render for PreviewView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.ensure_preview_frame(cx);
        let frame_png = self.frame_png.clone();
        let is_playing = self.state.is_playing;
        let fraction = self.state.playhead_fraction();
        let current_tc = self.state.format_timecode();
        let total_tc = self.state.format_total();
        let show_transform = self.show_transform_overlay;
        let show_crop = self.show_crop_overlay;
        let canvas_overlay = self.canvas_overlay.clone();
        let active_tab_idx = self.active_tab_idx;
        let tab_count = self.preview_tabs.len();
        let can_go_back = active_tab_idx > 0;
        let can_go_forward = active_tab_idx + 1 < tab_count;
        let tabs: Vec<(usize, String, bool)> = self
            .preview_tabs
            .iter()
            .enumerate()
            .map(|(i, t)| (i, t.display_name().to_string(), t.is_closeable()))
            .collect();

        let transform_entity = self.transform_overlay.clone();
        let crop_entity = self.crop_overlay.clone();
        let active_guides: Vec<ViewerGuide> = self.guide_state.active_guides().to_vec();
        let any_guides = !active_guides.is_empty();

        // Real project settings for the badge row (was hardcoded).
        let (aspect_label, fps_label, quality_label) = {
            let hub = crate::editor_state_hub::EditorStateHub::global();
            let exec = hub.executor();
            let guard = exec.lock().unwrap();
            let t = guard.timeline();
            let (w, h, fps) = (t.width, t.height, t.fps);
            let quality = timeline_core::QUALITY_PRESETS
                .iter()
                .find(|q| q.matches(w, h))
                .map(|q| q.label.to_string())
                .unwrap_or_else(|| "Custom".into());
            (timeline_core::format_aspect_ratio(w, h), format!("{fps} fps"), quality)
        };
        div()
            .id("preview-panel")
            .flex()
            .flex_col()
            .size_full()
            .bg(Background::BASE)
            // Header tab bar (matches Swift PreviewContainerView.tabBar)
            .child(
                div()
                    .id("preview-header")
                    .flex()
                    .flex_row()
                    .items_center()
                    .w_full()
                    .h(px(Layout::PANEL_HEADER_HEIGHT))
                    .px(px(Spacing::SM))
                    .gap(px(Spacing::XS))
                    .bg(Background::RAISED)
                    .border_b_1()
                    .border_color(BorderColors::PRIMARY)
                    // ← back nav button
                    .child(
                        div()
                            .id("preview-back")
                            .w(px(18.0))
                            .h(px(22.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .text_color(if can_go_back {
                                Text::SECONDARY
                            } else {
                                Text::MUTED
                            })
                            .text_size(px(FontSize::SM))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.go_back(cx);
                            }))
                            .child("<"),
                    )
                    // → forward nav button
                    .child(
                        div()
                            .id("preview-fwd")
                            .w(px(18.0))
                            .h(px(22.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .text_color(if can_go_forward {
                                Text::SECONDARY
                            } else {
                                Text::MUTED
                            })
                            .text_size(px(FontSize::SM))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.go_forward(cx);
                            }))
                            .child(">"),
                    )
                    // Tab list (scrollable; overflow handled by ellipsis button at right end)
                    .children(tabs.into_iter().map(|(i, name, closeable)| {
                        let is_active = i == active_tab_idx;
                        div()
                            .id(format!("preview-tab-{i}"))
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(Spacing::XXS))
                            .px(px(Spacing::XS))
                            .pb(px(2.0))
                            .border_b(px(if is_active { 1.5 } else { 0.0 }))
                            .border_color(Accent::PRIMARY)
                            .cursor_pointer()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.select_tab(i, cx);
                            }))
                            .child(
                                div()
                                    .text_color(if is_active {
                                        Text::PRIMARY
                                    } else {
                                        Text::SECONDARY
                                    })
                                    .text_size(px(FontSize::SM))
                                    .font_weight(if is_active {
                                        gpui::FontWeight::SEMIBOLD
                                    } else {
                                        gpui::FontWeight::MEDIUM
                                    })
                                    .child(name),
                            )
                            .when(closeable, |el| {
                                el.child(
                                    div()
                                        .id(format!("preview-tab-close-{i}"))
                                        .w(px(12.0))
                                        .h(px(12.0))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .text_color(Text::MUTED)
                                        .text_size(px(FontSize::XS))
                                        .cursor_pointer()
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.close_tab(i, cx);
                                        }))
                                        .child("×"),
                                )
                            })
                    }))
                    // Spacer pushes overflow button to the right end
                    .child(div().flex_1())
                    // Overflow ellipsis button (Swift: tabBarOverflowButton — shows hidden tabs)
                    .child(
                        div()
                            .id("preview-tabs-overflow")
                            .w(px(22.0))
                            .h(px(22.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(Radius::XS_SM))
                            .cursor_pointer()
                            .text_color(Text::MUTED)
                            .text_size(px(FontSize::SM))
                            .child("⋯"),
                    ),
            )
            // Canvas area (relative so overlays can stack absolutely)
            .child(
                div()
                    .id("preview-canvas")
                    .flex()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .w_full()
                    .relative()
                    .bg(Background::BASE)
                    // Composited frame (or placeholder) — hidden when an overlay is shown
                    .when(canvas_overlay == CanvasOverlay::None, |el| match &frame_png {
                        Some(path) => el.child(
                            gpui::img(path.clone())
                                .max_w_full()
                                .max_h_full()
                                .object_fit(gpui::ObjectFit::Contain),
                        ),
                        None => el.child(
                            div()
                                .text_color(Text::MUTED)
                                .text_size(px(FontSize::SM))
                                .child("Preview"),
                        ),
                    })
                    // Offline overlay (Swift: "Media Offline" message)
                    .when(canvas_overlay == CanvasOverlay::Offline, |el| {
                        el.child(
                            div()
                                .flex()
                                .flex_col()
                                .items_center()
                                .gap(px(Spacing::SM))
                                .child(
                                    div()
                                        .text_color(Text::MUTED)
                                        .text_size(px(FontSize::MD_LG))
                                        .child("Media Offline"),
                                )
                                .child(
                                    div()
                                        .text_color(Text::MUTED)
                                        .text_size(px(FontSize::SM))
                                        .child("File not found on disk"),
                                ),
                        )
                    })
                    // Generating overlay (Swift: generation progress spinner)
                    .when(
                        matches!(canvas_overlay, CanvasOverlay::Generating { .. }),
                        |el| {
                            let pct = if let CanvasOverlay::Generating { progress_pct } =
                                &canvas_overlay
                            {
                                *progress_pct
                            } else {
                                0
                            };
                            el.child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .items_center()
                                    .gap(px(Spacing::MD))
                                    .child(
                                        div()
                                            .text_color(Accent::PRIMARY)
                                            .text_size(px(FontSize::DISPLAY))
                                            .child("✦"),
                                    )
                                    .child(
                                        div()
                                            .text_color(Text::SECONDARY)
                                            .text_size(px(FontSize::SM))
                                            .child(format!("Generating… {}%", pct)),
                                    ),
                            )
                        },
                    )
                    // Failed overlay (Swift: error badge)
                    .when(
                        matches!(canvas_overlay, CanvasOverlay::Failed { .. }),
                        |el| {
                            let msg = if let CanvasOverlay::Failed { message } = &canvas_overlay {
                                message.clone()
                            } else {
                                String::new()
                            };
                            el.child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .items_center()
                                    .gap(px(Spacing::SM))
                                    .child(
                                        div()
                                            .text_color(gpui::Hsla {
                                                h: 0.0,
                                                s: 0.85,
                                                l: 0.55,
                                                a: 1.0,
                                            })
                                            .text_size(px(FontSize::MD_LG))
                                            .child("Generation Failed"),
                                    )
                                    .child(
                                        div()
                                            .text_color(Text::MUTED)
                                            .text_size(px(FontSize::SM))
                                            .child(msg),
                                    ),
                            )
                        },
                    )
                    // Transform overlay — shown when select tool + clip selected
                    .when(show_transform, |el| {
                        el.child(
                            div()
                                .absolute()
                                .top_0()
                                .left_0()
                                .size_full()
                                .child(transform_entity),
                        )
                    })
                    // Crop overlay — shown when crop tool + clip selected
                    .when(show_crop, |el| {
                        el.child(
                            div()
                                .absolute()
                                .top_0()
                                .left_0()
                                .size_full()
                                .child(crop_entity),
                        )
                    })
                    // Viewer guide overlays — safe zones + format reference bars
                    .when(any_guides, |el| {
                        el.child(viewer_guide_overlay(&active_guides))
                    }),
            )
            // Scrub bar
            .child(
                div()
                    .id("preview-scrub")
                    .relative()
                    .w_full()
                    .h(px(12.0))
                    .bg(BorderColors::SUBTLE)
                    .cursor_pointer()
                    .child(
                        div()
                            .absolute()
                            .top_0()
                            .left_0()
                            .h_full()
                            .w(px((fraction as f32) * Layout::PREVIEW_MIN_WIDTH))
                            .bg(Accent::PRIMARY),
                    )
                    .child(
                        div()
                            .absolute()
                            .top(px(3.0))
                            .left(px((fraction as f32) * Layout::PREVIEW_MIN_WIDTH - 3.0))
                            .w(px(6.0))
                            .h(px(6.0))
                            .rounded_full()
                            .bg(Text::PRIMARY),
                    ),
            )
            // Transport bar
            .child(
                div()
                    .id("preview-transport")
                    .flex()
                    .flex_row()
                    .items_center()
                    .w_full()
                    .h(px(36.0))
                    .px(px(Spacing::MD))
                    .bg(Background::RAISED)
                    .border_t_1()
                    .border_color(BorderColors::PRIMARY)
                    .child(
                        div()
                            .flex()
                            .flex_1()
                            .flex_row()
                            .items_center()
                            .gap(px(2.0))
                            .child(
                                div()
                                    .text_color(Accent::TIMECODE)
                                    .text_size(px(FontSize::SM))
                                    .child(current_tc),
                            )
                            .child(
                                div()
                                    .text_color(Text::TERTIARY)
                                    .text_size(px(FontSize::SM))
                                    .child("/"),
                            )
                            .child(
                                div()
                                    .text_color(Text::SECONDARY)
                                    .text_size(px(FontSize::SM))
                                    .child(total_tc),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(Spacing::XS))
                            .child(
                                transport_btn_svg("btn-go-start", "icons/skip_back.svg", false)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.go_to_start(cx);
                                    })),
                            )
                            .child(
                                transport_btn_svg("btn-step-back", "icons/step_back.svg", false)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.step_backward(cx);
                                    })),
                            )
                            .child(
                                transport_btn_svg(
                                    "btn-play",
                                    if is_playing {
                                        "icons/pause.svg"
                                    } else {
                                        "icons/play.svg"
                                    },
                                    true,
                                )
                                .on_click(cx.listener(
                                    |this, _, _, cx| {
                                        this.toggle_play(cx);
                                    },
                                )),
                            )
                            .child(
                                transport_btn_svg("btn-step-fwd", "icons/step_forward.svg", false)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.step_forward(cx);
                                    })),
                            )
                            .child(
                                transport_btn_svg("btn-go-end", "icons/skip_forward.svg", false)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.go_to_end(cx);
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_1()
                            .flex_row()
                            .justify_end()
                            .items_center()
                            .gap(px(Spacing::XS))
                            // Badges reflect the live project settings (Swift: aspectRatio, fps, quality, viewFit)
                            .child(settings_badge("badge-aspect", &aspect_label))
                            .child(settings_badge("badge-fps", &fps_label))
                            .child(settings_badge("badge-quality", &quality_label))
                            .child(settings_badge("badge-fit", "Fit"))
                            // Guides toggle (Swift: guideMenuButton — shows/hides ViewerGuideMenu)
                            .child(
                                div()
                                    .id("btn-guides")
                                    .w(px(24.0))
                                    .h(px(24.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(px(Radius::XS))
                                    .cursor_pointer()
                                    .bg(if any_guides {
                                        Accent::PRIMARY.opacity(Opacity::MUTED)
                                    } else {
                                        Background::SURFACE
                                    })
                                    .border_1()
                                    .border_color(if any_guides {
                                        Accent::PRIMARY
                                    } else {
                                        BorderColors::SUBTLE
                                    })
                                    .text_size(px(FontSize::XS))
                                    .text_color(if any_guides {
                                        Accent::PRIMARY
                                    } else {
                                        Text::MUTED
                                    })
                                    .child("⊞"),
                            )
                            // Capture frame button (Swift: captureFrameButton → camera SF symbol)
                            .child(transport_btn_svg(
                                "btn-capture-frame",
                                "icons/camera.svg",
                                false,
                            )),
                    ),
            )
    }
}

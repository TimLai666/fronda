/// Preview panel gpui view — canvas + scrub bar + transport controls.
///
/// Matches PreviewContainerView.swift layout.
/// TransformOverlayView and CropOverlayView are layered on top of the canvas.
use crate::crop_overlay_view::CropOverlayView;
use crate::preview_guides::{ViewerGuide, ViewerGuideState};
use crate::preview_model::PlaybackState;
use crate::theme::{
    Accent, Background, BorderColors, FontSize, Layout, Opacity, Radius, Spacing, Status, Text,
};
use crate::transform_overlay_view::TransformOverlayView;
use gpui::{
    canvas, div, prelude::*, px, svg, App, Context, Entity, FocusHandle, Focusable,
    InteractiveElement, IntoElement, MouseButton, MouseDownEvent, ParentElement, Render, Styled,
    Window,
};

/// Compact stereo audio meter — L/R level bars with a peak tick and a clip
/// tint (upstream #293). Fed by the playhead audio level.
fn render_audio_meter(
    display: audio_core::audio_meter::StereoMeterDisplay,
) -> impl IntoElement {
    let bar = |ch: audio_core::audio_meter::MeterChannelDisplay| {
        let level = audio_core::audio_meter::normalized_level(ch.level_db);
        let peak = audio_core::audio_meter::normalized_level(ch.peak_db);
        div()
            .w_full()
            .h(px(4.0))
            .rounded(px(1.0))
            .bg(Background::RAISED)
            .relative()
            .overflow_hidden()
            .child(
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left_0()
                    .w(gpui::relative(level))
                    .bg(if ch.clipped {
                        Status::ERROR
                    } else {
                        Accent::PRIMARY
                    }),
            )
            .child(
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left(gpui::relative(peak.clamp(0.0, 0.98)))
                    .w(px(1.5))
                    .bg(Text::PRIMARY),
            )
    };
    div()
        .flex()
        .flex_col()
        .justify_center()
        .gap(px(2.0))
        .w(px(56.0))
        .child(bar(display.left))
        .child(bar(display.right))
}

/// Read the RGB (0..1) of the composited frame PNG at normalized (u, v).
fn sample_png_pixel(path: &std::path::Path, u: f64, v: f64) -> Option<(f64, f64, f64)> {
    let img = image::open(path).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return None;
    }
    let x = ((u * w as f64) as i64).clamp(0, w as i64 - 1) as u32;
    let y = ((v * h as f64) as i64).clamp(0, h as i64 - 1) as u32;
    let p = img.get_pixel(x, y);
    Some((
        p[0] as f64 / 255.0,
        p[1] as f64 / 255.0,
        p[2] as f64 / 255.0,
    ))
}

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

/// Which transport-bar settings dropdown is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsMenu {
    Aspect,
    Fps,
    Quality,
    Zoom,
}

/// What picking a settings-menu row does.
#[derive(Debug, Clone, PartialEq)]
pub enum SettingsSelection {
    /// set_project_settings { aspectRatio }
    AspectRatio(&'static str),
    /// set_project_settings { fps }
    Fps(i64),
    /// set_project_settings { quality }
    Quality(&'static str),
    /// View-local canvas zoom factor (1.0 = Fit).
    Zoom(f64),
}

/// One row of a settings dropdown.
#[derive(Debug, Clone, PartialEq)]
pub struct SettingsRow {
    pub label: String,
    pub checked: bool,
    pub select: SettingsSelection,
}

/// Rows for a settings menu, fed by `timeline_core::project_presets` with the
/// preset's own active-selection logic (Swift: PreviewContainerView menus).
pub fn settings_menu_rows(
    menu: SettingsMenu,
    width: i64,
    height: i64,
    fps: i64,
    canvas_zoom: f64,
) -> Vec<SettingsRow> {
    match menu {
        SettingsMenu::Aspect => timeline_core::ASPECT_PRESETS
            .iter()
            .map(|p| SettingsRow {
                label: p.label.to_string(),
                checked: p.is_active(width, height),
                select: SettingsSelection::AspectRatio(p.label),
            })
            .collect(),
        SettingsMenu::Fps => timeline_core::FPS_PRESETS
            .iter()
            .map(|&f| SettingsRow {
                label: format!("{f} fps"),
                checked: fps == f,
                select: SettingsSelection::Fps(f),
            })
            .collect(),
        SettingsMenu::Quality => timeline_core::QUALITY_PRESETS
            .iter()
            .map(|p| SettingsRow {
                label: p.label.to_string(),
                checked: p.matches(width, height),
                select: SettingsSelection::Quality(p.label),
            })
            .collect(),
        SettingsMenu::Zoom => timeline_core::ZOOM_PRESETS
            .iter()
            .map(|p| SettingsRow {
                label: p.label.to_string(),
                checked: zoom_preset_active(canvas_zoom, p.value),
                select: SettingsSelection::Zoom(p.value),
            })
            .collect(),
    }
}

/// Swift `isZoomPresetActive`: within 0.01 of the preset value.
pub fn zoom_preset_active(canvas_zoom: f64, preset_value: f64) -> bool {
    (canvas_zoom - preset_value).abs() < 0.01
}

/// One row of the Guides dropdown (Issue #169).
#[derive(Debug, Clone, PartialEq)]
pub struct GuideRow {
    pub guide: ViewerGuide,
    pub label: &'static str,
    pub checked: bool,
}

/// Rows for the viewer-guides dropdown, in display order: safe-zone + center
/// first, then format references. Mirrors Swift ViewerGuideMenu.
pub fn guide_menu_rows(state: &ViewerGuideState) -> Vec<GuideRow> {
    const ORDER: [ViewerGuide; 7] = [
        ViewerGuide::ActionSafe,
        ViewerGuide::TitleSafe,
        ViewerGuide::Center,
        ViewerGuide::Scope,
        ViewerGuide::Wide,
        ViewerGuide::Square,
        ViewerGuide::Portrait,
    ];
    ORDER
        .iter()
        .map(|&g| GuideRow {
            guide: g,
            label: g.label(),
            checked: state.is_active(g),
        })
        .collect()
}

/// Zoom badge label (Swift `zoomBadgeLabel`): "Fit" at 1.0, else a percentage.
pub fn zoom_badge_label(canvas_zoom: f64) -> String {
    if zoom_preset_active(canvas_zoom, 1.0) {
        "Fit".to_string()
    } else {
        format!("{}%", (canvas_zoom * 100.0) as i64)
    }
}

/// File name for a captured frame PNG in the project media directory.
pub fn capture_file_name(frame: i64, stamp: u128) -> String {
    format!("frame-{frame}-{stamp}.png")
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
    /// Open transport-bar settings dropdown, if any.
    pub open_settings_menu: Option<SettingsMenu>,
    /// Canvas zoom factor (Swift: editor.canvasZoom; 1.0 = Fit).
    pub canvas_zoom: f64,
    transform_overlay: Entity<TransformOverlayView>,
    crop_overlay: Entity<CropOverlayView>,
    focus_handle: FocusHandle,
    /// Cache PNG of the last composited preview frame, shown on the canvas.
    frame_png: Option<std::path::PathBuf>,
    /// (project revision, frame) currently rendered or in flight — avoids
    /// re-compositing the same frame every render.
    rendered_key: Option<(u64, i64)>,
    /// A background render is in flight; blocks a second one so playback renders
    /// best-effort (latest frame when the previous finishes) without flooding.
    rendering: bool,
    /// Preview-canvas bounds captured during paint (window coords), so the
    /// chroma eyedropper can map a click to a frame pixel.
    canvas_bounds: std::sync::Arc<std::sync::Mutex<Option<gpui::Bounds<gpui::Pixels>>>>,
    /// Audio meter (upstream #293) fed by the timeline audio level at the
    /// playhead — Fronda has no live audio output, so this is a playhead meter.
    meter: audio_core::audio_meter::StereoMeter,
    meter_start: std::time::Instant,
    /// (revision, mono peak envelope over the timeline) — one bucket per frame,
    /// computed off the UI thread and sampled at the playhead.
    envelope: std::sync::Arc<std::sync::Mutex<Option<(u64, Vec<f32>)>>>,
    envelope_computing: bool,
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
            open_settings_menu: None,
            canvas_zoom: 1.0,
            transform_overlay: cx.new(|cx| TransformOverlayView::new(cx)),
            crop_overlay: cx.new(|cx| CropOverlayView::new(cx)),
            focus_handle: cx.focus_handle(),
            frame_png: None,
            rendered_key: None,
            rendering: false,
            canvas_bounds: std::sync::Arc::new(std::sync::Mutex::new(None)),
            meter: audio_core::audio_meter::StereoMeter::default(),
            meter_start: std::time::Instant::now(),
            envelope: std::sync::Arc::new(std::sync::Mutex::new(None)),
            envelope_computing: false,
        }
    }

    /// Compute the timeline audio envelope off the UI thread when the project
    /// changed, so the meter can sample the level at the playhead cheaply.
    fn ensure_envelope(&mut self, cx: &mut Context<Self>) {
        if self.envelope_computing {
            return;
        }
        let hub = crate::editor_state_hub::EditorStateHub::global();
        let revision = hub.revision();
        if self
            .envelope
            .lock()
            .ok()
            .and_then(|e| e.as_ref().map(|(r, _)| *r))
            == Some(revision)
        {
            return;
        }
        self.envelope_computing = true;
        let slot = self.envelope.clone();
        cx.spawn(async move |this, cx| {
            let env = cx
                .background_executor()
                .spawn(async move {
                    let hub = crate::editor_state_hub::EditorStateHub::global();
                    let (timeline, manifest, root) = {
                        let exec = hub.executor();
                        let guard = match exec.lock() {
                            Ok(g) => g,
                            Err(_) => return None,
                        };
                        (
                            guard.timeline().clone(),
                            guard.media_manifest().clone(),
                            hub.project_root()
                                .unwrap_or_else(|| std::env::temp_dir().join("fronda-preview")),
                        )
                    };
                    let buckets =
                        (timeline_core::TimelineMathExt::total_frames(&timeline).max(1)) as usize;
                    Some(crate::audio_export::timeline_audio_envelope(
                        &timeline, &manifest, &root, buckets,
                    ))
                })
                .await;
            let _ = this.update(cx, |view, cx| {
                view.envelope_computing = false;
                if let (Some(env), Ok(mut slot)) = (env, slot.lock()) {
                    *slot = Some((revision, env));
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Sample the audio level at the playhead and update the meter. Returns the
    /// current stereo display for rendering.
    fn update_meter(&mut self) -> audio_core::audio_meter::StereoMeterDisplay {
        let frame = self.state.active_frame.max(0) as usize;
        let level = self
            .envelope
            .lock()
            .ok()
            .and_then(|e| e.as_ref().and_then(|(_, env)| env.get(frame).copied()))
            .unwrap_or(0.0);
        let time = self.meter_start.elapsed().as_secs_f64();
        self.meter.ingest(level, level, time);
        self.meter.display(time)
    }

    /// Chroma eyedropper: when sampling is armed, map a preview click to a frame
    /// pixel, read its colour, and apply `key.chroma` with that hue to the
    /// armed clip. Assumes Fit (canvas_zoom = 1.0); other zooms may be slightly
    /// off. Returns true when a sample was taken (so the click is consumed).
    fn try_sample_chroma(
        &mut self,
        position: gpui::Point<gpui::Pixels>,
        cx: &mut Context<Self>,
    ) -> bool {
        if crate::chroma_sampling::sampling_clip().is_none() {
            return false;
        }
        let bounds = match self.canvas_bounds.lock().ok().and_then(|b| *b) {
            Some(b) => b,
            None => return false,
        };
        let path = match self.frame_png.clone() {
            Some(p) => p,
            None => {
                crate::chroma_sampling::set_sampling(None);
                return true;
            }
        };
        let (aspect, _cw, _ch) = {
            let hub = crate::editor_state_hub::EditorStateHub::global();
            let exec = hub.executor();
            let guard = match exec.lock() {
                Ok(g) => g,
                Err(_) => return true,
            };
            let t = guard.timeline();
            (t.width as f64 / t.height.max(1) as f64, t.width, t.height)
        };
        let rel = (
            position.x.as_f32() - bounds.origin.x.as_f32(),
            position.y.as_f32() - bounds.origin.y.as_f32(),
        );
        let canvas = (bounds.size.width.as_f32(), bounds.size.height.as_f32());
        let clip_id = crate::chroma_sampling::take_sampling();
        if let (Some((u, v)), Some(clip_id)) =
            (crate::chroma_controls::frame_uv_from_click(rel, canvas, aspect), clip_id)
        {
            if let Some((r, g, b)) = sample_png_pixel(&path, u, v) {
                let hue = crate::chroma_controls::rgb_to_hue(r, g, b);
                // Swift commit resets tolerance/softness on sample (fresh key).
                let mut ctrls = crate::chroma_controls::ChromaControls::default().with_hue(hue);
                ctrls.enabled = true;
                let args = ctrls.apply_args(&[clip_id]);
                let hub = crate::editor_state_hub::EditorStateHub::global();
                if let Ok(mut exec) = hub.executor().lock() {
                    let _ = exec.execute("apply_effect", &args);
                }
            }
        }
        cx.notify();
        true
    }

    /// If the current playhead frame hasn't been composited yet, kick a
    /// background render of it to a cache PNG and show it when ready. Only one
    /// render runs at a time, so during playback the preview updates best-effort
    /// (the latest frame once the previous finishes) rather than flooding the
    /// background executor. Compose + decode + encode run off the UI thread, so
    /// this never blocks rendering.
    fn ensure_preview_frame(&mut self, cx: &mut Context<Self>) {
        if self.active_tab_idx != 0 || self.rendering {
            return;
        }
        let hub = crate::editor_state_hub::EditorStateHub::global();
        let key = (hub.revision(), self.state.active_frame);
        if self.rendered_key == Some(key) {
            return;
        }
        self.rendered_key = Some(key);
        self.rendering = true;
        let frame = self.state.active_frame;
        let (revision, _) = key;
        cx.spawn(async move |this, cx| {
            let rendered = cx
                .background_executor()
                .spawn(async move {
                    let hub = crate::editor_state_hub::EditorStateHub::global();
                    let (timeline, manifest, timelines, root) = {
                        let exec = hub.executor();
                        let guard = exec.lock().unwrap();
                        (
                            guard.timeline().clone(),
                            guard.media_manifest().clone(),
                            guard.sibling_timeline_map(),
                            hub.project_root()
                                .unwrap_or_else(|| std::env::temp_dir().join("fronda-preview")),
                        )
                    };
                    let cache_dir =
                        crate::project_registry_store::fronda_config_dir().join("preview");
                    let out =
                        crate::preview_render::preview_cache_path(&cache_dir, revision, frame);
                    if out.is_file() {
                        return Some(out);
                    }
                    let result = crate::preview_render::render_frame_png(
                        &timeline, &manifest, &timelines, &root, frame, &out,
                    )
                    .ok()
                    .map(|()| out);
                    // Bound the on-disk preview cache (reuses the thumbnail pruner).
                    crate::video_thumbnails::prune_by_size(&cache_dir, 64 * 1024 * 1024);
                    result
                })
                .await;
            let _ = this.update(cx, |view, cx| {
                view.rendering = false;
                match rendered {
                    Some(path) => view.frame_png = Some(path),
                    // A failed render must not poison the cache key — allow
                    // a retry on the next paint (review F5).
                    None => view.rendered_key = None,
                }
                cx.notify();
            });
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

    fn toggle_settings_menu(&mut self, menu: SettingsMenu, cx: &mut Context<Self>) {
        self.show_guide_menu = false;
        self.open_settings_menu = if self.open_settings_menu == Some(menu) {
            None
        } else {
            Some(menu)
        };
        cx.notify();
    }

    /// Show/hide the Guides dropdown (Issue #169). Closes the settings menu so
    /// the two anchored dropdowns never overlap.
    fn toggle_guide_menu(&mut self, cx: &mut Context<Self>) {
        self.show_guide_menu = !self.show_guide_menu;
        if self.show_guide_menu {
            self.open_settings_menu = None;
        }
        cx.notify();
    }

    /// Toggle one viewer guide on/off; the menu stays open for multi-select.
    fn toggle_guide(&mut self, guide: ViewerGuide, cx: &mut Context<Self>) {
        self.guide_state.toggle(guide);
        cx.notify();
    }

    /// Apply a settings-menu pick: zoom is view-local, everything else goes
    /// through the shared set_project_settings tool (rescale semantics, undo).
    fn apply_settings_selection(&mut self, select: &SettingsSelection, cx: &mut Context<Self>) {
        match select {
            SettingsSelection::Zoom(value) => self.canvas_zoom = *value,
            SettingsSelection::AspectRatio(label) => {
                Self::run_shared_tool(
                    "set_project_settings",
                    serde_json::json!({ "aspectRatio": label }),
                );
            }
            SettingsSelection::Fps(fps) => {
                Self::run_shared_tool("set_project_settings", serde_json::json!({ "fps": fps }));
            }
            SettingsSelection::Quality(label) => {
                Self::run_shared_tool(
                    "set_project_settings",
                    serde_json::json!({ "quality": label }),
                );
            }
        }
        self.open_settings_menu = None;
        cx.notify();
    }

    /// Run a tool on the shared executor; tool errors leave the UI unchanged.
    fn run_shared_tool(tool: &str, args: serde_json::Value) {
        let executor = crate::editor_state_hub::EditorStateHub::global().executor();
        let guard = executor.lock();
        if let Ok(mut exec) = guard {
            if let Err(reason) = exec.execute(tool, &args) {
                eprintln!("{tool} failed: {reason}");
            }
        }
    }

    /// Capture Frame (Swift: captureCurrentFrameToMedia): composite the current
    /// timeline frame off the UI thread, write the PNG into the project's
    /// media/ directory (ProjectMatteWriter file semantics), and register it as
    /// an image asset via import_media — which bumps the shared revision so
    /// views refresh.
    fn capture_frame(&mut self, cx: &mut Context<Self>) {
        if self.active_tab_idx != 0 {
            return;
        }
        let frame = self.state.active_frame;
        cx.spawn(async move |this, cx| {
            let result: Result<(), String> = cx
                .background_executor()
                .spawn(async move {
                    let hub = crate::editor_state_hub::EditorStateHub::global();
                    let (timeline, manifest, timelines) = {
                        let exec = hub.executor();
                        let guard = exec.lock().map_err(|_| "editor state lock poisoned")?;
                        (
                            guard.timeline().clone(),
                            guard.media_manifest().clone(),
                            guard.sibling_timeline_map(),
                        )
                    };
                    // Unsaved projects have no package yet; captures then land
                    // in a temp folder and register as external assets.
                    let root = hub
                        .project_root()
                        .unwrap_or_else(|| std::env::temp_dir().join("fronda-captures"));
                    let media_dir = root.join("media");
                    std::fs::create_dir_all(&media_dir)
                        .map_err(|e| format!("create media dir: {e}"))?;
                    let stamp = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_nanos())
                        .unwrap_or(0);
                    let out = media_dir.join(capture_file_name(frame, stamp));
                    crate::preview_render::render_frame_png(
                        &timeline, &manifest, &timelines, &root, frame, &out,
                    )?;
                    let exec = hub.executor();
                    let mut guard = exec.lock().map_err(|_| "editor state lock poisoned")?;
                    guard
                        .execute(
                            "import_media",
                            &serde_json::json!({
                                "source": { "path": out.to_string_lossy() },
                                "name": format!("Frame {frame}"),
                            }),
                        )
                        .map(|_| ())
                })
                .await;
            let _ = this.update(cx, |_, cx| {
                if let Err(reason) = result {
                    eprintln!("Capture frame failed: {reason}");
                }
                cx.notify();
            });
        })
        .detach();
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
        self.ensure_envelope(cx);
        let meter = self.update_meter();
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
        let guide_menu_open = self.show_guide_menu;
        let guide_rows = guide_menu_rows(&self.guide_state);

        // Real project settings for the badge row (was hardcoded).
        let (aspect_label, fps_label, quality_label, tl_width, tl_height, tl_fps) = {
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
            (
                timeline_core::format_aspect_ratio(w, h),
                format!("{fps} fps"),
                quality,
                w,
                h,
                fps,
            )
        };
        let canvas_zoom = self.canvas_zoom;
        let zoom_label = zoom_badge_label(canvas_zoom);
        let open_menu = self.open_settings_menu;
        div()
            .id("preview-panel")
            .flex()
            .flex_col()
            .relative()
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
                    .overflow_hidden()
                    .bg(Background::BASE)
                    // Capture the canvas bounds each paint so the chroma
                    // eyedropper can map a click to a frame pixel.
                    .child({
                        let slot = self.canvas_bounds.clone();
                        canvas(
                            move |bounds, _, _| {
                                if let Ok(mut b) = slot.lock() {
                                    *b = Some(bounds);
                                }
                            },
                            |_, _, _, _| {},
                        )
                        .absolute()
                        .size_full()
                    })
                    // While the eyedropper is armed, a click samples the frame.
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, e: &MouseDownEvent, _, cx| {
                            this.try_sample_chroma(e.position, cx);
                        }),
                    )
                    // Composited frame (or placeholder) — hidden when an overlay is
                    // shown. The zoom wrapper scales the fitted frame by
                    // canvas_zoom (Swift: fitSize * editor.canvasZoom, clipped).
                    .when(
                        canvas_overlay == CanvasOverlay::None,
                        |el| match &frame_png {
                            Some(path) => el.child(
                                div()
                                    .w(gpui::relative(canvas_zoom as f32))
                                    .h(gpui::relative(canvas_zoom as f32))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .flex_none()
                                    .child(
                                        gpui::img(path.clone())
                                            .max_w_full()
                                            .max_h_full()
                                            .object_fit(gpui::ObjectFit::Contain),
                                    ),
                            ),
                            None => el.child(
                                div()
                                    .text_color(Text::MUTED)
                                    .text_size(px(FontSize::SM))
                                    .child("Preview"),
                            ),
                        },
                    )
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
                            )
                            .child(div().pl(px(Spacing::MD)).child(render_audio_meter(meter))),
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
                            // Badges reflect the live project settings and open
                            // their preset menus (Swift: projectSettingsGroup)
                            .child(settings_badge("badge-aspect", &aspect_label).on_click(
                                cx.listener(|this, _, _, cx| {
                                    this.toggle_settings_menu(SettingsMenu::Aspect, cx);
                                }),
                            ))
                            .child(settings_badge("badge-fps", &fps_label).on_click(cx.listener(
                                |this, _, _, cx| {
                                    this.toggle_settings_menu(SettingsMenu::Fps, cx);
                                },
                            )))
                            .child(settings_badge("badge-quality", &quality_label).on_click(
                                cx.listener(|this, _, _, cx| {
                                    this.toggle_settings_menu(SettingsMenu::Quality, cx);
                                }),
                            ))
                            .child(settings_badge("badge-fit", &zoom_label).on_click(
                                cx.listener(|this, _, _, cx| {
                                    this.toggle_settings_menu(SettingsMenu::Zoom, cx);
                                }),
                            ))
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
                                    .child("⊞")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.toggle_guide_menu(cx);
                                    })),
                            )
                            // Capture frame button (Swift: captureFrameButton → camera SF symbol)
                            .child(
                                transport_btn_svg("btn-capture-frame", "icons/camera.svg", false)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.capture_frame(cx);
                                    })),
                            ),
                    ),
            )
            // Settings dropdown — anchored above the transport bar's badges
            .when_some(open_menu, |el, menu| {
                let rows = settings_menu_rows(menu, tl_width, tl_height, tl_fps, canvas_zoom);
                let mut panel = div()
                    .id("preview-settings-menu")
                    .occlude()
                    .absolute()
                    .bottom(px(36.0 + Spacing::XS))
                    .right(px(Spacing::MD))
                    .flex()
                    .flex_col()
                    .py(px(Spacing::XS))
                    .min_w(px(120.0))
                    .bg(Background::RAISED)
                    .border_1()
                    .border_color(BorderColors::SUBTLE)
                    .rounded(px(Radius::SM))
                    .shadow_lg()
                    .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                        this.open_settings_menu = None;
                        cx.notify();
                    }));
                for (i, row) in rows.into_iter().enumerate() {
                    let select = row.select.clone();
                    panel = panel.child(
                        div()
                            .id(("preview-settings-row", i))
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(Spacing::SM))
                            .px(px(Spacing::MD))
                            .py(px(Spacing::XS))
                            .cursor_pointer()
                            .hover(|s| s.bg(Background::PROMINENT))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.apply_settings_selection(&select, cx);
                            }))
                            .child(
                                div()
                                    .w(px(crate::theme::IconSize::XXS))
                                    .text_size(px(FontSize::XS))
                                    .text_color(Accent::PRIMARY)
                                    .child(if row.checked { "✓" } else { "" }),
                            )
                            .child(
                                div()
                                    .text_size(px(FontSize::SM))
                                    .text_color(Text::SECONDARY)
                                    .child(row.label),
                            ),
                    );
                }
                el.child(panel)
            })
            // Guides dropdown — SMPTE safe zones, center cross, format references
            // (Swift ViewerGuideMenu). Anchored above the transport bar (Issue #169).
            .when(guide_menu_open, |el| {
                let mut panel = div()
                    .id("preview-guide-menu")
                    .occlude()
                    .absolute()
                    .bottom(px(36.0 + Spacing::XS))
                    .right(px(Spacing::MD))
                    .flex()
                    .flex_col()
                    .py(px(Spacing::XS))
                    .min_w(px(160.0))
                    .bg(Background::RAISED)
                    .border_1()
                    .border_color(BorderColors::SUBTLE)
                    .rounded(px(Radius::SM))
                    .shadow_lg()
                    .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                        this.show_guide_menu = false;
                        cx.notify();
                    }));
                for (i, row) in guide_rows.into_iter().enumerate() {
                    let guide = row.guide;
                    panel = panel.child(
                        div()
                            .id(("preview-guide-row", i))
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(Spacing::SM))
                            .px(px(Spacing::MD))
                            .py(px(Spacing::XS))
                            .cursor_pointer()
                            .hover(|s| s.bg(Background::PROMINENT))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.toggle_guide(guide, cx);
                            }))
                            .child(
                                div()
                                    .w(px(crate::theme::IconSize::XXS))
                                    .text_size(px(FontSize::XS))
                                    .text_color(Accent::PRIMARY)
                                    .child(if row.checked { "✓" } else { "" }),
                            )
                            .child(
                                div()
                                    .text_size(px(FontSize::SM))
                                    .text_color(Text::SECONDARY)
                                    .child(row.label),
                            ),
                    );
                }
                el.child(panel)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aspect_rows_mirror_presets_and_mark_active() {
        let rows = settings_menu_rows(SettingsMenu::Aspect, 1920, 1080, 30, 1.0);
        assert_eq!(rows.len(), timeline_core::ASPECT_PRESETS.len());
        assert_eq!(rows[0].label, "16:9");
        assert!(rows[0].checked, "1920x1080 marks 16:9 active");
        assert!(rows.iter().filter(|r| r.checked).count() == 1);
        assert_eq!(rows[0].select, SettingsSelection::AspectRatio("16:9"));
    }

    #[test]
    fn fps_rows_mark_current_rate() {
        let rows = settings_menu_rows(SettingsMenu::Fps, 1920, 1080, 25, 1.0);
        let active: Vec<_> = rows.iter().filter(|r| r.checked).collect();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].label, "25 fps");
        assert_eq!(active[0].select, SettingsSelection::Fps(25));
    }

    #[test]
    fn quality_rows_match_on_short_edge() {
        let rows = settings_menu_rows(SettingsMenu::Quality, 1080, 1920, 30, 1.0);
        let active: Vec<_> = rows.iter().filter(|r| r.checked).collect();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].label, "1080p");
        assert_eq!(active[0].select, SettingsSelection::Quality("1080p"));
    }

    #[test]
    fn zoom_rows_mark_fit_at_one() {
        let rows = settings_menu_rows(SettingsMenu::Zoom, 1920, 1080, 30, 1.0);
        let active: Vec<_> = rows.iter().filter(|r| r.checked).collect();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].label, "Fit");
        let rows = settings_menu_rows(SettingsMenu::Zoom, 1920, 1080, 30, 0.5);
        let active: Vec<_> = rows.iter().filter(|r| r.checked).collect();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].select, SettingsSelection::Zoom(0.5));
    }

    #[test]
    fn zoom_badge_label_matches_swift() {
        assert_eq!(zoom_badge_label(1.0), "Fit");
        assert_eq!(zoom_badge_label(1.005), "Fit"); // within 0.01 tolerance
        assert_eq!(zoom_badge_label(0.25), "25%");
        assert_eq!(zoom_badge_label(2.0), "200%");
    }

    #[test]
    fn capture_file_name_is_frame_and_stamp_keyed() {
        assert_eq!(capture_file_name(42, 7), "frame-42-7.png");
        assert_ne!(capture_file_name(42, 7), capture_file_name(42, 8));
        assert_ne!(capture_file_name(42, 7), capture_file_name(43, 7));
    }

    #[test]
    fn guide_menu_rows_reflect_state() {
        let mut st = ViewerGuideState::new();
        let rows = guide_menu_rows(&st);
        assert_eq!(rows.len(), 7);
        assert_eq!(rows[0].guide, ViewerGuide::ActionSafe);
        assert_eq!(rows[0].label, "Action Safe");
        assert!(rows.iter().all(|r| !r.checked), "none active initially");
        st.toggle(ViewerGuide::TitleSafe);
        let rows = guide_menu_rows(&st);
        assert!(
            rows.iter().find(|r| r.guide == ViewerGuide::TitleSafe).unwrap().checked,
            "toggled guide is checked"
        );
        assert!(
            !rows.iter().find(|r| r.guide == ViewerGuide::ActionSafe).unwrap().checked,
            "others stay unchecked"
        );
    }
}

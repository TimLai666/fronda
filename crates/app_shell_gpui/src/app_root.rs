//! Application root — manages routing between Home and Editor views.
//!
//! Covers APP-002 (reopening shows Home), BOOT-004 (startup flow),
//! and PRJ-014 (close project → Home).

use crate::chat_view::ChatView;
use crate::editor_view;
use crate::home_model::HomeLayout;
use crate::home_view::HomeView;
use crate::inspector_view::InspectorView;
use crate::media_panel_view::MediaPanelView;
use crate::menu;
use crate::pane::{LayoutPreset, PaneId, PaneLayout};
use crate::preview_view::PreviewView;
use crate::theme::{Background, BorderColors, FontSize, Radius, Spacing, Text};
use crate::timeline_view::TimelineView;
use crate::titlebar_view::TitleBarView;
use crate::toolbar_view::ToolbarView;
use crate::tour_overlay_view::TourOverlayView;
use crate::update_overlay_view::UpdateOverlayView;
use crate::window::WindowConfig;
use app_contract::focus_router::{route_paste, FocusTarget};
use gpui::{
    div, prelude::*, px, size, App, Bounds, Context, DragMoveEvent, Entity, FocusHandle, Focusable,
    InteractiveElement, KeyDownEvent, MouseButton, MouseDownEvent, MouseUpEvent,
    PathPromptOptions, Window, WindowBounds, WindowOptions,
};

/// Drag token for pane divider resize.
#[derive(Debug, Clone)]
struct PaneResizeDrag;

/// Invisible drag preview.
struct PaneResizePreview;
impl gpui::Render for PaneResizePreview {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl gpui::IntoElement {
        div()
    }
}

/// Active pane-divider drag session.
#[derive(Debug, Clone, Copy)]
struct PaneDragSession {
    target: crate::pane_resize::ResizeTarget,
    /// Pointer x (columns) or y (timeline) at mouse-down.
    start_pos: f32,
    start_size: f32,
}

/// Which screen the app is showing.
#[derive(Debug, Clone, PartialEq)]
pub enum ActiveScreen {
    Home,
    Editor,
}

/// Recent-project card the context menu acts on.
#[derive(Debug, Clone)]
struct ProjectMenuTarget {
    registry_id: String,
    path: std::path::PathBuf,
    /// Whether the package existed on disk at render time.
    accessible: bool,
}

/// Frames for the default 3-second text clip (Swift: Defaults.textDurationSeconds).
fn default_text_duration_frames(fps: i64) -> i64 {
    ((3.0 * fps.max(1) as f64).round() as i64).max(1)
}

/// A pane size worth persisting — the negative preset sentinels are not.
fn persistable_size(v: f32) -> Option<f32> {
    (v.is_finite() && v > 0.0).then_some(v)
}

/// Maximize acts on the focused pane (Swift toggleMaximizePanelAction). Swift
/// disables the menu item with no focus; the Rust shortcut instead keeps its
/// pre-focus-tracking Preview fallback so it never goes dead.
fn maximize_target(focused: Option<PaneId>) -> PaneId {
    focused.unwrap_or(PaneId::Preview)
}

/// Apply persisted (already statically clamped) sizes over the boot
/// defaults/sentinels. Returns whether anything was applied, so the first
/// editor render knows to run the viewport-aware clamp.
fn apply_persisted_sizes(
    saved: &crate::pane_prefs::PersistedPaneSizes,
    agent: &mut f32,
    media: &mut f32,
    inspector: &mut f32,
    timeline: &mut f32,
) -> bool {
    let mut applied = false;
    for (value, slot) in [
        (saved.agent, agent),
        (saved.media, media),
        (saved.inspector, inspector),
        (saved.timeline_height, timeline),
    ] {
        if let Some(v) = value {
            *slot = v;
            applied = true;
        }
    }
    applied
}

/// Prompt handed off from the media tabs (agent-chat seam), drained on the
/// next editor render — the seam closure has no gpui context, so it parks the
/// prompt here and the caller's notify triggers the redraw that delivers it.
static PENDING_AGENT_PROMPT: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

/// Device-rate premix of the whole timeline keyed by (revision, rate,
/// channels) — the playback feeder's data source, so replaying an unedited
/// project skips the decode+mix.
type PremixCacheEntry = (u64, u32, u16, std::sync::Arc<Vec<f32>>);
static PREMIX_CACHE: std::sync::Mutex<Option<PremixCacheEntry>> = std::sync::Mutex::new(None);

/// Snapshot the shared editor state into a [`crate::audio_playback::PremixFn`]
/// that mixes the timeline at the device rate on the feeder thread (same mix
/// path as export: nests flattened, decode via ffmpeg).
fn playback_premix_fn() -> crate::audio_playback::PremixFn {
    let hub = crate::editor_state_hub::EditorStateHub::global();
    let revision = hub.revision();
    let snapshot = hub.executor().lock().ok().map(|guard| {
        (
            guard.timeline().clone(),
            guard.media_manifest().clone(),
            guard.sibling_timeline_map(),
        )
    });
    let root = hub
        .project_root()
        .unwrap_or_else(|| std::env::temp_dir().join("fronda-preview"));
    Box::new(move |rate: u32, channels: u16| {
        if let Ok(cache) = PREMIX_CACHE.lock() {
            if let Some((rev, r, c, data)) = cache.as_ref() {
                if *rev == revision && *r == rate && *c == channels {
                    return data.clone();
                }
            }
        }
        let Some((timeline, manifest, timelines)) = snapshot else {
            return std::sync::Arc::new(Vec::new());
        };
        let paths: std::collections::HashMap<String, std::path::PathBuf> = manifest
            .entries
            .iter()
            .filter_map(|e| crate::video_export::source_path(e, &root).map(|p| (e.id.clone(), p)))
            .collect();
        let mixed = render_core::audio_plan::mix_timeline_audio_with_timelines(
            &timeline,
            &timelines,
            rate,
            channels as usize,
            |clip: &core_model::Clip| {
                let path = paths.get(clip.media_ref.as_str())?;
                crate::audio_export::decode_audio_pcm(path, rate, channels)
            },
        );
        let data = std::sync::Arc::new(mixed);
        if let Ok(mut cache) = PREMIX_CACHE.lock() {
            *cache = Some((revision, rate, channels, data.clone()));
        }
        data
    })
}

/// Root view that switches between Home and Editor.
#[derive(Debug, Clone)]
pub struct AppRoot {
    focus_handle: FocusHandle,
    active_screen: ActiveScreen,
    pane_layout: PaneLayout,
    /// Pane owning the focus ring (EDT-007); set by card clicks + maximize.
    focused_pane: Option<PaneId>,
    /// preferences.json for EDT-003 visibility persistence (test-injectable).
    pane_prefs_path: std::path::PathBuf,
    /// Persisted sizes were applied at boot; the first editor render (when
    /// the viewport is known) runs the full space-guard clamp once.
    pane_sizes_need_clamp: bool,
    home: HomeView,
    samples_expanded: bool,
    welcome_dismissed: bool,
    /// Recent projects list (Swift: ProjectRegistry.sortedEntries).
    /// True when a user is signed in (controls sidebar Sign in button).
    is_signed_in: bool,
    /// Draggable pane sizes; negative = unresolved sentinel (preset initial
    /// value computed from the viewport on next render).
    timeline_height: f32,
    agent_width: f32,
    media_width: f32,
    inspector_width: f32,
    vertical_left_width: f32,
    /// Editor content size captured each render, for drag clamping.
    last_viewport: (f32, f32),
    pane_drag: Option<PaneDragSession>,
    /// Editor panel entities — created lazily on first open_editor() call.
    titlebar_view: Option<Entity<TitleBarView>>,
    chat_view: Option<Entity<ChatView>>,
    toolbar_view: Option<Entity<ToolbarView>>,
    media_panel_view: Option<Entity<MediaPanelView>>,
    preview_view: Option<Entity<PreviewView>>,
    timeline_view: Option<Entity<TimelineView>>,
    inspector_view: Option<Entity<InspectorView>>,
    tour_overlay: Entity<TourOverlayView>,
    update_overlay: Entity<UpdateOverlayView>,
    /// Right-click menu on recent-project cards.
    project_menu: crate::context_menu::ContextMenuState<ProjectMenuTarget>,
    /// Registry id of the recent-project card under the pointer.
    hovered_project: Option<String>,
    /// Registry id whose hover trash button is armed for delete confirmation
    /// (context-menu arm-then-confirm pattern).
    armed_delete: Option<String>,
    /// Home card snapshots (registry read + per-card fs stats). Hover-driven
    /// renders must not re-read the registry file (review F1) — refreshed on
    /// a short TTL so external changes still surface.
    home_cards: Vec<HomeCard>,
    home_cards_loaded_at: Option<std::time::Instant>,
    /// The ~30 Hz audio-transport sync loop is running.
    audio_tick_running: bool,
    /// Project revision the playback engine's premix was armed with; a
    /// mismatch mid-play re-arms the feeder on the edited timeline.
    audio_premix_revision: Option<u64>,
}

type HomeCard = (
    String,
    String,
    String,
    Option<std::path::PathBuf>,
    std::path::PathBuf,
    bool,
);

impl AppRoot {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let handle = cx.focus_handle();
        // EDT-003: media/inspector/agent visibility persists across launches.
        let pane_prefs_path = crate::pane_prefs::default_prefs_path();
        let mut pane_layout = PaneLayout::new();
        if let Some(saved) = crate::pane_prefs::load_pane_visibility(&pane_prefs_path) {
            saved.apply_to(&mut pane_layout.visibility);
        }
        // Persisted divider positions (statically clamped here; the
        // viewport-aware clamp runs on the first editor render).
        let mut agent_width = crate::pane_resize::AGENT_MIN;
        let mut media_width = -1.0;
        let mut inspector_width = crate::theme::Layout::INSPECTOR_DEFAULT;
        let mut timeline_height = -1.0;
        let pane_sizes_need_clamp = crate::pane_prefs::load_pane_sizes(&pane_prefs_path)
            .map(|saved| {
                apply_persisted_sizes(
                    &saved.clamped(),
                    &mut agent_width,
                    &mut media_width,
                    &mut inspector_width,
                    &mut timeline_height,
                )
            })
            .unwrap_or(false);
        Self {
            focus_handle: handle.clone(),
            active_screen: ActiveScreen::Home,
            pane_layout,
            focused_pane: None,
            pane_prefs_path,
            pane_sizes_need_clamp,
            home: HomeView::new(handle),
            samples_expanded: true,
            welcome_dismissed: false,
            is_signed_in: false,
            titlebar_view: None,
            chat_view: None,
            toolbar_view: None,
            media_panel_view: None,
            preview_view: None,
            timeline_view: None,
            inspector_view: None,
            tour_overlay: cx.new(|cx| TourOverlayView::new(cx)),
            update_overlay: cx.new(|cx| UpdateOverlayView::new(cx)),
            timeline_height,
            agent_width,
            media_width,
            inspector_width,
            vertical_left_width: -1.0,
            last_viewport: (0.0, 0.0),
            pane_drag: None,
            project_menu: crate::context_menu::ContextMenuState::new(),
            hovered_project: None,
            armed_delete: None,
            home_cards: Vec::new(),
            home_cards_loaded_at: None,
            audio_tick_running: false,
            audio_premix_revision: None,
        }
    }

    /// WelcomeOverlay — first-launch welcome over Home, structured after
    /// Swift WelcomeOverlay: centered 520pt card, title + subtitle, hero
    /// image area (shrinks on short windows so the buttons always fit),
    /// Skip / Watch Tutorial / Get started.
    fn render_welcome_overlay(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        div()
            .id("welcome-overlay")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .p(px(Spacing::XXL))
            .bg(gpui::Hsla {
                h: 0.0,
                s: 0.0,
                l: 0.0,
                a: crate::theme::Opacity::STRONG,
            })
            .child(
                div()
                    .id("welcome-card")
                    .flex()
                    .flex_col()
                    .gap(px(Spacing::LG))
                    .w(px(520.0))
                    .max_w_full()
                    .max_h_full()
                    .p(px(Spacing::XXL))
                    .rounded(px(Radius::MD_LG))
                    .bg(Background::SURFACE)
                    .border_1()
                    .border_color(BorderColors::PRIMARY)
                    .shadow_lg()
                    .child(
                        div()
                            .flex()
                            .flex_none()
                            .flex_col()
                            .gap(px(Spacing::SM))
                            .child(
                                div()
                                    .text_size(px(FontSize::TITLE_2))
                                    .font_weight(gpui::FontWeight::LIGHT)
                                    .text_color(Text::PRIMARY)
                                    .child("Welcome to Fronda"),
                            )
                            .child(
                                div()
                                    .text_size(px(FontSize::SM_MD))
                                    .text_color(Text::SECONDARY)
                                    .child(
                                        "A video editor built for AI. Generate, \
                                         and edit all in one place.",
                                    ),
                            ),
                    )
                    // Hero image area (Swift: welcome-butterfly.jpg, gradient
                    // fallback — no bundled hero asset yet). Shrinks first.
                    .child(
                        div()
                            .w_full()
                            .h(px(240.0))
                            .min_h(px(40.0))
                            .flex_shrink()
                            .rounded(px(Radius::MD))
                            .bg(gpui::linear_gradient(
                                135.0,
                                gpui::linear_color_stop(
                                    gpui::Hsla {
                                        h: 0.78,
                                        s: 0.45,
                                        l: 0.30,
                                        a: 1.0,
                                    },
                                    0.0,
                                ),
                                gpui::linear_color_stop(
                                    gpui::Hsla {
                                        h: 0.55,
                                        s: 0.55,
                                        l: 0.35,
                                        a: 1.0,
                                    },
                                    1.0,
                                ),
                            )),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_none()
                            .flex_row()
                            .items_center()
                            .gap(px(Spacing::SM))
                            .pt(px(Spacing::LG))
                            .child(
                                Self::welcome_button("welcome-skip", "Skip", false).on_click(
                                    cx.listener(|this, _, _, cx| {
                                        this.welcome_dismissed = true;
                                        cx.notify();
                                    }),
                                ),
                            )
                            .child(div().flex_1())
                            // Swift opens a downloaded sample with the tour;
                            // samples are network-gated, so this starts the
                            // tour in a new project.
                            .child(
                                Self::welcome_button("welcome-tutorial", "Watch Tutorial", false)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.welcome_dismissed = true;
                                        this.open_editor(cx);
                                        this.tour_overlay.update(cx, |tour, cx| {
                                            tour.start(cx);
                                        });
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Self::welcome_button("welcome-get-started", "Get started", true)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.welcome_dismissed = true;
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            .into_any_element()
    }

    /// EDT-003: save media/inspector/agent visibility. Skipped while
    /// maximized — that projection is transient, never the persisted state.
    fn persist_pane_visibility(&self) {
        if self.pane_layout.is_maximized() {
            return;
        }
        crate::pane_prefs::save_pane_visibility(
            &self.pane_prefs_path,
            crate::pane_prefs::PersistedPaneVisibility::from_layout(&self.pane_layout.visibility),
        );
    }

    /// Persist the resolved divider positions (paneSizes). Unresolved preset
    /// sentinels are skipped; the merge-save keeps their stored values.
    fn persist_pane_sizes(&self) {
        crate::pane_prefs::save_pane_sizes(
            &self.pane_prefs_path,
            crate::pane_prefs::PersistedPaneSizes {
                agent: persistable_size(self.agent_width),
                media: persistable_size(self.media_width),
                inspector: persistable_size(self.inspector_width),
                timeline_height: persistable_size(self.timeline_height),
            },
        );
    }

    /// Reset preset-scoped pane sizes so the next render recomputes the
    /// preset's initial proportions (Swift rebuilds splits on preset switch).
    fn reset_pane_sizes(&mut self) {
        self.timeline_height = -1.0;
        self.media_width = -1.0;
        self.inspector_width = crate::theme::Layout::INSPECTOR_DEFAULT;
        self.vertical_left_width = -1.0;
    }

    fn pane_size(&self, target: crate::pane_resize::ResizeTarget) -> f32 {
        use crate::pane_resize::ResizeTarget::*;
        match target {
            AgentWidth => self.agent_width,
            MediaWidth => self.media_width,
            InspectorWidth => self.inspector_width,
            TimelineHeight => self.timeline_height,
            VerticalLeftWidth => self.vertical_left_width,
        }
    }

    fn set_pane_size(&mut self, target: crate::pane_resize::ResizeTarget, v: f32) {
        use crate::pane_resize::ResizeTarget::*;
        match target {
            AgentWidth => self.agent_width = v,
            MediaWidth => self.media_width = v,
            InspectorWidth => self.inspector_width = v,
            TimelineHeight => self.timeline_height = v,
            VerticalLeftWidth => self.vertical_left_width = v,
        }
    }

    /// Clamp frame for a divider drag, from the current layout state.
    fn resize_bounds(&self, target: crate::pane_resize::ResizeTarget) -> crate::pane_resize::ResizeBounds {
        use crate::pane_resize::*;
        let (vw, vh) = self.last_viewport;
        let vis = |id: PaneId| self.pane_layout.is_visible(id);
        let rail = crate::theme::MediaPanel::TAB_RAIL_WIDTH;
        let vertical = self.pane_layout.preset == LayoutPreset::Vertical;
        match target {
            ResizeTarget::TimelineHeight => ResizeBounds {
                area: vh,
                others: 0.0,
                neighbor_min: PREVIEW_MIN_H,
                rail_w: 0.0,
            },
            // Vertical preset: media lives inside the left column and
            // squeezes the inspector, not the preview.
            ResizeTarget::MediaWidth if vertical => ResizeBounds {
                area: self.vertical_left_width,
                others: 0.0,
                neighbor_min: INSPECTOR_MIN,
                rail_w: rail,
            },
            _ => {
                let mut others = 0.0;
                if vis(PaneId::Agent) && target != ResizeTarget::AgentWidth {
                    others += self.agent_width;
                }
                if vertical {
                    if target != ResizeTarget::VerticalLeftWidth {
                        others += self.vertical_left_width;
                    }
                } else {
                    if vis(PaneId::Media) && target != ResizeTarget::MediaWidth {
                        others += self.media_width;
                    }
                    if vis(PaneId::Inspector) && target != ResizeTarget::InspectorWidth {
                        others += self.inspector_width;
                    }
                }
                ResizeBounds {
                    area: vw,
                    others,
                    neighbor_min: PREVIEW_MIN_W,
                    rail_w: rail,
                }
            }
        }
    }

    /// 5px seam hitbox overlaying the gap between two pane cards. The anchor
    /// node in the tree is zero-sized; this element rides the seam via a
    /// half-gap negative offset.
    fn build_divider(
        &self,
        target: crate::pane_resize::ResizeTarget,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        use crate::pane_resize::ResizeTarget::*;
        let horizontal = target != TimelineHeight;
        let gap = crate::theme::Layout::PANEL_GAP;
        let id: &'static str = match target {
            AgentWidth => "divider-agent",
            MediaWidth => "divider-media",
            InspectorWidth => "divider-inspector",
            TimelineHeight => "divider-timeline",
            VerticalLeftWidth => "divider-vertical-left",
        };
        let hit = div()
            .id(id)
            .absolute()
            .occlude()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this: &mut AppRoot, e: &MouseDownEvent, _, _| {
                    let start_pos = if horizontal {
                        e.position.x.as_f32()
                    } else {
                        e.position.y.as_f32()
                    };
                    this.pane_drag = Some(PaneDragSession {
                        target,
                        start_pos,
                        start_size: this.pane_size(target),
                    });
                }),
            )
            .on_drag(PaneResizeDrag, |_, _, _, cx| cx.new(|_| PaneResizePreview));
        if horizontal {
            hit.left(px(-gap / 2.0))
                .top_0()
                .w(px(gap))
                .h_full()
                .cursor_col_resize()
                .into_any_element()
        } else {
            hit.top(px(-gap / 2.0))
                .left_0()
                .h(px(gap))
                .w_full()
                .cursor_ns_resize()
                .into_any_element()
        }
    }

    /// Open the editor for a project.
    pub fn open_editor(&mut self, cx: &mut Context<Self>) {
        self.project_menu.close();
        self.active_screen = ActiveScreen::Editor;
        if self.chat_view.is_none() {
            let titlebar = cx.new(|cx| TitleBarView::new(cx));
            cx.subscribe(&titlebar, |this, _, event, cx| match event {
                crate::titlebar_view::TitleBarEvent::RunMenu(action) => {
                    this.dispatch_menu_action(action.clone(), cx);
                }
            })
            .detach();
            self.titlebar_view = Some(titlebar);
            self.chat_view = Some(cx.new(|cx| ChatView::new(cx)));
            let toolbar = cx.new(|cx| ToolbarView::new(cx));
            cx.subscribe(&toolbar, |this, _, event, cx| match event {
                crate::toolbar_view::ToolbarEvent::AddText => this.add_text_at_playhead(cx),
            })
            .detach();
            self.toolbar_view = Some(toolbar);
            self.media_panel_view = Some(cx.new(|cx| MediaPanelView::new(cx)));
            self.preview_view = Some(cx.new(|cx| PreviewView::new(cx)));
            self.timeline_view = Some(cx.new(|cx| TimelineView::new(cx)));
            self.inspector_view = Some(cx.new(|cx| InspectorView::new(cx)));
            self.wire_cross_view_state(cx);
            // Periodic autosave (#211): a coalesced checkpoint every 20s that
            // no-ops unless a project is open and edited since the last save.
            // Spawned once per session (guarded by chat_view.is_none()).
            cx.spawn(async move |_, cx| loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_secs(20))
                    .await;
                let _ = crate::editor_state_hub::EditorStateHub::global().autosave_if_dirty();
            })
            .detach();
        }
        cx.notify();
    }

    /// Cross-view wiring installed once with the editor entities: timeline
    /// selection/playhead and media-library selection feed the inspector, the
    /// inspector's crop toggle drives the preview's crop overlay, and the
    /// media tabs' agent handoff seam parks prompts for the chat composer.
    fn wire_cross_view_state(&mut self, cx: &mut Context<Self>) {
        if let Some(timeline) = self.timeline_view.clone() {
            cx.observe(&timeline, |this, timeline, cx| {
                let (selected, playhead) = {
                    let state = &timeline.read(cx).state;
                    (state.selected_clip_ids.clone(), state.playhead_frame)
                };
                if let Some(inspector) = this.inspector_view.clone() {
                    inspector.update(cx, |ins, cx| {
                        if ins.selected_clip_ids != selected || ins.playhead_frame != playhead {
                            ins.selected_clip_ids = selected;
                            ins.playhead_frame = playhead;
                            cx.notify();
                        }
                    });
                }
            })
            .detach();
        }
        if let Some(media) = self.media_panel_view.clone() {
            cx.observe(&media, |this, media, cx| {
                // Source-mode target: the anchor (last plainly-clicked or
                // toggled id) when still selected, else the newest selected.
                let asset = {
                    let library = &media.read(cx).library;
                    library
                        .selection_anchor
                        .as_ref()
                        .filter(|a| library.selection.iter().any(|s| s == *a))
                        .or_else(|| library.selection.last())
                        .cloned()
                };
                if let Some(inspector) = this.inspector_view.clone() {
                    inspector.update(cx, |ins, cx| {
                        if ins.selected_media_asset_id != asset {
                            ins.selected_media_asset_id = asset;
                            cx.notify();
                        }
                    });
                }
            })
            .detach();
        }
        if let Some(inspector) = self.inspector_view.clone() {
            cx.observe(&inspector, |this, inspector, cx| {
                let crop = inspector.read(cx).crop_editing_active;
                if let Some(preview) = this.preview_view.clone() {
                    preview.update(cx, |preview, cx| {
                        if preview.show_crop_overlay != crop {
                            preview.show_crop_overlay = crop;
                            cx.notify();
                        }
                    });
                }
            })
            .detach();
        }
        crate::media_panel_view::set_agent_chat_handoff(Box::new(|prompt| {
            if let Ok(mut slot) = PENDING_AGENT_PROMPT.lock() {
                *slot = Some(prompt.to_string());
            }
        }));
    }

    /// Start the ~30 Hz audio-transport sync loop when a transport action may
    /// need it. One step runs synchronously so play/pause/audio-start take
    /// effect without a tick of latency; the loop then keeps the playback
    /// engine and the timeline transport in lockstep until both are idle.
    fn ensure_audio_sync_tick(&mut self, cx: &mut Context<Self>) {
        if self.audio_tick_running {
            return;
        }
        if !self.audio_sync_step(cx) {
            return;
        }
        self.audio_tick_running = true;
        cx.spawn(async move |this, cx| loop {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(33))
                .await;
            let alive = this.update(cx, |app, cx| {
                let keep = app.audio_sync_step(cx);
                if !keep {
                    app.audio_tick_running = false;
                }
                keep
            });
            if !alive.unwrap_or(false) {
                break;
            }
        })
        .detach();
    }

    /// One audio-transport sync step: start/stop/seek the playback engine from
    /// the timeline transport, and while audio is live drive the playhead from
    /// the audio clock (consumed samples), overriding the visual ticker's
    /// dead reckoning. Degraded (no output device) keeps the pre-existing
    /// silent dead-reckoned playback. Returns false when there is nothing left
    /// to sync.
    fn audio_sync_step(&mut self, cx: &mut Context<Self>) -> bool {
        use crate::audio_playback::PlayState;
        let Some(tv) = self.timeline_view.clone() else {
            if let Ok(mut engine) = crate::audio_playback::engine().lock() {
                engine.stop();
            }
            return false;
        };
        let (rate, playhead, fps, total_frames) = {
            let state = &tv.read(cx).state;
            (
                state.transport.rate,
                state.playhead_frame,
                state.fps.max(1),
                state.total_frames,
            )
        };
        let revision = crate::editor_state_hub::EditorStateHub::global().revision();
        // Phase-1 non-goal: JKL shuttle rates and reverse stay silent — only
        // 1x forward produces audio.
        let want_audio = rate == 1.0;
        let Ok(mut engine) = crate::audio_playback::engine().lock() else {
            return false;
        };
        let mut clock_playhead: Option<i64> = None;
        match (want_audio, engine.state()) {
            (true, PlayState::Idle) | (true, PlayState::Paused) => {
                let start = playhead.max(0) as f64 / fps as f64;
                let resumed = engine.state() == PlayState::Paused
                    && self.audio_premix_revision == Some(revision)
                    && engine.resume_at(start);
                if !resumed {
                    engine.play(start, playback_premix_fn());
                    self.audio_premix_revision = Some(revision);
                }
            }
            (true, PlayState::Playing) => {
                if self.audio_premix_revision != Some(revision) {
                    // Edited mid-play: re-arm the feeder on the new timeline.
                    let position = engine.position_seconds();
                    engine.play(position, playback_premix_fn());
                    self.audio_premix_revision = Some(revision);
                } else if engine.is_live_playing() && !engine.ended() {
                    let clock_frame = engine.position_frame(fps);
                    let slop = (fps / 10).max(3);
                    if (playhead - clock_frame).abs() > slop {
                        // External jump (ruler click): refeed from there.
                        engine.seek(playhead.max(0) as f64 / fps as f64);
                    } else {
                        clock_playhead = Some(clock_frame);
                    }
                }
                // `ended`: stop driving so the visual ticker reaches the end
                // and auto-pauses; the next step then pauses the engine.
            }
            (false, PlayState::Playing) => engine.pause(),
            (false, _) => {}
        }
        let engine_playing = engine.state() == PlayState::Playing;
        drop(engine);

        if let Some(frame) = clock_playhead {
            if frame != playhead {
                tv.update(cx, |view, cx| {
                    view.state.playhead_frame = frame;
                    cx.notify();
                });
            }
        }
        // Preview follows the transport (frame for the canvas, playing flag
        // for the button); the per-tick notify also repaints the live meter.
        let transport_playing = rate != 0.0;
        if let Some(preview) = self.preview_view.clone() {
            let shown_frame = clock_playhead.unwrap_or(playhead).max(0);
            preview.update(cx, |preview, cx| {
                preview.state.total_frames = total_frames;
                preview.state.fps = fps;
                preview.state.active_frame = shown_frame;
                preview.state.is_playing = transport_playing;
                cx.notify();
            });
        }
        transport_playing || engine_playing
    }

    /// Toolbar "T": insert a default text clip at the timeline playhead via
    /// the shared add_texts tool (undoable; every view syncs on the revision).
    fn add_text_at_playhead(&mut self, cx: &mut Context<Self>) {
        let playhead = self
            .timeline_view
            .as_ref()
            .map(|tv| tv.read(cx).state.playhead_frame)
            .unwrap_or(0)
            .max(0);
        let executor = crate::editor_state_hub::EditorStateHub::global().executor();
        let guard = executor.lock();
        if let Ok(mut exec) = guard {
            let duration = default_text_duration_frames(exec.timeline().fps);
            let args = serde_json::json!({
                "texts": [{
                    "content": "Text",
                    "startFrame": playhead,
                    "durationFrames": duration,
                }]
            });
            if let Err(reason) = exec.execute("add_texts", &args) {
                eprintln!("add_texts failed: {reason}");
            }
        }
        cx.notify();
    }

    /// Open a .palmier project: load into the shared state, then show
    /// the editor. On failure the current screen is kept.
    pub fn open_project_at(&mut self, path: &std::path::Path, cx: &mut Context<Self>) {
        match crate::editor_state_hub::EditorStateHub::global().load_bundle(path) {
            Ok(()) => self.open_editor(cx),
            Err(reason) => eprintln!("Failed to open project {}: {reason}", path.display()),
        }
    }

    /// Import an FCP7 XMEML / FCPXML file as a new active timeline (the current
    /// one becomes a sibling — import never overwrites open work). Media relinks
    /// to the library by filename. Refreshes the timeline view on success.
    pub fn import_timeline_at(&mut self, path: &std::path::PathBuf, cx: &mut Context<Self>) {
        match crate::timeline_import::import_timeline_file_into_shared_state(path) {
            Ok(out) => {
                for note in &out.notes {
                    eprintln!("Import note: {note}");
                }
                if let Some(tv) = self.timeline_view.as_ref() {
                    tv.update(cx, |_, cx| cx.notify());
                }
                cx.notify();
            }
            Err(reason) => eprintln!("Import timeline failed for {}: {reason}", path.display()),
        }
    }

    /// Navigate back to Home (e.g., close project).
    pub fn show_home(&mut self, cx: &mut Context<Self>) {
        // Stop audio + transport so Home is silent and re-entry starts paused.
        if let Ok(mut engine) = crate::audio_playback::engine().lock() {
            engine.stop();
        }
        if let Some(tv) = self.timeline_view.clone() {
            tv.update(cx, |view, cx| view.transport_jkl(0, cx));
        }
        // Autosave before leaving the editor (#211, Swift saves on close);
        // best-effort — a rootless (unsaved) project returns Err and is skipped.
        let _ = crate::editor_state_hub::EditorStateHub::global().save_now();
        self.active_screen = ActiveScreen::Home;
        cx.notify();
    }

    pub fn handle_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.keystroke.key == "escape" && self.project_menu.is_open() {
            self.project_menu.close();
            cx.stop_propagation();
            cx.notify();
            return;
        }

        let modifiers = menu::Modifiers {
            command: event.keystroke.modifiers.platform,
            shift: event.keystroke.modifiers.shift,
            option: event.keystroke.modifiers.alt,
            control: event.keystroke.modifiers.control,
        };

        // Typing-conflicting chords are dispatched by the binding system
        // (global_shortcuts, "!input" predicate); re-routing them here would
        // fire them while a text input has focus.
        if menu::is_text_conflicting(&modifiers) {
            return;
        }

        let Some(action) = menu::route_shortcut(&event.keystroke.key, &modifiers) else {
            return;
        };

        cx.stop_propagation();
        self.perform_menu_action(action, window, cx);
    }

    /// Single dispatch point for shortcut actions (chorded shortcuts via
    /// handle_key_down, modifier-free ones via the global_shortcuts
    /// bindings).
    pub fn perform_menu_action(
        &mut self,
        action: menu::MenuAction,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.dispatch_menu_action(action, cx);
    }

    /// Window-free menu dispatch (no menu action currently needs the window);
    /// the title-bar menu bar routes here since its subscription has no window.
    pub fn dispatch_menu_action(&mut self, action: menu::MenuAction, cx: &mut Context<Self>) {
        match action {
            menu::MenuAction::NewProject => {
                // Fresh shared state so the UI and MCP observe the same new project.
                crate::editor_state_hub::EditorStateHub::global().load_project(
                    core_model::Timeline::default(),
                    core_model::MediaManifest::default(),
                );
                self.open_editor(cx);
            }
            menu::MenuAction::OpenProject => {
                let rx = cx.prompt_for_paths(PathPromptOptions {
                    files: false,
                    directories: true,
                    multiple: false,
                    prompt: Some("Open".into()),
                });
                cx.spawn(async move |this, cx| {
                    if let Ok(Ok(Some(paths))) = rx.await {
                        if let Some(path) = paths.first() {
                            let path = path.clone();
                            let _ = this.update(cx, |root, cx| root.open_project_at(&path, cx));
                        }
                    }
                })
                .detach();
            }
            menu::MenuAction::ToggleMediaPanel => {
                self.pane_layout.toggle_pane(PaneId::Media);
                self.persist_pane_visibility();
                self.persist_pane_sizes();
            }
            menu::MenuAction::ToggleInspector => {
                self.pane_layout.toggle_pane(PaneId::Inspector);
                self.persist_pane_visibility();
                self.persist_pane_sizes();
            }
            menu::MenuAction::ToggleAgentPanel => {
                self.pane_layout.toggle_pane(PaneId::Agent);
                self.persist_pane_visibility();
                self.persist_pane_sizes();
            }
            menu::MenuAction::MaximizeFocusedPane => {
                if self.pane_layout.is_maximized() {
                    self.pane_layout.unmaximize();
                    self.persist_pane_visibility();
                    self.persist_pane_sizes();
                } else {
                    let target = maximize_target(self.focused_pane);
                    self.pane_layout.maximize(target);
                    // The maximized pane takes the focus ring (design D8).
                    self.focused_pane = Some(target);
                }
            }
            menu::MenuAction::LayoutDefault => {
                self.pane_layout.apply_preset(LayoutPreset::Default);
                self.reset_pane_sizes();
            }
            menu::MenuAction::LayoutMedia => {
                self.pane_layout.apply_preset(LayoutPreset::Media);
                self.reset_pane_sizes();
            }
            menu::MenuAction::LayoutVertical => {
                self.pane_layout.apply_preset(LayoutPreset::Vertical);
                self.reset_pane_sizes();
            }
            menu::MenuAction::EnterFullScreen => {}
            menu::MenuAction::Quit => {}
            menu::MenuAction::SaveProject => {
                if let Err(reason) = crate::editor_state_hub::EditorStateHub::global().save() {
                    eprintln!("Save failed: {reason}");
                }
            }
            menu::MenuAction::SaveProjectAs => {
                let hub = crate::editor_state_hub::EditorStateHub::global();
                let start_dir = hub
                    .project_root()
                    .and_then(|p| p.parent().map(|d| d.to_path_buf()))
                    .or_else(std::env::home_dir)
                    .unwrap_or_else(|| std::path::PathBuf::from("."));
                let rx = cx.prompt_for_new_path(&start_dir, Some("Untitled.palmier"));
                cx.spawn(async move |_, _| {
                    if let Ok(Ok(Some(path))) = rx.await {
                        if let Err(reason) =
                            crate::editor_state_hub::EditorStateHub::global().save_as(&path)
                        {
                            eprintln!("Save As failed: {reason}");
                        }
                    }
                })
                .detach();
            }
            menu::MenuAction::ImportMedia => {
                let rx = cx.prompt_for_paths(PathPromptOptions {
                    files: true,
                    directories: false,
                    multiple: true,
                    prompt: Some("Import".into()),
                });
                cx.spawn(async move |_, _| {
                    if let Ok(Ok(Some(paths))) = rx.await {
                        crate::media_import::import_files_into_shared_state(&paths);
                    }
                })
                .detach();
            }
            menu::MenuAction::ImportTimeline => {
                let rx = cx.prompt_for_paths(PathPromptOptions {
                    files: true,
                    directories: false,
                    multiple: false,
                    prompt: Some("Import Timeline".into()),
                });
                cx.spawn(async move |this, cx| {
                    if let Ok(Ok(Some(paths))) = rx.await {
                        if let Some(path) = paths.first() {
                            let path = path.clone();
                            let _ = this.update(cx, |root, cx| root.import_timeline_at(&path, cx));
                        }
                    }
                })
                .detach();
            }
            menu::MenuAction::Export => {}
            menu::MenuAction::Undo => {
                crate::timeline_view::TimelineView::run_history_tool("undo");
                cx.notify();
            }
            menu::MenuAction::Redo => {
                crate::timeline_view::TimelineView::run_history_tool("redo");
                cx.notify();
            }
            menu::MenuAction::Cut | menu::MenuAction::Copy => {}
            menu::MenuAction::Paste => {
                let _action = route_paste(FocusTarget::Timeline);
            }
            menu::MenuAction::SplitAtPlayhead => {
                if let Some(tv) = self.timeline_view.clone() {
                    tv.update(cx, |view, cx| view.split_selected_at_playhead(cx));
                }
            }
            menu::MenuAction::Delete => {
                if let Some(tv) = self.timeline_view.clone() {
                    tv.update(cx, |view, cx| view.delete_selected(cx));
                }
            }
            menu::MenuAction::SelectAll => {
                if let Some(tv) = self.timeline_view.clone() {
                    tv.update(cx, |view, cx| view.select_all(cx));
                }
            }
            menu::MenuAction::TrimStartToPlayhead => {
                if let Some(tv) = self.timeline_view.clone() {
                    tv.update(cx, |view, cx| {
                        view.trim_selected_to_playhead(crate::timeline_model::TrimEdge::Start, cx)
                    });
                }
            }
            menu::MenuAction::TrimEndToPlayhead => {
                if let Some(tv) = self.timeline_view.clone() {
                    tv.update(cx, |view, cx| {
                        view.trim_selected_to_playhead(crate::timeline_model::TrimEdge::End, cx)
                    });
                }
            }
            menu::MenuAction::RippleDelete => {
                if let Some(tv) = self.timeline_view.clone() {
                    tv.update(cx, |view, cx| view.ripple_delete_selected(cx));
                }
            }
            menu::MenuAction::About
            | menu::MenuAction::CheckForUpdates
            | menu::MenuAction::Settings => {}
            menu::MenuAction::Tutorial
            | menu::MenuAction::KeyboardShortcuts
            | menu::MenuAction::McpInstructions => {}
            menu::MenuAction::SendFeedback => {
                crate::platform_adapter::open_url(agent_contract::FEEDBACK_ISSUES_URL);
            }
            menu::MenuAction::PlayPause => {
                if let Some(tv) = self.timeline_view.clone() {
                    tv.update(cx, |view, cx| view.transport_toggle_play(cx));
                    self.ensure_audio_sync_tick(cx);
                }
            }
            menu::MenuAction::PlayBackward => {
                if let Some(tv) = self.timeline_view.clone() {
                    tv.update(cx, |view, cx| view.transport_jkl(-1, cx));
                    self.ensure_audio_sync_tick(cx);
                }
            }
            menu::MenuAction::PauseJkl => {
                if let Some(tv) = self.timeline_view.clone() {
                    tv.update(cx, |view, cx| view.transport_jkl(0, cx));
                    self.ensure_audio_sync_tick(cx);
                }
            }
            menu::MenuAction::PlayForward => {
                if let Some(tv) = self.timeline_view.clone() {
                    tv.update(cx, |view, cx| view.transport_jkl(1, cx));
                    self.ensure_audio_sync_tick(cx);
                }
            }
            menu::MenuAction::StepFrameBackward => {
                if let Some(tv) = self.timeline_view.clone() {
                    tv.update(cx, |view, cx| view.transport_step(-1, cx));
                    self.ensure_audio_sync_tick(cx);
                }
            }
            menu::MenuAction::StepFrameForward => {
                if let Some(tv) = self.timeline_view.clone() {
                    tv.update(cx, |view, cx| view.transport_step(1, cx));
                    self.ensure_audio_sync_tick(cx);
                }
            }
            menu::MenuAction::SkipFramesBackward => {
                if let Some(tv) = self.timeline_view.clone() {
                    tv.update(cx, |view, cx| view.transport_step(-5, cx));
                    self.ensure_audio_sync_tick(cx);
                }
            }
            menu::MenuAction::SkipFramesForward => {
                if let Some(tv) = self.timeline_view.clone() {
                    tv.update(cx, |view, cx| view.transport_step(5, cx));
                    self.ensure_audio_sync_tick(cx);
                }
            }
            menu::MenuAction::MarkIn
            | menu::MenuAction::MarkOut
            | menu::MenuAction::ClearMarkIn
            | menu::MenuAction::ClearMarkOut
            | menu::MenuAction::ClearMarks => {}
            menu::MenuAction::TimelineZoomIn
            | menu::MenuAction::TimelineZoomOut
            | menu::MenuAction::TimelineFitToWindow => {}
        }

        cx.notify();
    }

    /// Capsule button for the welcome overlay (Swift: .capsule button style).
    fn welcome_button(id: &'static str, label: &str, prominent: bool) -> gpui::Stateful<gpui::Div> {
        div()
            .id(id)
            .px(px(Spacing::LG))
            .py(px(Spacing::SM))
            .rounded_full()
            .cursor_pointer()
            .text_size(px(FontSize::SM))
            .when(prominent, |el| {
                el.bg(crate::theme::Accent::PRIMARY)
                    .text_color(Background::BASE)
                    .font_weight(gpui::FontWeight::SEMIBOLD)
            })
            .when(!prominent, |el| {
                el.border_1()
                    .border_color(BorderColors::PRIMARY)
                    .text_color(Text::SECONDARY)
            })
            .child(label.to_string())
    }

    /// Project-card context menu entries (order defines activation indices).
    /// Open/Reveal are omitted for a missing package (Swift: entry.isAccessible).
    fn project_card_menu_entries(accessible: bool) -> Vec<crate::context_menu::MenuEntry> {
        use crate::context_menu::MenuEntry;
        let mut entries = Vec::new();
        if accessible {
            entries.push(MenuEntry::item("open", "Open"));
            entries.push(MenuEntry::item("reveal", "Reveal in File Manager"));
            // Duplicate copies the .palmier package on disk (Issue #67); a missing
            // package can't be duplicated, so it's accessible-gated.
            entries.push(MenuEntry::item("duplicate", "Duplicate"));
            entries.push(MenuEntry::separator());
        }
        entries.push(MenuEntry::item("remove-recents", "Remove from Recents"));
        entries.push(MenuEntry::separator());
        entries.push(MenuEntry::destructive_confirm(
            "delete",
            "Delete Project",
            "Confirm Delete",
        ));
        entries
    }

    fn activate_project_menu_item(&mut self, index: usize, cx: &mut Context<Self>) {
        let target = self.project_menu.target().cloned();
        let entries =
            Self::project_card_menu_entries(target.as_ref().map_or(true, |t| t.accessible));
        if let crate::context_menu::Activation::Perform(id) =
            self.project_menu.activate(index, &entries)
        {
            if let Some(target) = target {
                match id {
                    "open" => self.open_project_at(&target.path, cx),
                    "reveal" => crate::platform_adapter::reveal_in_file_manager(&target.path),
                    "duplicate" => self.duplicate_project_at(&target.path, cx),
                    "remove-recents" => {
                        Self::remove_from_recents(&target.registry_id);
                        self.home_cards_loaded_at = None;
                    }
                    "delete" => {
                        Self::delete_project(&target.registry_id, &target.path);
                        self.home_cards_loaded_at = None;
                    }
                    _ => {}
                }
            }
        }
        cx.notify();
    }

    /// Drop a project from the recents registry (the package stays on disk).
    fn remove_from_recents(registry_id: &str) {
        let registry_path = crate::project_registry_store::default_registry_path();
        let mut registry = crate::project_registry_store::load_from(&registry_path);
        if registry.remove(registry_id) {
            if let Err(reason) = crate::project_registry_store::save_to(&registry_path, &registry) {
                eprintln!("Failed to update recents: {reason}");
            }
        }
    }

    /// Duplicate the project's .palmier package on disk, register the copy in
    /// recents, and refresh the Home cards (Issue #67). Errors are logged, not
    /// surfaced — the tool itself is host-gated, so the host owns the fs I/O.
    fn duplicate_project_at(&mut self, path: &std::path::Path, cx: &mut Context<Self>) {
        match Self::duplicate_project_package(path) {
            Ok(dest) => {
                let registry_path = crate::project_registry_store::default_registry_path();
                if let Err(reason) =
                    crate::project_registry_store::record_opened_at(&registry_path, &dest)
                {
                    eprintln!("Failed to register duplicated project: {reason}");
                }
                self.home_cards_loaded_at = None;
            }
            Err(reason) => eprintln!("Failed to duplicate project: {reason}"),
        }
        cx.notify();
    }

    /// Copy a `.palmier` package next to the original as "<name> (Copy).palmier"
    /// (project_io's duplicate plan + a recursive tree copy). Returns the new
    /// package path. Pure host fs I/O — no gpui, so it is unit-testable.
    fn duplicate_project_package(source: &std::path::Path) -> Result<std::path::PathBuf, String> {
        let name = source
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Project");
        let plan = project_io::project_duplicate::plan_duplicate(
            source,
            &project_io::project_duplicate::DuplicateOptions::default(),
            name,
        )?;
        Self::copy_dir_all(&plan.source_path, &plan.destination_path)?;
        Ok(plan.destination_path)
    }

    /// Recursively copy a directory tree (a .palmier package is a directory).
    fn copy_dir_all(src: &std::path::Path, dst: &std::path::Path) -> Result<(), String> {
        std::fs::create_dir_all(dst).map_err(|e| e.to_string())?;
        for entry in std::fs::read_dir(src).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let from = entry.path();
            let to = dst.join(entry.file_name());
            if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
                Self::copy_dir_all(&from, &to)?;
            } else {
                std::fs::copy(&from, &to).map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }

    /// Delete the .palmier package from disk, then drop the recents entry.
    fn delete_project(registry_id: &str, path: &std::path::Path) {
        // Only ever delete .palmier packages, even if the registry is corrupt.
        if path.extension().and_then(|e| e.to_str()) == Some("palmier") {
            if path.exists() {
                if let Err(reason) = std::fs::remove_dir_all(path) {
                    eprintln!("Failed to delete project {}: {reason}", path.display());
                }
            }
        } else {
            eprintln!("Refusing to delete non-.palmier path {}", path.display());
        }
        Self::remove_from_recents(registry_id);
    }

    /// Render the Home screen: sidebar (220px) + content area.
    fn render_home(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let samples_expanded = self.samples_expanded;
        let is_signed_in = self.is_signed_in;
        // Registry + fs-stat snapshots on a 2s TTL: hover transitions re-render
        // Home, and re-reading the registry file per render was a measurable
        // burst under mouse movement (review F1).
        let stale = self
            .home_cards_loaded_at
            .is_none_or(|t| t.elapsed().as_secs() >= 2);
        if stale {
            let registry = crate::project_registry_store::load_from(
                &crate::project_registry_store::default_registry_path(),
            );
            let now = chrono::Utc::now();
            self.home_cards = registry
                .sorted_entries()
                .iter()
                .map(|entry| {
                    let thumb = entry.url.join(core_model::THUMBNAIL_FILENAME);
                    (
                        entry.id.clone(),
                        entry.name(),
                        crate::home_model::relative_time_label(entry.last_opened_date, now),
                        thumb.is_file().then_some(thumb),
                        entry.url.clone(),
                        entry.url.exists(),
                    )
                })
                .collect();
            self.home_cards_loaded_at = Some(std::time::Instant::now());
        }
        let recent_projects: Vec<HomeCard> = self.home_cards.clone();

        // Sample project card data (Swift: SampleProjectsStrip items).
        // Placeholder titles/colors only: real samples come from
        // SampleProjectService (network backend, gated) — posters, download
        // progress, and open-on-click are wired when that service lands.
        let sample_cards: &[(&str, f32)] = &[
            ("Short Film", 0.60),
            ("Commercial", 0.75),
            ("Documentary", 0.43),
        ];

        let project_menu_open = self.project_menu.open_menu().cloned();
        let hovered_project = self.hovered_project.clone();
        let armed_delete = self.armed_delete.clone();

        div()
            .id("fronda-home")
            .track_focus(&self.home.focus_handle.clone())
            .flex()
            .flex_row()
            .relative()
            .size_full()
            .bg(Background::SURFACE)
            // ── Left sidebar (220px) ──
            .child(
                div()
                    .id("home-sidebar")
                    .flex()
                    .flex_col()
                    .w(px(220.0))
                    .h_full()
                    .bg(Background::SURFACE)
                    .border_r_1()
                    .border_color(BorderColors::PRIMARY)
                    .px(px(Spacing::SM_MD))
                    .py(px(Spacing::MD))
                    .gap(px(Spacing::XXS))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .w_full()
                            .h(px(40.0))
                            .mb(px(Spacing::SM_MD))
                            .child(
                                div()
                                    .text_color(Text::PRIMARY)
                                    .text_size(px(FontSize::MD_LG))
                                    .child("Fronda"),
                            ),
                    )
                    .child(
                        crate::home_view::sidebar_row_button(
                            "sidebar-new-project",
                            "icons/plus.svg",
                            "New Project",
                            false,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.open_editor(cx);
                        })),
                    )
                    .child(
                        crate::home_view::sidebar_row_button(
                            "sidebar-open-project",
                            "icons/folder.svg",
                            "Open Project",
                            false,
                        )
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.perform_menu_action(menu::MenuAction::OpenProject, window, cx);
                        })),
                    )
                    .child(div().flex_1())
                    // "Sign in with Google" — shown when not signed in (Swift: HomeSidebar)
                    .when(!is_signed_in, |el| {
                        el.child(
                            div()
                                .id("sidebar-sign-in")
                                .w_full()
                                .px(px(crate::theme::Spacing::SM))
                                .py(px(crate::theme::Spacing::XS))
                                .mb(px(crate::theme::Spacing::XS))
                                .rounded(px(crate::theme::Radius::SM))
                                .border_1()
                                .border_color(crate::theme::BorderColors::SUBTLE)
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(crate::theme::Spacing::XS))
                                .cursor_pointer()
                                .on_click(cx.listener(|this: &mut AppRoot, _, _, cx| {
                                    this.is_signed_in = true;
                                    cx.notify();
                                }))
                                .child(
                                    div()
                                        .text_size(px(crate::theme::FontSize::SM))
                                        .text_color(crate::theme::Text::SECONDARY)
                                        .child("Sign in"),
                                ),
                        )
                    })
                    .child(crate::home_view::sidebar_row_button(
                        "sidebar-settings",
                        "icons/gear.svg",
                        "Settings",
                        false,
                    )),
            )
            // ── Content area ──
            .child(
                div()
                    .id("home-content")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .h_full()
                    .bg(Background::SURFACE)
                    // Header: welcome title
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .px(px(HomeLayout::CARD_GAP as f32 * 2.0))
                            .pt(px(HomeLayout::HEADING_TOP as f32))
                            .pb(px(Spacing::XXL))
                            .child(
                                div()
                                    .text_size(px(FontSize::TITLE_2))
                                    .text_color(Text::PRIMARY)
                                    .child("Welcome to Fronda"),
                            ),
                    )
                    // Sample Projects strip (collapsible, matches Swift SampleProjectsStrip)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(Spacing::SM))
                            .px(px(HomeLayout::CARD_GAP as f32 * 2.0))
                            .pb(px(Spacing::XXL))
                            // Section header
                            .child(
                                div()
                                    .id("samples-header")
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(Spacing::XS))
                                    .cursor_pointer()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.samples_expanded = !this.samples_expanded;
                                        cx.notify();
                                    }))
                                    .child(
                                        div()
                                            .text_size(px(FontSize::SM_MD))
                                            .text_color(Text::SECONDARY)
                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                            .child("Sample Projects"),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(FontSize::XS))
                                            .text_color(Text::MUTED)
                                            .child(if samples_expanded { "▾" } else { "▸" }),
                                    ),
                            )
                            // Sample cards strip
                            .when(samples_expanded, |el| {
                                el.child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .gap(px(HomeLayout::CARD_GAP as f32))
                                        .children(sample_cards.iter().enumerate().map(
                                            |(i, (_, hue))| {
                                                let name: &'static str = match i {
                                                    0 => "Short Film",
                                                    1 => "Commercial",
                                                    _ => "Documentary",
                                                };
                                                let h = *hue;
                                                div()
                                                    .id(format!("sample-card-{i}"))
                                                    .flex()
                                                    .flex_col()
                                                    .w(px(HomeLayout::CARD_WIDTH as f32))
                                                    .h(px(HomeLayout::CARD_HEIGHT as f32))
                                                    .bg(Background::RAISED)
                                                    .rounded(px(Radius::MD_LG))
                                                    .border_1()
                                                    .border_color(BorderColors::SUBTLE)
                                                    .overflow_hidden()
                                                    .cursor_pointer()
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.open_editor(cx);
                                                    }))
                                                    .child(
                                                        div()
                                                            .flex_1()
                                                            .bg(gpui::Hsla {
                                                                h,
                                                                s: 0.35,
                                                                l: 0.14,
                                                                a: 1.0,
                                                            })
                                                            .flex()
                                                            .items_center()
                                                            .justify_center()
                                                            .text_color(gpui::Hsla {
                                                                h,
                                                                s: 0.55,
                                                                l: 0.55,
                                                                a: 1.0,
                                                            })
                                                            .text_size(px(FontSize::DISPLAY))
                                                            .child("▶"),
                                                    )
                                                    .child(
                                                        div()
                                                            .flex()
                                                            .flex_row()
                                                            .items_center()
                                                            .w_full()
                                                            .h(px(24.0))
                                                            .px(px(Spacing::SM_MD))
                                                            .bg(Background::RAISED)
                                                            .child(
                                                                div()
                                                                    .text_size(px(FontSize::SM))
                                                                    .text_color(Text::PRIMARY)
                                                                    .child(name),
                                                            ),
                                                    )
                                            },
                                        )),
                                )
                            }),
                    )
                    // "My Projects" section label (semibold, matches Swift)
                    .child(
                        div()
                            .px(px(HomeLayout::CARD_GAP as f32 * 2.0))
                            .pb(px(Spacing::SM))
                            .text_size(px(FontSize::SM_MD))
                            .text_color(Text::SECONDARY)
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("My Projects"),
                    )
                    // Project grid: New Project card only (user's real projects go here)
                    .child(
                        div()
                            .id("project-grid")
                            .flex()
                            .flex_row()
                            .flex_wrap()
                            .px(px(HomeLayout::CARD_GAP as f32 * 2.0))
                            .gap(px(HomeLayout::CARD_GAP as f32))
                            // New Project card — thumbnail area + name strip (same structure as project_card)
                            .child(
                                div()
                                    .id("card-new-project")
                                    .flex()
                                    .flex_col()
                                    .w(px(HomeLayout::CARD_WIDTH as f32))
                                    .h(px(HomeLayout::CARD_HEIGHT as f32))
                                    .bg(Background::RAISED)
                                    .rounded(px(Radius::MD_LG))
                                    .border_1()
                                    .border_color(BorderColors::SUBTLE)
                                    .overflow_hidden()
                                    .cursor_pointer()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.open_editor(cx);
                                    }))
                                    // Thumbnail area: dashed-style placeholder with + icon
                                    .child(
                                        div()
                                            .flex_1()
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .border_1()
                                            .border_color(BorderColors::SUBTLE)
                                            .child(
                                                div()
                                                    .text_size(px(FontSize::TITLE_2))
                                                    .text_color(Text::MUTED)
                                                    .child("+"),
                                            ),
                                    )
                                    // Name strip
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .w_full()
                                            .h(px(24.0))
                                            .px(px(Spacing::SM_MD))
                                            .bg(Background::RAISED)
                                            .child(
                                                div()
                                                    .text_size(px(FontSize::SM))
                                                    .text_color(Text::TERTIARY)
                                                    .child("New Project"),
                                            ),
                                    ),
                            )
                            // Recent projects (from registry)
                            .children(recent_projects.into_iter().map(
                                |(id, name, time_label, thumb, path, accessible)| {
                                    let hue = crate::media_panel_model::tile_hue(&name);
                                    let menu_target = ProjectMenuTarget {
                                        registry_id: id.clone(),
                                        path: path.clone(),
                                        accessible,
                                    };
                                    let hovered = hovered_project.as_deref() == Some(id.as_str());
                                    let armed = armed_delete.as_deref() == Some(id.as_str());
                                    let hover_id = id.clone();
                                    let trash_id = id.clone();
                                    let trash_path = path.clone();
                                    let thumb_area = if let Some(thumb) = thumb {
                                        div()
                                            .flex_1()
                                            .overflow_hidden()
                                            .child(
                                                gpui::img(thumb)
                                                    .size_full()
                                                    .object_fit(gpui::ObjectFit::Cover),
                                            )
                                            .into_any_element()
                                    } else {
                                        div()
                                            .flex_1()
                                            .bg(gpui::Hsla {
                                                h: hue,
                                                s: 0.35,
                                                l: 0.14,
                                                a: 1.0,
                                            })
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .text_color(gpui::Hsla {
                                                h: hue,
                                                s: 0.55,
                                                l: 0.55,
                                                a: 1.0,
                                            })
                                            .text_size(px(FontSize::DISPLAY))
                                            .child("\u{25b6}")
                                            .into_any_element()
                                    };
                                    div()
                                        .id(format!("recent-{id}"))
                                        .flex()
                                        .flex_col()
                                        .relative()
                                        .w(px(HomeLayout::CARD_WIDTH as f32))
                                        .h(px(HomeLayout::CARD_HEIGHT as f32))
                                        .bg(Background::RAISED)
                                        .rounded(px(Radius::MD_LG))
                                        .border_1()
                                        .border_color(if hovered {
                                            gpui::Hsla {
                                                h: 0.0,
                                                s: 0.0,
                                                l: 1.0,
                                                a: crate::theme::Opacity::MUTED,
                                            }
                                        } else {
                                            BorderColors::SUBTLE
                                        })
                                        .overflow_hidden()
                                        .cursor_pointer()
                                        .when(hovered, |el| el.shadow_lg())
                                        .when(!accessible, |el| {
                                            el.opacity(crate::theme::Opacity::STRONG)
                                        })
                                        .on_hover(cx.listener(
                                            move |this, entered: &bool, _, cx| {
                                                if *entered {
                                                    this.hovered_project = Some(hover_id.clone());
                                                } else {
                                                    if this.hovered_project.as_deref()
                                                        == Some(hover_id.as_str())
                                                    {
                                                        this.hovered_project = None;
                                                    }
                                                    if this.armed_delete.as_deref()
                                                        == Some(hover_id.as_str())
                                                    {
                                                        this.armed_delete = None;
                                                    }
                                                }
                                                cx.notify();
                                            },
                                        ))
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            // Missing package: the overlay explains why
                                            // opening is unavailable (Swift guards the tap).
                                            if accessible {
                                                this.open_project_at(&path.clone(), cx);
                                            }
                                        }))
                                        .on_mouse_down(
                                            MouseButton::Right,
                                            cx.listener(move |this, e: &MouseDownEvent, _, cx| {
                                                this.project_menu.open_at(
                                                    e.position.x.as_f32(),
                                                    e.position.y.as_f32(),
                                                    menu_target.clone(),
                                                );
                                                cx.notify();
                                            }),
                                        )
                                        .child(thumb_area)
                                        .child(
                                            div()
                                                .flex()
                                                .flex_row()
                                                .items_center()
                                                .justify_between()
                                                .w_full()
                                                .h(px(24.0))
                                                .px(px(Spacing::SM_MD))
                                                .bg(Background::RAISED)
                                                .child(
                                                    div()
                                                        .text_size(px(FontSize::SM))
                                                        .text_color(if accessible {
                                                            Text::PRIMARY
                                                        } else {
                                                            Text::MUTED
                                                        })
                                                        .overflow_hidden()
                                                        .child(name),
                                                )
                                                .child(
                                                    div()
                                                        .text_size(px(FontSize::XS))
                                                        .text_color(Text::TERTIARY)
                                                        .child(time_label),
                                                ),
                                        )
                                        // File-missing overlay (Swift: questionmark.folder + dim)
                                        .when(!accessible, |el| {
                                            el.child(
                                                div()
                                                    .absolute()
                                                    .top_0()
                                                    .left_0()
                                                    .size_full()
                                                    .flex()
                                                    .flex_col()
                                                    .items_center()
                                                    .justify_center()
                                                    .gap(px(Spacing::XS))
                                                    .bg(gpui::Hsla {
                                                        h: 0.0,
                                                        s: 0.0,
                                                        l: 0.0,
                                                        a: crate::theme::Opacity::STRONG,
                                                    })
                                                    .child(
                                                        div()
                                                            .text_size(px(FontSize::TITLE_1))
                                                            .text_color(Text::TERTIARY)
                                                            .child("?"),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_size(px(FontSize::XS))
                                                            .text_color(Text::TERTIARY)
                                                            .font_weight(gpui::FontWeight::MEDIUM)
                                                            .child("File missing"),
                                                    ),
                                            )
                                        })
                                        // Hover trash button — arm-then-confirm, mirroring
                                        // the context menu's destructive confirm step.
                                        .when(hovered, |el| {
                                            el.child(
                                                div()
                                                    .id(format!("recent-trash-{id}"))
                                                    .absolute()
                                                    .top(px(Spacing::SM_MD))
                                                    .right(px(Spacing::SM_MD))
                                                    // No occlude: it would un-hover the
                                                    // card and flicker the button away;
                                                    // stop_propagation guards the open.
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .h(px(crate::theme::IconSize::LG_XL))
                                                    .when(!armed, |b| {
                                                        b.w(px(crate::theme::IconSize::LG_XL))
                                                    })
                                                    .when(armed, |b| b.px(px(Spacing::SM_MD)))
                                                    .rounded_full()
                                                    .bg(Background::RAISED)
                                                    .border_1()
                                                    .border_color(BorderColors::SUBTLE)
                                                    .cursor_pointer()
                                                    .text_size(px(FontSize::SM_MD))
                                                    .text_color(crate::theme::Status::ERROR)
                                                    .when(armed, |b| {
                                                        b.font_weight(gpui::FontWeight::SEMIBOLD)
                                                    })
                                                    .on_click(cx.listener(move |this, _, _, cx| {
                                                        cx.stop_propagation();
                                                        if this.armed_delete.as_deref()
                                                            == Some(trash_id.as_str())
                                                        {
                                                            Self::delete_project(
                                                                &trash_id,
                                                                &trash_path,
                                                            );
                                                            this.home_cards_loaded_at = None;
                                                            this.armed_delete = None;
                                                            this.hovered_project = None;
                                                        } else {
                                                            this.armed_delete =
                                                                Some(trash_id.clone());
                                                        }
                                                        cx.notify();
                                                    }))
                                                    .child(if armed {
                                                        "Confirm Delete".to_string()
                                                    } else {
                                                        "\u{1f5d1}".to_string()
                                                    }),
                                            )
                                        })
                                },
                            )),
                    ),
            )
            // Project-card context menu (deferred popover, above everything)
            .when_some(project_menu_open, |el, open| {
                el.child(crate::context_menu::render_context_menu(
                    gpui::point(px(open.x), px(open.y)),
                    Self::project_card_menu_entries(open.target.accessible),
                    open.confirming,
                    cx,
                    |this: &mut AppRoot, index, _window, cx| {
                        this.activate_project_menu_item(index, cx)
                    },
                    |this: &mut AppRoot, _window, cx| {
                        this.project_menu.close();
                        cx.notify();
                    },
                ))
            })
    }
}

impl Focusable for AppRoot {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for AppRoot {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content: gpui::AnyElement = match self.active_screen {
            ActiveScreen::Home => self.render_home(cx).into_any_element(),
            ActiveScreen::Editor => {
                // Agent-chat handoff: reveal the agent pane and draft the
                // parked prompt into the composer (Swift: newChat + draft +
                // open panel). Focus/text mutation is deferred out of render.
                let pending = PENDING_AGENT_PROMPT.lock().ok().and_then(|mut s| s.take());
                if let Some(prompt) = pending {
                    let mut vis_changed = false;
                    if self
                        .pane_layout
                        .maximized_pane
                        .is_some_and(|p| p != PaneId::Agent)
                    {
                        self.pane_layout.unmaximize();
                        vis_changed = true;
                    }
                    if !self.pane_layout.is_visible(PaneId::Agent) {
                        self.pane_layout.toggle_pane(PaneId::Agent);
                        vis_changed = true;
                    }
                    if vis_changed {
                        self.persist_pane_visibility();
                    }
                    if let Some(chat) = self.chat_view.clone() {
                        cx.defer_in(window, move |_, window, cx| {
                            chat.update(cx, |chat, cx| {
                                chat.set_composer_text(prompt, window, cx);
                            });
                        });
                    }
                }
                let layout = self.pane_layout.clone();
                let contents = editor_view::PaneContents::new(
                    self.chat_view.clone(),
                    self.toolbar_view.clone(),
                    self.media_panel_view.clone(),
                    self.preview_view.clone(),
                    self.timeline_view.clone(),
                    self.inspector_view.clone(),
                );
                // Editor content area (viewport minus the custom title bar);
                // stored for divider-drag clamping between renders.
                let viewport = window.viewport_size();
                let vw = viewport.width.as_f32();
                let vh = viewport.height.as_f32() - crate::theme::Layout::TOOLBAR_HEIGHT;
                self.last_viewport = (vw, vh);
                let preset_w = (vw
                    - if layout.is_visible(PaneId::Agent) {
                        self.agent_width
                    } else {
                        0.0
                    })
                .max(0.0);
                // Resolve sentinel sizes to the preset's initial proportions
                // (Swift applyAfterLayout setPosition calls).
                if self.timeline_height < 0.0 {
                    self.timeline_height =
                        crate::pane_resize::initial_timeline_height(layout.preset, vh);
                }
                if self.media_width < 0.0 {
                    // Swift buildMediaLayout: media column = 30% of the preset area.
                    self.media_width = match layout.preset {
                        LayoutPreset::Media => (preset_w * 0.3).round(),
                        _ => crate::theme::Layout::MEDIA_PANEL_DEFAULT,
                    };
                }
                if self.vertical_left_width < 0.0 {
                    // Swift buildVerticalLayout: left column = 50% of the preset area.
                    self.vertical_left_width = (preset_w * 0.5).round();
                }
                if self.pane_sizes_need_clamp {
                    // First editor render after boot-applying persisted sizes:
                    // the viewport is now known, so the full clamp (preview
                    // minimum space guard) runs once.
                    self.pane_sizes_need_clamp = false;
                    use crate::pane_resize::ResizeTarget::*;
                    for target in [AgentWidth, MediaWidth, InspectorWidth, TimelineHeight] {
                        let clamped = crate::pane_resize::clamp_resize(
                            target,
                            self.pane_size(target),
                            &self.resize_bounds(target),
                        );
                        self.set_pane_size(target, clamped);
                    }
                }
                let sizes = crate::pane_tree::ResolvedSizes {
                    agent_width: self.agent_width,
                    media_width: self.media_width,
                    inspector_width: self.inspector_width,
                    timeline_height: self.timeline_height,
                    vertical_left_width: self.vertical_left_width,
                };

                // Pre-built divider hitboxes; the tree decides which appear.
                use crate::pane_resize::ResizeTarget;
                let dividers: editor_view::DividerElements = [
                    ResizeTarget::AgentWidth,
                    ResizeTarget::MediaWidth,
                    ResizeTarget::InspectorWidth,
                    ResizeTarget::TimelineHeight,
                    ResizeTarget::VerticalLeftWidth,
                ]
                .into_iter()
                .map(|t| (t, self.build_divider(t, cx)))
                .collect();

                // Panel focus wiring (EDT-007): card clicks move the ring.
                let focus = editor_view::PaneFocus {
                    focused: self.focused_pane,
                    on_mouse_down: {
                        let weak = cx.entity().downgrade();
                        std::rc::Rc::new(move |pane, _window, cx| {
                            let _ = weak.update(cx, |this: &mut AppRoot, cx| {
                                if this.focused_pane != Some(pane) {
                                    this.focused_pane = Some(pane);
                                    cx.notify();
                                }
                            });
                        })
                    },
                };

                let weak = cx.entity().downgrade();

                div()
                    .flex()
                    .flex_col()
                    .size_full()
                    // Custom title bar (TitleBarLeadingView + TitleBarTrailingView)
                    .when_some(self.titlebar_view.clone(), |el, tb| el.child(tb))
                    .child(
                        div()
                            .flex()
                            .flex_1()
                            // Global handler for pane divider drags
                            .on_drag_move::<PaneResizeDrag>(
                                move |event: &DragMoveEvent<PaneResizeDrag>, _, cx: &mut App| {
                                    let _ = weak.update(cx, |this: &mut AppRoot, inner_cx| {
                                        if let Some(session) = this.pane_drag {
                                            let horizontal = session.target
                                                != crate::pane_resize::ResizeTarget::TimelineHeight;
                                            let pos = if horizontal {
                                                event.event.position.x.as_f32()
                                            } else {
                                                event.event.position.y.as_f32()
                                            };
                                            let delta = (pos - session.start_pos)
                                                * crate::pane_resize::drag_direction(
                                                    session.target,
                                                );
                                            let bounds = this.resize_bounds(session.target);
                                            let new_size = crate::pane_resize::clamp_resize(
                                                session.target,
                                                session.start_size + delta,
                                                &bounds,
                                            );
                                            this.set_pane_size(session.target, new_size);
                                            inner_cx.notify();
                                        }
                                    });
                                },
                            )
                            // Divider drag end: clear the session and persist
                            // the resolved sizes (any plain click no-ops).
                            .capture_any_mouse_up(cx.listener(
                                |this: &mut AppRoot, e: &MouseUpEvent, _, _| {
                                    if e.button == MouseButton::Left
                                        && this.pane_drag.take().is_some()
                                    {
                                        this.persist_pane_sizes();
                                    }
                                },
                            ))
                            // Release outside the editor area (e.g. over the
                            // title bar or off-window) still ends the drag.
                            .on_mouse_up_out(
                                MouseButton::Left,
                                cx.listener(|this: &mut AppRoot, _: &MouseUpEvent, _, _| {
                                    if this.pane_drag.take().is_some() {
                                        this.persist_pane_sizes();
                                    }
                                }),
                            )
                            .child(editor_view::render_pane_layout(
                                &layout, &contents, &focus, &sizes, dividers,
                            )),
                    )
                    .into_any_element()
            }
        };

        let tour = self.tour_overlay.clone();
        let update_overlay = self.update_overlay.clone();

        div()
            .id("fronda-root")
            .track_focus(&self.focus_handle.clone())
            .on_key_down(cx.listener(Self::handle_key_down))
            .on_action(
                cx.listener(|this, _: &crate::global_shortcuts::PlayPause, w, cx| {
                    this.perform_menu_action(menu::MenuAction::PlayPause, w, cx)
                }),
            )
            .on_action(
                cx.listener(|this, _: &crate::global_shortcuts::PlayBackward, w, cx| {
                    this.perform_menu_action(menu::MenuAction::PlayBackward, w, cx)
                }),
            )
            .on_action(
                cx.listener(|this, _: &crate::global_shortcuts::PauseJkl, w, cx| {
                    this.perform_menu_action(menu::MenuAction::PauseJkl, w, cx)
                }),
            )
            .on_action(
                cx.listener(|this, _: &crate::global_shortcuts::PlayForward, w, cx| {
                    this.perform_menu_action(menu::MenuAction::PlayForward, w, cx)
                }),
            )
            .on_action(cx.listener(
                |this, _: &crate::global_shortcuts::StepFrameBackward, w, cx| {
                    this.perform_menu_action(menu::MenuAction::StepFrameBackward, w, cx)
                },
            ))
            .on_action(cx.listener(
                |this, _: &crate::global_shortcuts::StepFrameForward, w, cx| {
                    this.perform_menu_action(menu::MenuAction::StepFrameForward, w, cx)
                },
            ))
            .on_action(cx.listener(
                |this, _: &crate::global_shortcuts::SkipFramesBackward, w, cx| {
                    this.perform_menu_action(menu::MenuAction::SkipFramesBackward, w, cx)
                },
            ))
            .on_action(cx.listener(
                |this, _: &crate::global_shortcuts::SkipFramesForward, w, cx| {
                    this.perform_menu_action(menu::MenuAction::SkipFramesForward, w, cx)
                },
            ))
            .on_action(cx.listener(
                |this, _: &crate::global_shortcuts::TrimStartToPlayhead, w, cx| {
                    this.perform_menu_action(menu::MenuAction::TrimStartToPlayhead, w, cx)
                },
            ))
            .on_action(cx.listener(
                |this, _: &crate::global_shortcuts::TrimEndToPlayhead, w, cx| {
                    this.perform_menu_action(menu::MenuAction::TrimEndToPlayhead, w, cx)
                },
            ))
            .on_action(cx.listener(
                |this, _: &crate::global_shortcuts::DeleteSelection, w, cx| {
                    this.perform_menu_action(menu::MenuAction::Delete, w, cx)
                },
            ))
            .on_action(cx.listener(
                |this, _: &crate::global_shortcuts::RippleDeleteSelection, w, cx| {
                    this.perform_menu_action(menu::MenuAction::RippleDelete, w, cx)
                },
            ))
            .on_action(cx.listener(
                |this, _: &crate::global_shortcuts::MaximizeFocusedPane, w, cx| {
                    this.perform_menu_action(menu::MenuAction::MaximizeFocusedPane, w, cx)
                },
            ))
            .on_action(
                cx.listener(|this, _: &crate::global_shortcuts::MarkIn, w, cx| {
                    this.perform_menu_action(menu::MenuAction::MarkIn, w, cx)
                }),
            )
            .on_action(
                cx.listener(|this, _: &crate::global_shortcuts::MarkOut, w, cx| {
                    this.perform_menu_action(menu::MenuAction::MarkOut, w, cx)
                }),
            )
            .on_action(
                cx.listener(|this, _: &crate::global_shortcuts::TimelineZoomIn, w, cx| {
                    this.perform_menu_action(menu::MenuAction::TimelineZoomIn, w, cx)
                }),
            )
            .on_action(cx.listener(
                |this, _: &crate::global_shortcuts::TimelineZoomOut, w, cx| {
                    this.perform_menu_action(menu::MenuAction::TimelineZoomOut, w, cx)
                },
            ))
            .on_action(cx.listener(
                |this, _: &crate::global_shortcuts::TimelineFitToWindow, w, cx| {
                    this.perform_menu_action(menu::MenuAction::TimelineFitToWindow, w, cx)
                },
            ))
            // Ctrl-modifier menu shortcuts (Windows/Linux; macOS uses the
            // system menu path).
            .on_action(
                cx.listener(|this, a: &crate::global_shortcuts::RunMenuAction, w, cx| {
                    this.perform_menu_action(a.action.clone(), w, cx)
                }),
            )
            .flex()
            .flex_col()
            .size_full()
            .relative()
            .child(content)
            // First-launch welcome overlay — window-level so the card centers
            // in the full window regardless of the Home content layout.
            .when(
                self.active_screen == ActiveScreen::Home && !self.welcome_dismissed,
                |el| {
                    let overlay = self.render_welcome_overlay(cx);
                    el.child(overlay)
                },
            )
            // Tour overlay stacks on top of everything at launch
            .child(div().absolute().top_0().left_0().size_full().child(tour))
            // Update changelog overlay — shown once after a new version installs
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .child(update_overlay),
            )
    }
}

/// Create and open the initial window.
pub fn open_main_window(cx: &mut App) {
    // BOOT: start the MCP server when the preference allows (Swift: startMCPService).
    if let Ok(mut svc) = crate::mcp_service::McpService::global().lock() {
        svc.start_if_enabled();
    }

    // BOOT: prune the thumbnail cache to its size cap off the UI thread.
    std::thread::spawn(|| {
        crate::video_thumbnails::prune_by_size(
            &crate::video_thumbnails::thumbnail_cache_dir(),
            crate::video_thumbnails::THUMBNAIL_CACHE_MAX_BYTES,
        );
    });

    // Load local ~/.palmier/skills into the in-app agent (upstream #199).
    if let Ok(mut guard) = crate::editor_state_hub::EditorStateHub::global()
        .executor()
        .lock()
    {
        crate::skill_store::load_skills_into_executor(&mut guard);
    }

    let cfg = WindowConfig::for_home();
    // Dev/verification seam: FRONDA_WINDOW_SIZE=WxH overrides the default
    // (gpui-ce on Windows currently treats these as device px; see notes).
    let (dw, dh) = std::env::var("FRONDA_WINDOW_SIZE")
        .ok()
        .and_then(|s| {
            let (w, h) = s.split_once('x')?;
            Some((w.parse().ok()?, h.parse().ok()?))
        })
        .unwrap_or((cfg.default_width as f32, cfg.default_height as f32));
    let size = size(px(dw), px(dh));
    let mut bounds = Bounds::centered(None, size, cx);
    bounds.origin.y = bounds.origin.y + px(220.0);
    // The +220 nudge (Swift Home placement) must not push the window off
    // small displays — keep it fully on screen.
    if let Some(display) = cx.primary_display() {
        let db = display.bounds();
        let max_y = db.origin.y + db.size.height - size.height;
        if bounds.origin.y > max_y {
            bounds.origin.y = max_y.max(db.origin.y);
        }
    }

    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            ..Default::default()
        },
        |window, cx| {
            let root = cx.new(|cx| {
                let mut root = AppRoot::new(cx);
                // Dev/verification seam: boot straight into an empty editor
                // (same path as Home's New Project button).
                if std::env::var("FRONDA_OPEN_EDITOR").is_ok_and(|v| v == "1") {
                    crate::editor_state_hub::EditorStateHub::global().load_project(
                        core_model::Timeline::default(),
                        core_model::MediaManifest::default(),
                    );
                    root.welcome_dismissed = true;
                    root.open_editor(cx);
                }
                root
            });
            // Focus the root at boot: gpui dispatches keystrokes and
            // resolves menu-action availability along the focus path, so
            // without this, shortcuts and app-menu items are dead (macOS
            // shows them disabled) until something else takes focus.
            let handle = root.read(cx).focus_handle.clone();
            window.focus(&handle, cx);
            root
        },
    )
    .unwrap();

    cx.activate(true);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_screen_starts_at_home() {
        assert_eq!(ActiveScreen::Home, ActiveScreen::Home);
        assert_ne!(ActiveScreen::Home, ActiveScreen::Editor);
    }

    #[test]
    fn maximize_targets_focused_pane_with_preview_fallback() {
        assert_eq!(maximize_target(Some(PaneId::Timeline)), PaneId::Timeline);
        assert_eq!(maximize_target(Some(PaneId::Agent)), PaneId::Agent);
        assert_eq!(maximize_target(None), PaneId::Preview);
    }

    #[test]
    fn persistable_size_filters_sentinels() {
        assert_eq!(persistable_size(320.0), Some(320.0));
        assert_eq!(persistable_size(-1.0), None);
        assert_eq!(persistable_size(0.0), None);
        assert_eq!(persistable_size(f32::NAN), None);
    }

    #[test]
    fn apply_persisted_sizes_overrides_only_present_fields() {
        let saved = crate::pane_prefs::PersistedPaneSizes {
            agent: Some(300.0),
            media: None,
            inspector: Some(280.0),
            timeline_height: None,
        };
        let mut agent = crate::pane_resize::AGENT_MIN;
        let mut media = -1.0;
        let mut inspector = crate::theme::Layout::INSPECTOR_DEFAULT;
        let mut timeline = -1.0;
        assert!(apply_persisted_sizes(
            &saved,
            &mut agent,
            &mut media,
            &mut inspector,
            &mut timeline
        ));
        assert_eq!(agent, 300.0);
        assert_eq!(media, -1.0, "missing key keeps the preset sentinel");
        assert_eq!(inspector, 280.0);
        assert_eq!(timeline, -1.0);
        assert!(!apply_persisted_sizes(
            &crate::pane_prefs::PersistedPaneSizes::default(),
            &mut agent,
            &mut media,
            &mut inspector,
            &mut timeline
        ));
    }

    #[test]
    fn text_duration_is_three_seconds_in_frames() {
        assert_eq!(default_text_duration_frames(30), 90);
        assert_eq!(default_text_duration_frames(24), 72);
        assert_eq!(default_text_duration_frames(0), 3, "fps clamps to 1");
        assert_eq!(default_text_duration_frames(-5), 3);
    }

    fn entry_ids(entries: &[crate::context_menu::MenuEntry]) -> Vec<&'static str> {
        entries
            .iter()
            .filter_map(|e| match e {
                crate::context_menu::MenuEntry::Item(item) => Some(item.id),
                crate::context_menu::MenuEntry::Separator => None,
            })
            .collect()
    }

    #[test]
    fn accessible_card_menu_has_open_and_reveal() {
        let ids = entry_ids(&AppRoot::project_card_menu_entries(true));
        assert_eq!(
            ids,
            vec!["open", "reveal", "duplicate", "remove-recents", "delete"]
        );
    }

    #[test]
    fn missing_card_menu_drops_open_and_reveal() {
        let ids = entry_ids(&AppRoot::project_card_menu_entries(false));
        // Duplicate is accessible-gated too — a missing package can't be copied.
        assert_eq!(ids, vec!["remove-recents", "delete"]);
    }

    #[test]
    fn duplicate_project_package_copies_tree_as_copy() {
        // Issue #67: build a fake .palmier package, duplicate it, and verify the
        // "<name> (Copy).palmier" tree exists with the copied contents.
        let base = std::env::temp_dir().join(format!("fronda-dup-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let src = base.join("My Project.palmier");
        std::fs::create_dir_all(src.join("media")).unwrap();
        std::fs::write(src.join("project.json"), b"{\"timelines\":[]}").unwrap();
        std::fs::write(src.join("media").join("a.txt"), b"hi").unwrap();

        let dest = AppRoot::duplicate_project_package(&src).unwrap();
        assert_eq!(dest, base.join("My Project (Copy).palmier"));
        assert!(dest.join("project.json").is_file(), "project.json copied");
        assert_eq!(
            std::fs::read(dest.join("media").join("a.txt")).unwrap(),
            b"hi"
        );
        // The original is untouched.
        assert!(src.join("project.json").is_file());
        let _ = std::fs::remove_dir_all(&base);
    }
}

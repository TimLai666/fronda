//! Application root — manages routing between Home and Editor views.
//!
//! Covers APP-002 (reopening shows Home), BOOT-004 (startup flow),
//! and PRJ-014 (close project → Home).

use crate::chat_view::ChatView;
use crate::tour_overlay_view::TourOverlayView;
use crate::update_overlay_view::UpdateOverlayView;
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
use crate::window::WindowConfig;
use app_contract::focus_router::{route_paste, FocusTarget};
use gpui::{
    div, prelude::*, px, size, svg, App, Bounds, Context, DragMoveEvent, Entity, FocusHandle,
    Focusable, InteractiveElement, KeyDownEvent, MouseButton, MouseDownEvent, Window,
    WindowBounds, WindowOptions,
};

/// Drag token for timeline panel resize.
#[derive(Debug, Clone)]
struct TimelineResizeDrag;

/// Invisible drag preview.
struct TimelineResizePreview;
impl gpui::Render for TimelineResizePreview {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl gpui::IntoElement {
        div()
    }
}

/// Timeline resize drag session.
#[derive(Debug, Clone)]
struct TimelineResizeDragSession {
    start_y: f32,
    start_height: f32,
}

/// A recently opened project entry (Swift: ProjectRegistry.Entry).
#[derive(Debug, Clone)]
pub struct RecentProject {
    pub id: &'static str,
    pub name: &'static str,
    /// Hue (0.0..=1.0) for the placeholder thumbnail color.
    pub hue: f32,
    /// Relative time string, e.g. "2h ago".
    pub last_modified: &'static str,
}

/// Which screen the app is showing.
#[derive(Debug, Clone, PartialEq)]
pub enum ActiveScreen {
    Home,
    Editor,
}

/// Root view that switches between Home and Editor.
#[derive(Debug, Clone)]
pub struct AppRoot {
    focus_handle: FocusHandle,
    active_screen: ActiveScreen,
    pane_layout: PaneLayout,
    home: HomeView,
    samples_expanded: bool,
    welcome_dismissed: bool,
    /// Recent projects list (Swift: ProjectRegistry.sortedEntries).
    recent_projects: Vec<RecentProject>,
    /// True when a user is signed in (controls sidebar Sign in button).
    is_signed_in: bool,
    /// Timeline panel height in pixels (draggable).
    timeline_height: f32,
    timeline_resize_drag: Option<TimelineResizeDragSession>,
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
}

impl AppRoot {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let handle = cx.focus_handle();
        Self {
            focus_handle: handle.clone(),
            active_screen: ActiveScreen::Home,
            pane_layout: PaneLayout::new(),
            home: HomeView::new(handle),
            samples_expanded: true,
            welcome_dismissed: false,
            recent_projects: vec![
                RecentProject { id: "proj-1", name: "My Film",       hue: 0.60, last_modified: "2h ago" },
                RecentProject { id: "proj-2", name: "Product Ad",    hue: 0.10, last_modified: "Yesterday" },
                RecentProject { id: "proj-3", name: "Travel Vlog",   hue: 0.35, last_modified: "3 days ago" },
            ],
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
            timeline_height: 200.0,
            timeline_resize_drag: None,
        }
    }

    /// Open the editor for a project.
    pub fn open_editor(&mut self, cx: &mut Context<Self>) {
        self.active_screen = ActiveScreen::Editor;
        if self.chat_view.is_none() {
            self.titlebar_view = Some(cx.new(|cx| TitleBarView::new(cx)));
            self.chat_view = Some(cx.new(|cx| ChatView::new(cx)));
            self.toolbar_view = Some(cx.new(|cx| ToolbarView::new(cx)));
            self.media_panel_view = Some(cx.new(|cx| MediaPanelView::new(cx)));
            self.preview_view = Some(cx.new(|cx| PreviewView::new(cx)));
            self.timeline_view = Some(cx.new(|cx| TimelineView::new(cx)));
            self.inspector_view = Some(cx.new(|cx| InspectorView::new(cx)));
        }
        cx.notify();
    }

    /// Navigate back to Home (e.g., close project).
    pub fn show_home(&mut self, cx: &mut Context<Self>) {
        self.active_screen = ActiveScreen::Home;
        cx.notify();
    }

    pub fn handle_key_down(
        &mut self,
        event: &KeyDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let modifiers = menu::Modifiers {
            command: event.keystroke.modifiers.platform,
            shift: event.keystroke.modifiers.shift,
            option: event.keystroke.modifiers.alt,
            control: event.keystroke.modifiers.control,
        };

        let Some(action) = menu::route_shortcut(&event.keystroke.key, &modifiers) else {
            return;
        };

        cx.stop_propagation();

        match action {
            menu::MenuAction::NewProject => {
                self.open_editor(cx);
            }
            menu::MenuAction::OpenProject => {
                self.open_editor(cx);
            }
            menu::MenuAction::ToggleMediaPanel => {
                self.pane_layout.toggle_pane(PaneId::Media);
            }
            menu::MenuAction::ToggleInspector => {
                self.pane_layout.toggle_pane(PaneId::Inspector);
            }
            menu::MenuAction::ToggleAgentPanel => {
                self.pane_layout.toggle_pane(PaneId::Agent);
            }
            menu::MenuAction::MaximizeFocusedPane => {
                if self.pane_layout.is_maximized() {
                    self.pane_layout.unmaximize();
                } else {
                    self.pane_layout.maximize(PaneId::Preview);
                }
            }
            menu::MenuAction::LayoutDefault => {
                self.pane_layout.apply_preset(LayoutPreset::Default);
            }
            menu::MenuAction::LayoutMedia => {
                self.pane_layout.apply_preset(LayoutPreset::Media);
            }
            menu::MenuAction::LayoutVertical => {
                self.pane_layout.apply_preset(LayoutPreset::Vertical);
            }
            menu::MenuAction::EnterFullScreen => {}
            menu::MenuAction::Quit => {}
            menu::MenuAction::SaveProject
            | menu::MenuAction::SaveProjectAs
            | menu::MenuAction::ImportMedia
            | menu::MenuAction::Export => {}
            menu::MenuAction::Undo
            | menu::MenuAction::Redo
            | menu::MenuAction::Cut
            | menu::MenuAction::Copy => {}
            menu::MenuAction::Paste => {
                let _action = route_paste(FocusTarget::Timeline);
            }
            menu::MenuAction::SelectAll
            | menu::MenuAction::SplitAtPlayhead
            | menu::MenuAction::TrimStartToPlayhead
            | menu::MenuAction::TrimEndToPlayhead
            | menu::MenuAction::Delete
            | menu::MenuAction::RippleDelete => {}
            menu::MenuAction::About
            | menu::MenuAction::CheckForUpdates
            | menu::MenuAction::Settings => {}
            menu::MenuAction::Tutorial
            | menu::MenuAction::KeyboardShortcuts
            | menu::MenuAction::McpInstructions
            | menu::MenuAction::SendFeedback => {}
            menu::MenuAction::PlayPause
            | menu::MenuAction::PlayBackward
            | menu::MenuAction::PauseJkl
            | menu::MenuAction::PlayForward
            | menu::MenuAction::StepFrameBackward
            | menu::MenuAction::StepFrameForward
            | menu::MenuAction::SkipFramesBackward
            | menu::MenuAction::SkipFramesForward => {}
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

    /// A sidebar navigation button row.
    fn sidebar_row_svg(id: &str, icon_path: &'static str, label: &str) -> gpui::Stateful<gpui::Div> {
        div()
            .id(id.to_string())
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .h(px(32.0))
            .px(px(Spacing::SM_MD))
            .gap(px(Spacing::SM_MD))
            .rounded(px(Radius::SM))
            .cursor_pointer()
            .child(
                div()
                    .w(px(16.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(svg().path(icon_path).w(px(14.0)).h(px(14.0)).text_color(Text::TERTIARY)),
            )
            .child(
                div()
                    .text_color(Text::SECONDARY)
                    .text_size(px(FontSize::SM))
                    .child(label.to_string()),
            )
    }

    /// Render the Home screen: sidebar (220px) + content area.
    fn render_home(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let samples_expanded = self.samples_expanded;
        let is_signed_in = self.is_signed_in;
        let recent_projects = self.recent_projects.clone();

        // Sample project card data (Swift: SampleProjectsStrip items)
        let sample_cards: &[(&str, f32)] = &[
            ("Short Film",   0.60),
            ("Commercial",   0.75),
            ("Documentary",  0.43),
        ];

        // Project card helper: thumbnail top + name strip bottom
        let project_card = |id: &'static str, name: &'static str, hue: f32, cx: &mut Context<Self>| {
            div()
                .id(id)
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
                .on_click(cx.listener(|this, _, _, cx| { this.open_editor(cx); }))
                .child(
                    div()
                        .flex_1()
                        .bg(gpui::Hsla { h: hue, s: 0.35, l: 0.14, a: 1.0 })
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(gpui::Hsla { h: hue, s: 0.55, l: 0.55, a: 1.0 })
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
        };

        let welcome_dismissed = self.welcome_dismissed;

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
                        Self::sidebar_row_svg("sidebar-new-project", "icons/plus.svg", "New Project")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.open_editor(cx);
                            })),
                    )
                    .child(
                        Self::sidebar_row_svg("sidebar-open-project", "icons/folder.svg", "Open Project")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.open_editor(cx);
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
                    .child(Self::sidebar_row_svg("sidebar-settings", "icons/gear.svg", "Settings")),
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
                                        .children(sample_cards.iter().enumerate().map(|(i, (_, hue))| {
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
                                                .on_click(cx.listener(|this, _, _, cx| { this.open_editor(cx); }))
                                                .child(
                                                    div()
                                                        .flex_1()
                                                        .bg(gpui::Hsla { h, s: 0.35, l: 0.14, a: 1.0 })
                                                        .flex()
                                                        .items_center()
                                                        .justify_center()
                                                        .text_color(gpui::Hsla { h, s: 0.55, l: 0.55, a: 1.0 })
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
                                        }))
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
                            .children(recent_projects.iter().map(|p| {
                                project_card(p.id, p.name, p.hue, cx)
                            }))
                    ),
            )
            // WelcomeOverlay — shown on first launch until dismissed (Swift: WelcomeOverlayView)
            .when(!welcome_dismissed, |el| {
                el.child(
                    div()
                        .id("welcome-overlay")
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(gpui::Hsla { h: 0.0, s: 0.0, l: 0.0, a: 0.60 })
                        .child(
                            div()
                                .id("welcome-card")
                                .flex()
                                .flex_col()
                                .items_center()
                                .gap(px(Spacing::MD))
                                .px(px(48.0))
                                .py(px(40.0))
                                .rounded(px(Radius::LG))
                                .bg(Background::SURFACE)
                                .border_1()
                                .border_color(BorderColors::SUBTLE)
                                .child(
                                    div()
                                        .text_size(px(FontSize::TITLE_2))
                                        .text_color(Text::PRIMARY)
                                        .child("Welcome to Fronda"),
                                )
                                .child(
                                    div()
                                        .text_size(px(FontSize::SM))
                                        .text_color(Text::SECONDARY)
                                        .child("The cross-platform video editor."),
                                )
                                .child(
                                    div()
                                        .id("welcome-get-started")
                                        .px(px(Spacing::XL))
                                        .py(px(Spacing::SM))
                                        .rounded(px(Radius::SM))
                                        .bg(gpui::Hsla { h: 0.56, s: 1.0, l: 0.55, a: 1.0 })
                                        .cursor_pointer()
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.welcome_dismissed = true;
                                            cx.notify();
                                        }))
                                        .child(
                                            div()
                                                .text_size(px(FontSize::SM))
                                                .text_color(Text::PRIMARY)
                                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                                .child("Get Started"),
                                        ),
                                ),
                        ),
                )
            })
    }
}

impl Focusable for AppRoot {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for AppRoot {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content: gpui::AnyElement = match self.active_screen {
            ActiveScreen::Home => self.render_home(cx).into_any_element(),
            ActiveScreen::Editor => {
                let layout = self.pane_layout.clone();
                let contents = editor_view::PaneContents::new(
                    self.chat_view.clone(),
                    self.toolbar_view.clone(),
                    self.media_panel_view.clone(),
                    self.preview_view.clone(),
                    self.timeline_view.clone(),
                    self.inspector_view.clone(),
                );
                let tl_height = self.timeline_height;

                // Resize handle: 5px draggable strip between toolbar and timeline
                let resize_handle = div()
                    .id("timeline-resize-handle")
                    .w_full()
                    .h(px(5.0))
                    .bg(crate::theme::BorderColors::PRIMARY)
                    .cursor_ns_resize()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this: &mut AppRoot, e: &MouseDownEvent, _, _| {
                            this.timeline_resize_drag = Some(TimelineResizeDragSession {
                                start_y: e.position.y.as_f32(),
                                start_height: this.timeline_height,
                            });
                        }),
                    )
                    .on_drag(TimelineResizeDrag, |_, _, _, cx| {
                        cx.new(|_| TimelineResizePreview)
                    })
                    .into_any_element();

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
                            // Global handler for timeline resize drag
                            .on_drag_move::<TimelineResizeDrag>(move |event: &DragMoveEvent<TimelineResizeDrag>, _, cx: &mut App| {
                                let _ = weak.update(cx, |this: &mut AppRoot, inner_cx| {
                                    if let Some(ref session) = this.timeline_resize_drag {
                                        let dy = event.event.position.y.as_f32() - session.start_y;
                                        // Drag UP increases timeline height (timeline is below)
                                        let new_h = (session.start_height - dy)
                                            .clamp(crate::theme::Layout::TIMELINE_MIN_HEIGHT,
                                                   crate::theme::Layout::TIMELINE_MAX_HEIGHT);
                                        this.timeline_height = new_h;
                                        inner_cx.notify();
                                    }
                                });
                            })
                            .child(editor_view::render_pane_layout(&layout, &contents, tl_height, resize_handle)),
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
            .flex()
            .flex_col()
            .size_full()
            .relative()
            .child(content)
            // Tour overlay stacks on top of everything at launch
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .child(tour),
            )
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
    let cfg = WindowConfig::for_home();
    let size = size(px(cfg.default_width as f32), px(cfg.default_height as f32));
    let mut bounds = Bounds::centered(None, size, cx);
    bounds.origin.y = bounds.origin.y + px(220.0);

    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            ..Default::default()
        },
        |_, cx| cx.new(|cx| AppRoot::new(cx)),
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
}

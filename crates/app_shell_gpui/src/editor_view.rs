use crate::chat_view::ChatView;
use crate::inspector_view::InspectorView;
use crate::media_panel_view::MediaPanelView;
use crate::pane::{PaneId, PaneLayout};
use crate::pane_tree::{PaneNode, PaneNodeKind, PaneSize};
use crate::preview_view::PreviewView;
use crate::theme::{Accent, Anim, Background, BorderWidth, Layout, Radius, Text};
use crate::timeline_view::TimelineView;
use crate::toolbar_view::ToolbarView;
use gpui::{
    div, prelude::*, px, Animation, AnimationExt as _, AnyElement, Entity, IntoElement,
    MouseButton, ParentElement, SharedString, Styled,
};
use std::time::Duration;

/// Human-readable label for each pane.
fn pane_label(id: PaneId) -> &'static str {
    match id {
        PaneId::Media => "Media",
        PaneId::Preview => "Preview",
        PaneId::Inspector => "Inspector",
        PaneId::Timeline => "Timeline",
        PaneId::Agent => "Agent",
    }
}

/// A placeholder pane div — background color + centered label.
fn pane_div(id: PaneId) -> impl IntoElement {
    let label = pane_label(id);
    let bg = match id {
        PaneId::Preview => Background::BASE,
        PaneId::Timeline => Background::SURFACE,
        _ => Background::SURFACE,
    };
    div()
        .id(format!("pane-{}", label.to_lowercase()))
        .flex()
        .items_center()
        .justify_center()
        .bg(bg)
        .w_full()
        .h_full()
        .text_color(Text::MUTED)
        .child(div().text_lg().child(label.to_string()))
}

/// Optional real content for panes that have view implementations.
#[derive(Clone)]
pub struct PaneContents {
    pub agent_chat: Option<Entity<ChatView>>,
    pub toolbar: Option<Entity<ToolbarView>>,
    pub media_panel: Option<Entity<MediaPanelView>>,
    pub preview: Option<Entity<PreviewView>>,
    pub timeline: Option<Entity<TimelineView>>,
    pub inspector: Option<Entity<InspectorView>>,
}

impl PaneContents {
    pub fn new(
        chat: Option<Entity<ChatView>>,
        toolbar: Option<Entity<ToolbarView>>,
        media_panel: Option<Entity<MediaPanelView>>,
        preview: Option<Entity<PreviewView>>,
        timeline: Option<Entity<TimelineView>>,
        inspector: Option<Entity<InspectorView>>,
    ) -> Self {
        Self {
            agent_chat: chat,
            toolbar,
            media_panel,
            preview,
            timeline,
            inspector,
        }
    }
}

fn agent_content(contents: &PaneContents) -> gpui::AnyElement {
    contents
        .agent_chat
        .clone()
        .map(|e| e.into_any_element())
        .unwrap_or_else(|| pane_div(PaneId::Agent).into_any_element())
}

fn toolbar_content(contents: &PaneContents) -> gpui::AnyElement {
    contents
        .toolbar
        .clone()
        .map(|e| e.into_any_element())
        .unwrap_or_else(|| {
            div()
                .id("toolbar-placeholder")
                .h(px(Layout::TOOLBAR_HEIGHT))
                .w_full()
                .bg(Background::RAISED)
                .flex()
                .items_center()
                .px(px(10.0))
                .text_color(Text::MUTED)
                .child("Toolbar")
                .into_any_element()
        })
}

fn media_panel_content(contents: &PaneContents) -> gpui::AnyElement {
    contents
        .media_panel
        .clone()
        .map(|e| e.into_any_element())
        .unwrap_or_else(|| pane_div(PaneId::Media).into_any_element())
}

fn preview_content(contents: &PaneContents) -> gpui::AnyElement {
    contents
        .preview
        .clone()
        .map(|e| e.into_any_element())
        .unwrap_or_else(|| pane_div(PaneId::Preview).into_any_element())
}

fn timeline_content(contents: &PaneContents) -> gpui::AnyElement {
    contents
        .timeline
        .clone()
        .map(|e| e.into_any_element())
        .unwrap_or_else(|| pane_div(PaneId::Timeline).into_any_element())
}

fn inspector_content(contents: &PaneContents) -> gpui::AnyElement {
    contents
        .inspector
        .clone()
        .map(|e| e.into_any_element())
        .unwrap_or_else(|| pane_div(PaneId::Inspector).into_any_element())
}

// ── Tree-driven rendering ──

/// Host hook a pane card's left mouse-down runs (capture phase) to move
/// panel focus; AppRoot updates `focused_pane` in it.
pub type PaneFocusHandler = std::rc::Rc<dyn Fn(PaneId, &mut gpui::Window, &mut gpui::App)>;

/// Focus wiring for pane cards (EDT-007): which pane owns the focus ring,
/// and the handler that moves it on click.
pub struct PaneFocus {
    pub focused: Option<PaneId>,
    pub on_mouse_down: PaneFocusHandler,
}

/// Swift PanelFocusRing stroke: `Accent.primary` at the 0.6 opacity literal
/// PanelFocusRing hardcodes (EditorView.swift).
pub(crate) const FOCUS_RING_OPACITY: f32 = 0.6;
pub(crate) const FOCUS_RING_WIDTH: f32 = BorderWidth::MEDIUM;
pub(crate) const FOCUS_RING_RADIUS: f32 = Radius::SM;

pub(crate) fn focus_ring_color() -> gpui::Hsla {
    gpui::Hsla {
        a: FOCUS_RING_OPACITY,
        ..Accent::PRIMARY
    }
}

/// Quadratic ease-out (SwiftUI `.easeOut` shape): fast start, decelerating.
fn ease_out_quad(delta: f32) -> f32 {
    1.0 - (1.0 - delta) * (1.0 - delta)
}

/// Non-interactive accent ring overlaying a focused card (Swift
/// PanelFocusRing).
fn focus_ring() -> gpui::Div {
    div()
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .rounded(px(FOCUS_RING_RADIUS))
        .border(px(FOCUS_RING_WIDTH))
        .border_color(focus_ring_color())
}

/// The ring with Swift PanelFocusRing's appearance animation: 0.2s easeOut
/// opacity fade-in. The element only exists while its pane is focused, so
/// gpui's per-element animation state restarts on every focus move — the
/// fade-out half of the Swift animation has no element to run on and is
/// instant (recorded deviation).
fn focus_ring_fading_in(pane: PaneId) -> impl IntoElement {
    focus_ring().with_animation(
        SharedString::from(format!("focus-ring-{}", pane_label(pane))),
        Animation::new(Duration::from_millis((Anim::TRANSITION * 1000.0) as u64))
            .with_easing(ease_out_quad),
        |ring, delta| ring.opacity(delta),
    )
}

/// Swift makeHosting panel shell: surface rounded card inset by half the
/// panel gap against the base background, so adjacent panes show a 5px seam.
/// Left mouse-down (capture, non-consuming) moves panel focus to `id`;
/// the focused card draws the accent ring.
fn pane_card(id: PaneId, focus: &PaneFocus, inner: AnyElement) -> gpui::Div {
    let handler = focus.on_mouse_down.clone();
    let focused = focus.focused == Some(id);
    div().size_full().p(px(Layout::PANEL_GAP / 2.0)).child(
        div()
            .size_full()
            .relative()
            .bg(Background::SURFACE)
            .rounded(px(Radius::SM))
            .overflow_hidden()
            .capture_any_mouse_down(move |event, window, cx| {
                if event.button == MouseButton::Left {
                    handler(id, window, cx);
                }
            })
            .child(inner)
            .when(focused, |el| el.child(focus_ring_fading_in(id))),
    )
}

fn pane_content(id: PaneId, contents: &PaneContents) -> AnyElement {
    match id {
        PaneId::Agent => agent_content(contents),
        PaneId::Media => media_panel_content(contents),
        PaneId::Preview => preview_content(contents),
        PaneId::Inspector => inspector_content(contents),
        // Timeline renders through TimelineRegion, never as a bare leaf.
        PaneId::Timeline => timeline_content(contents),
    }
}

fn apply_size(node_div: gpui::Div, size: PaneSize, horizontal_axis: bool) -> gpui::Div {
    match (size, horizontal_axis) {
        (PaneSize::Fixed(v), true) => node_div.w(px(v)).h_full(),
        (PaneSize::Fixed(v), false) => node_div.h(px(v)).w_full(),
        (PaneSize::Flex, _) => node_div.flex_1().min_w(px(0.0)).min_h(px(0.0)),
    }
}

/// Pre-built divider hitbox elements, one per resize target (a tree contains
/// each target at most once). Built by the host so listeners bind to it.
pub type DividerElements = Vec<(crate::pane_resize::ResizeTarget, AnyElement)>;

/// Recursively render a PaneNode. `horizontal_axis` is the PARENT's axis
/// (true = this node's size applies to width). Divider leaves consume their
/// pre-built element from `dividers`.
fn render_node(
    node: &PaneNode,
    contents: &PaneContents,
    focus: &PaneFocus,
    dividers: &mut DividerElements,
    horizontal_axis: bool,
) -> AnyElement {
    match &node.kind {
        PaneNodeKind::Row(children) => {
            let mut row = div().flex().flex_row();
            row = apply_size(row, node.size, horizontal_axis);
            for child in children {
                row = row.child(render_node(child, contents, focus, dividers, true));
            }
            row.into_any_element()
        }
        PaneNodeKind::Column(children) => {
            let mut col = div().flex().flex_col();
            col = apply_size(col, node.size, horizontal_axis);
            for child in children {
                col = col.child(render_node(child, contents, focus, dividers, false));
            }
            col.into_any_element()
        }
        PaneNodeKind::Pane(id) => {
            let card = pane_card(*id, focus, pane_content(*id, contents));
            apply_size(card, node.size, horizontal_axis).into_any_element()
        }
        // Toolbar + Timeline composite card (Swift timelineHC VStack).
        PaneNodeKind::TimelineRegion => {
            let inner = div()
                .size_full()
                .flex()
                .flex_col()
                .child(toolbar_content(contents))
                .child(
                    div()
                        .flex_1()
                        .min_h(px(0.0))
                        .child(timeline_content(contents)),
                );
            let card = pane_card(PaneId::Timeline, focus, inner.into_any_element());
            apply_size(card, node.size, horizontal_axis).into_any_element()
        }
        // Zero-sized seam anchor; the host's hitbox overlays the gap.
        PaneNodeKind::Divider { target, .. } => {
            let elem = dividers
                .iter()
                .position(|(t, _)| t == target)
                .map(|i| dividers.remove(i).1);
            let anchor = if horizontal_axis {
                div().w(px(0.0)).h_full()
            } else {
                div().h(px(0.0)).w_full()
            };
            anchor
                .flex_none()
                .relative()
                .children(elem)
                .into_any_element()
        }
    }
}

/// Render the full editor pane layout from the pure description tree.
pub fn render_pane_layout(
    layout: &PaneLayout,
    contents: &PaneContents,
    focus: &PaneFocus,
    sizes: &crate::pane_tree::ResolvedSizes,
    mut dividers: DividerElements,
) -> impl IntoElement {
    let tree = crate::pane_tree::build_pane_tree(layout, sizes);
    div()
        .id("editor-pane-layout")
        .size_full()
        .flex()
        .bg(Background::BASE)
        // Outer half-gap so the window edge shows the same seam as between panes.
        .p(px(Layout::PANEL_GAP / 2.0))
        .child(render_node(&tree, contents, focus, &mut dividers, true))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pane_label_covers_all_ids() {
        assert_eq!(pane_label(PaneId::Media), "Media");
        assert_eq!(pane_label(PaneId::Preview), "Preview");
        assert_eq!(pane_label(PaneId::Inspector), "Inspector");
        assert_eq!(pane_label(PaneId::Timeline), "Timeline");
        assert_eq!(pane_label(PaneId::Agent), "Agent");
    }

    #[test]
    fn focus_ring_matches_swift_panel_focus_ring() {
        // Swift PanelFocusRing: Accent.primary stroke, BorderWidth.medium
        // line width, Radius.sm continuous corner, 0.6 opacity when focused.
        let color = focus_ring_color();
        assert_eq!(color.h, Accent::PRIMARY.h);
        assert_eq!(color.s, Accent::PRIMARY.s);
        assert_eq!(color.l, Accent::PRIMARY.l);
        assert!((color.a - 0.6).abs() < 1e-6);
        assert_eq!(FOCUS_RING_WIDTH, BorderWidth::MEDIUM);
        assert_eq!(FOCUS_RING_RADIUS, Radius::SM);
    }

    #[test]
    fn ring_fade_matches_swift_ease_out_transition() {
        // Swift: .easeOut(duration: Anim.transition) — 0.2s, decelerating,
        // 0 → full opacity endpoints.
        assert!((Anim::TRANSITION - 0.2).abs() < 1e-6);
        assert_eq!(ease_out_quad(0.0), 0.0);
        assert_eq!(ease_out_quad(1.0), 1.0);
        // Ease-OUT: the first half covers more than half the distance.
        assert!(ease_out_quad(0.5) > 0.5);
        // Monotonic on [0, 1].
        assert!(ease_out_quad(0.25) < ease_out_quad(0.5));
        assert!(ease_out_quad(0.5) < ease_out_quad(0.75));
    }

    #[test]
    fn pane_contents_fields_are_optional() {
        let c = PaneContents::new(None, None, None, None, None, None);
        assert!(c.agent_chat.is_none());
        assert!(c.toolbar.is_none());
        assert!(c.media_panel.is_none());
        assert!(c.preview.is_none());
        assert!(c.timeline.is_none());
        assert!(c.inspector.is_none());
    }
}

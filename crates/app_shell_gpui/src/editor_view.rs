use crate::chat_view::ChatView;
use crate::media_panel_view::MediaPanelView;
use crate::pane::{LayoutPreset, PaneId, PaneLayout};
use crate::theme::{Background, BorderColors, Layout, Text};
use crate::toolbar_view::ToolbarView;
use gpui::{div, px, Entity, IntoElement, ParentElement, Styled};

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
}

impl PaneContents {
    pub fn new(
        chat: Option<Entity<ChatView>>,
        toolbar: Option<Entity<ToolbarView>>,
        media_panel: Option<Entity<MediaPanelView>>,
    ) -> Self {
        Self {
            agent_chat: chat,
            toolbar,
            media_panel,
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

// ── Layout builders ──

/// Default preset: [Agent left?] | Media | Preview + Toolbar + Timeline | Inspector
///
/// Agent panel is a LEFT column sibling to the rest (matches Swift layout).
fn build_default(layout: &PaneLayout, contents: &PaneContents) -> impl IntoElement {
    let mut root = div().id("layout-default").flex().flex_row().size_full();

    // Agent panel: LEFT column (240px min, Swift: AGENT_PANEL_MIN=240)
    if layout.is_visible(PaneId::Agent) {
        root = root.child(
            div()
                .w(px(Layout::AGENT_PANEL_MIN))
                .h_full()
                .border_r_1()
                .border_color(BorderColors::PRIMARY)
                .child(agent_content(contents)),
        );
    }

    // Media panel: 500px default (Layout::MEDIA_PANEL_DEFAULT)
    if layout.is_visible(PaneId::Media) {
        root = root.child(
            div()
                .w(px(Layout::MEDIA_PANEL_DEFAULT))
                .h_full()
                .border_r_1()
                .border_color(BorderColors::PRIMARY)
                .child(media_panel_content(contents)),
        );
    }

    // Center column: Preview + Toolbar + Timeline
    let mut center = div().flex().flex_col().flex_1().h_full();

    if layout.is_visible(PaneId::Preview) {
        center = center.child(
            div()
                .flex_1()
                .border_b_1()
                .border_color(BorderColors::PRIMARY)
                .child(pane_div(PaneId::Preview)),
        );
    }

    // Toolbar between Preview and Timeline (UIX-001)
    center = center.child(toolbar_content(contents));

    if layout.is_visible(PaneId::Timeline) {
        center = center.child(
            div()
                .h(px(200.0_f32))
                .border_t_1()
                .border_color(BorderColors::PRIMARY)
                .child(pane_div(PaneId::Timeline)),
        );
    }

    root = root.child(center);

    // Inspector: Layout::INSPECTOR_DEFAULT = 260px
    if layout.is_visible(PaneId::Inspector) {
        root = root.child(
            div()
                .w(px(Layout::INSPECTOR_DEFAULT))
                .h_full()
                .border_l_1()
                .border_color(BorderColors::PRIMARY)
                .child(pane_div(PaneId::Inspector)),
        );
    }

    root
}

/// Media preset: media (wide) | preview.
fn build_media(layout: &PaneLayout, contents: &PaneContents) -> impl IntoElement {
    let mut root = div().id("layout-media").flex().flex_row().size_full();

    if layout.is_visible(PaneId::Media) {
        root = root.child(
            div()
                .flex_1()
                .h_full()
                .border_r_1()
                .border_color(BorderColors::PRIMARY)
                .child(media_panel_content(contents)),
        );
    }

    if layout.is_visible(PaneId::Preview) {
        root = root.child(
            div()
                .flex_1()
                .h_full()
                .child(pane_div(PaneId::Preview)),
        );
    }

    root
}

/// Vertical preset (Swift: Media+Inspector stacked left / Toolbar+Timeline left | Preview right).
fn build_vertical(layout: &PaneLayout, contents: &PaneContents) -> impl IntoElement {
    let mut root = div().id("layout-vertical").flex().flex_row().size_full();

    // Left stack: media + inspector (stacked vertically)
    let mut left = div().flex().flex_col().w(px(300.0)).h_full();

    if layout.is_visible(PaneId::Media) {
        left = left.child(
            div()
                .flex_1()
                .border_b_1()
                .border_color(BorderColors::PRIMARY)
                .child(media_panel_content(contents)),
        );
    }

    if layout.is_visible(PaneId::Inspector) {
        left = left.child(
            div()
                .h(px(240.0))
                .child(pane_div(PaneId::Inspector)),
        );
    }

    root = root.child(
        left.border_r_1()
            .border_color(BorderColors::PRIMARY),
    );

    // Right: Preview fills remaining space
    if layout.is_visible(PaneId::Preview) {
        root = root.child(
            div()
                .flex_1()
                .h_full()
                .flex()
                .flex_col()
                .child(
                    div()
                        .flex_1()
                        .child(pane_div(PaneId::Preview)),
                )
                .child(toolbar_content(contents))
                .child(
                    div()
                        .h(px(180.0))
                        .border_t_1()
                        .border_color(BorderColors::PRIMARY)
                        .child(pane_div(PaneId::Timeline)),
                ),
        );
    }

    root
}

/// Render the full editor pane layout.
pub fn render_pane_layout(layout: &PaneLayout, contents: &PaneContents) -> impl IntoElement {
    match layout.preset {
        LayoutPreset::Default => build_default(layout, contents).into_any_element(),
        LayoutPreset::Media => build_media(layout, contents).into_any_element(),
        LayoutPreset::Vertical => build_vertical(layout, contents).into_any_element(),
    }
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
    fn pane_contents_fields_are_optional() {
        let c = PaneContents::new(None, None, None);
        assert!(c.agent_chat.is_none());
        assert!(c.toolbar.is_none());
        assert!(c.media_panel.is_none());
    }
}

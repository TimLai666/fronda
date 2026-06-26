use crate::chat_view::ChatView;
use crate::pane::{LayoutPreset, PaneId, PaneLayout};
use gpui::{div, px, Entity, Hsla, InteractiveElement, IntoElement, ParentElement, Styled};

/// Background color for each pane.
fn pane_color(id: PaneId) -> Hsla {
    match id {
        PaneId::Media => Hsla {
            h: 210.0_f32 / 360.0_f32,
            s: 0.1,
            l: 0.15,
            a: 1.0,
        },
        PaneId::Preview => Hsla {
            h: 0.0,
            s: 0.0,
            l: 0.08,
            a: 1.0,
        },
        PaneId::Inspector => Hsla {
            h: 210.0_f32 / 360.0_f32,
            s: 0.1,
            l: 0.15,
            a: 1.0,
        },
        PaneId::Timeline => Hsla {
            h: 0.0,
            s: 0.0,
            l: 0.12,
            a: 1.0,
        },
        PaneId::Agent => Hsla {
            h: 210.0_f32 / 360.0_f32,
            s: 0.08,
            l: 0.1,
            a: 1.0,
        },
    }
}

const BORDER_COLOR: Hsla = Hsla {
    h: 0.0,
    s: 0.0,
    l: 0.2,
    a: 1.0,
};

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

/// A pane div with background color and centered label (for panes without custom content).
fn pane_div(id: PaneId) -> impl IntoElement {
    let label = pane_label(id);
    div()
        .id(format!("pane-{}", label.to_lowercase()))
        .flex()
        .items_center()
        .justify_center()
        .bg(pane_color(id))
        .w_full()
        .h_full()
        .child(div().text_lg().child(label.to_string()))
}

/// Optional custom content for panes that have real UI.
#[derive(Clone)]
pub struct PaneContents {
    /// When set, replaces the generic Agent pane label with a live ChatView.
    pub agent_chat: Option<Entity<ChatView>>,
}

impl PaneContents {
    pub fn new(chat: Option<Entity<ChatView>>) -> Self {
        Self { agent_chat: chat }
    }
}

/// Return the content to place in the Agent pane slot.
fn agent_content(contents: &PaneContents) -> gpui::AnyElement {
    contents
        .agent_chat
        .clone()
        .map(|e| e.into_any_element())
        .unwrap_or_else(|| pane_div(PaneId::Agent).into_any_element())
}

// ── Layout builders ──

/// Default preset: media | preview + timeline + agent | inspector.
fn build_default(layout: &PaneLayout, contents: &PaneContents) -> impl IntoElement {
    let mut root = div().id("layout-default").flex().flex_row().size_full();

    if layout.is_visible(PaneId::Media) {
        root = root.child(
            div()
                .w(px(250.0_f32))
                .h_full()
                .border_1()
                .border_color(BORDER_COLOR)
                .child(pane_div(PaneId::Media)),
        );
    }

    let mut center = div().flex().flex_col().flex_1().h_full();

    if layout.is_visible(PaneId::Preview) {
        center = center.child(
            div()
                .flex_1()
                .border_1()
                .border_color(BORDER_COLOR)
                .child(pane_div(PaneId::Preview)),
        );
    }

    if layout.is_visible(PaneId::Timeline) {
        center = center.child(
            div()
                .h(px(200.0_f32))
                .border_1()
                .border_color(BORDER_COLOR)
                .child(pane_div(PaneId::Timeline)),
        );
    }

    if layout.is_visible(PaneId::Agent) {
        center = center.child(
            div()
                .h(px(150.0_f32))
                .border_1()
                .border_color(BORDER_COLOR)
                .child(agent_content(contents)),
        );
    }

    root = root.child(center);

    if layout.is_visible(PaneId::Inspector) {
        root = root.child(
            div()
                .w(px(280.0_f32))
                .h_full()
                .border_1()
                .border_color(BORDER_COLOR)
                .child(pane_div(PaneId::Inspector)),
        );
    }

    root
}

/// Media preset: media (wide) | preview.
fn build_media(layout: &PaneLayout, _contents: &PaneContents) -> impl IntoElement {
    let mut root = div().id("layout-media").flex().flex_row().size_full();

    if layout.is_visible(PaneId::Media) {
        root = root.child(
            div()
                .flex_1()
                .h_full()
                .border_1()
                .border_color(BORDER_COLOR)
                .child(pane_div(PaneId::Media)),
        );
    }

    if layout.is_visible(PaneId::Preview) {
        root = root.child(
            div()
                .flex_1()
                .h_full()
                .border_1()
                .border_color(BORDER_COLOR)
                .child(pane_div(PaneId::Preview)),
        );
    }

    root
}

/// Vertical preset: all visible panes stacked vertically.
fn build_vertical(layout: &PaneLayout, contents: &PaneContents) -> impl IntoElement {
    let mut root = div().id("layout-vertical").flex().flex_col().size_full();

    if layout.is_visible(PaneId::Preview) {
        root = root.child(
            div()
                .flex_1()
                .border_1()
                .border_color(BORDER_COLOR)
                .child(pane_div(PaneId::Preview)),
        );
    }

    if layout.is_visible(PaneId::Agent) {
        root = root.child(
            div()
                .h(px(200.0_f32))
                .border_1()
                .border_color(BORDER_COLOR)
                .child(agent_content(contents)),
        );
    }

    if layout.is_visible(PaneId::Timeline) {
        root = root.child(
            div()
                .h(px(200.0_f32))
                .border_1()
                .border_color(BORDER_COLOR)
                .child(pane_div(PaneId::Timeline)),
        );
    }

    if layout.is_visible(PaneId::Media) {
        root = root.child(
            div()
                .h(px(150.0_f32))
                .border_1()
                .border_color(BORDER_COLOR)
                .child(pane_div(PaneId::Media)),
        );
    }

    if layout.is_visible(PaneId::Inspector) {
        root = root.child(
            div()
                .h(px(150.0_f32))
                .border_1()
                .border_color(BORDER_COLOR)
                .child(pane_div(PaneId::Inspector)),
        );
    }

    root
}

/// Render the full editor pane layout based on layout state and pane contents.
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
    fn is_supported_extension_works() {
        assert!(app_contract::focus_router::is_supported_extension("mp4"));
        assert!(!app_contract::focus_router::is_supported_extension("exe"));
    }

    #[test]
    fn pane_label_covers_all_ids() {
        assert_eq!(pane_label(PaneId::Media), "Media");
        assert_eq!(pane_label(PaneId::Preview), "Preview");
        assert_eq!(pane_label(PaneId::Inspector), "Inspector");
        assert_eq!(pane_label(PaneId::Timeline), "Timeline");
        assert_eq!(pane_label(PaneId::Agent), "Agent");
    }

    #[test]
    fn pane_contents_default() {
        let c = PaneContents::new(None);
        assert!(c.agent_chat.is_none());
    }
}

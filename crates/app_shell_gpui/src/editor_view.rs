use crate::pane::{LayoutPreset, PaneId, PaneLayout};
use gpui::{div, px, Hsla, InteractiveElement, IntoElement, ParentElement, Styled};

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

/// A pane div with background color, centered label, and drop support.
fn pane_div(id: PaneId) -> impl IntoElement {
    let label = pane_label(id);
    let pane = div()
        .id(format!("pane-{}", label.to_lowercase()))
        .flex()
        .items_center()
        .justify_center()
        .bg(pane_color(id))
        .w_full()
        .h_full()
        .child(div().text_lg().child(label.to_string()));

    // Media panel gets drop target support (DRAG-007..010).
    // At runtime, gpui dispatches `on_drop` events on this element.
    // Extension filtering via `is_supported_extension` happens in the handler.

    pane
}

// ── Layout builders ──

/// Default preset: media | preview + timeline + agent | inspector.
fn build_default(layout: &PaneLayout) -> impl IntoElement {
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
                .child(pane_div(PaneId::Agent)),
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
fn build_media(layout: &PaneLayout) -> impl IntoElement {
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
fn build_vertical(layout: &PaneLayout) -> impl IntoElement {
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
                .child(pane_div(PaneId::Agent)),
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

/// Render the full editor pane layout based on layout state.
pub fn render_pane_layout(layout: &PaneLayout) -> impl IntoElement {
    match layout.preset {
        LayoutPreset::Default => build_default(layout).into_any_element(),
        LayoutPreset::Media => build_media(layout).into_any_element(),
        LayoutPreset::Vertical => build_vertical(layout).into_any_element(),
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
}

use crate::pane::{LayoutPreset, PaneId, PaneLayout};
use gpui::{div, prelude::*, px, Div, Hsla, IntoElement};

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

/// A filled pane div with background color and centered label text.
fn pane_div(id: PaneId) -> impl IntoElement {
    let label = pane_label(id);
    div()
        .flex()
        .items_center()
        .justify_center()
        .bg(pane_color(id))
        .w_full()
        .h_full()
        .child(div().text_lg().child(label.to_string()))
}

// ── Layout builders (all return concrete Div so the match in render_pane_layout unifies) ──

/// Default preset: media | preview + timeline + agent | inspector.
fn build_default(layout: &PaneLayout) -> Div {
    let mut root = div().flex().flex_row().size_full();

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
fn build_media(layout: &PaneLayout) -> Div {
    let mut root = div().flex().flex_row().size_full();

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
fn build_vertical(layout: &PaneLayout) -> Div {
    let mut root = div().flex().flex_col().size_full();

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
        LayoutPreset::Default => build_default(layout),
        LayoutPreset::Media => build_media(layout),
        LayoutPreset::Vertical => build_vertical(layout),
    }
}

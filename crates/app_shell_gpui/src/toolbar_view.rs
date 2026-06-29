//! Toolbar gpui view — 38px strip with undo/redo, tool mode, edit actions, zoom.
//!
//! Covers UIX-001 (height 38), UIX-002 (button set), UIX-003 (V/C shortcuts),
//! UIX-007 (zoom bounds).

use crate::theme::{
    Accent, Background, BorderColors, FontSize, Layout, Opacity, Radius, Spacing, Text,
};
use crate::toolbar_model::{ToolMode, ToolbarState};
use gpui::{
    div, prelude::*, px, svg, App, Context, FocusHandle, Focusable, Hsla, InteractiveElement,
    IntoElement, ParentElement, Render, Styled, Window,
};

/// Toolbar gpui entity.
pub struct ToolbarView {
    pub state: ToolbarState,
    focus_handle: FocusHandle,
}

impl ToolbarView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            state: ToolbarState::new(),
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn set_tool_mode(&mut self, mode: ToolMode, cx: &mut Context<Self>) {
        self.state.set_tool_mode(mode);
        cx.notify();
    }

    pub fn set_zoom(&mut self, scale: f32, cx: &mut Context<Self>) {
        self.state.set_zoom(scale);
        cx.notify();
    }

    pub fn set_undo_redo(&mut self, can_undo: bool, can_redo: bool, cx: &mut Context<Self>) {
        self.state.set_undo_redo(can_undo, can_redo);
        cx.notify();
    }
}

impl Focusable for ToolbarView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

/// Active tool bg: white@6% (Swift: hoverHighlight isActive).
const TOOL_ACTIVE_BG: Hsla = Hsla {
    h: 0.0,
    s: 0.0,
    l: 1.0,
    a: Opacity::HINT, // 0.06
};

/// 24×24 toolbar button with hover-highlight support.
/// `icon_path` is a path like "icons/undo.svg" served by FrondaAssets.
fn tool_btn_svg(
    id: &str,
    icon_path: &'static str,
    active: bool,
    enabled: bool,
) -> gpui::Stateful<gpui::Div> {
    let color = if !enabled {
        Text::MUTED
    } else if active {
        Text::PRIMARY
    } else {
        Text::SECONDARY
    };
    div()
        .id(id.to_string())
        .w(px(24.0))
        .h(px(24.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(Radius::XS_SM))
        .cursor_pointer()
        .bg(if active {
            TOOL_ACTIVE_BG
        } else {
            Background::RAISED
        })
        .child(
            svg()
                .path(icon_path)
                .w(px(14.0))
                .h(px(14.0))
                .text_color(color),
        )
}

/// 24×24 toolbar button with a text glyph (fallback when no SVG is available).
fn tool_btn(id: &str, glyph: &str, active: bool, enabled: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id.to_string())
        .w(px(24.0))
        .h(px(24.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(Radius::XS_SM))
        .cursor_pointer()
        .bg(if active {
            TOOL_ACTIVE_BG
        } else {
            Background::RAISED
        })
        .text_color(if !enabled {
            Text::MUTED
        } else if active {
            Text::PRIMARY
        } else {
            Text::SECONDARY
        })
        .text_size(px(FontSize::SM_MD))
        .child(glyph.to_string())
}

impl Render for ToolbarView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pointer_active = self.state.tool_mode == ToolMode::Pointer;
        let razor_active = self.state.tool_mode == ToolMode::Razor;
        let can_undo = self.state.can_undo;
        let can_redo = self.state.can_redo;

        // Zoom: 4px/frame default maps to ~4% of log range; approximate display
        let zoom_scale = self.state.zoom_scale;
        let zoom_pct = (zoom_scale / 4.0 * 100.0).round() as i32;
        let zoom_pct_label = format!("{zoom_pct}%");

        // Zoom slider fill: 0..1 in log space (log min=0.05, log max=40, log default=4)
        let log_min = 0.05_f32.ln();
        let log_max = 40.0_f32.ln();
        let log_val = zoom_scale.max(0.05).ln();
        let zoom_frac = ((log_val - log_min) / (log_max - log_min)).clamp(0.0, 1.0);
        let track_w = 100.0_f32;
        let fill_w = zoom_frac * track_w;

        div()
            .id("toolbar")
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .h(px(Layout::TOOLBAR_HEIGHT))
            .px(px(Spacing::MD))
            .gap(px(Spacing::MD))
            .bg(Background::RAISED)
            .border_b_1()
            .border_color(BorderColors::SUBTLE)
            // ── Undo / Redo ──
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(Spacing::SM))
                    .child(
                        tool_btn_svg("btn-undo", "icons/undo.svg", false, can_undo)
                            .on_click(cx.listener(|_, _, _, _| {})),
                    )
                    .child(
                        tool_btn_svg("btn-redo", "icons/redo.svg", false, can_redo)
                            .on_click(cx.listener(|_, _, _, _| {})),
                    ),
            )
            // ── Divider ──
            .child(toolbar_sep())
            // ── Tool mode: Pointer / Razor ──
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(Spacing::SM))
                    .child(
                        tool_btn_svg("btn-pointer", "icons/cursor.svg", pointer_active, true)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.set_tool_mode(ToolMode::Pointer, cx);
                            })),
                    )
                    .child(
                        tool_btn_svg("btn-razor", "icons/razor.svg", razor_active, true).on_click(
                            cx.listener(|this, _, _, cx| {
                                this.set_tool_mode(ToolMode::Razor, cx);
                            }),
                        ),
                    ),
            )
            // ── Divider ──
            .child(toolbar_sep())
            // ── Edit actions: Split / Trim [ / Trim ] ──
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(Spacing::SM))
                    .child(
                        tool_btn_svg("btn-split", "icons/split.svg", false, true)
                            .on_click(cx.listener(|_, _, _, _| {})),
                    )
                    .child(
                        tool_btn("btn-trim-start", "[", false, true)
                            .on_click(cx.listener(|_, _, _, _| {})),
                    )
                    .child(
                        tool_btn("btn-trim-end", "]", false, true)
                            .on_click(cx.listener(|_, _, _, _| {})),
                    ),
            )
            // ── Divider ──
            .child(toolbar_sep())
            // ── Add text (serif T — matches Swift) ──
            .child(
                div()
                    .id("btn-add-text")
                    .w(px(24.0))
                    .h(px(24.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(Radius::XS_SM))
                    .cursor_pointer()
                    .text_color(Text::SECONDARY)
                    .text_size(px(FontSize::MD_LG))
                    .font_weight(gpui::FontWeight::BOLD)
                    .on_click(cx.listener(|_, _, _, _| {}))
                    .child("T"),
            )
            // ── Spacer ──
            .child(div().flex_1())
            // ── Zoom controls ──
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(Spacing::XS))
                    .child(
                        div()
                            .id("btn-zoom-out")
                            .w(px(20.0))
                            .h(px(20.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .text_color(Text::SECONDARY)
                            .text_size(px(FontSize::SM_MD))
                            .on_click(cx.listener(|this, _, _, cx| {
                                let new_scale = this.state.zoom_scale / 1.5;
                                this.set_zoom(new_scale, cx);
                            }))
                            .child("⊖"),
                    )
                    // Zoom track with progress fill
                    .child(
                        div()
                            .id("zoom-track")
                            .relative()
                            .w(px(track_w))
                            .h(px(4.0))
                            .rounded_full()
                            .bg(BorderColors::SUBTLE)
                            // Fill progress
                            .child(
                                div()
                                    .absolute()
                                    .top_0()
                                    .left_0()
                                    .h_full()
                                    .w(px(fill_w))
                                    .rounded_full()
                                    .bg(Accent::PRIMARY),
                            ),
                    )
                    .child(
                        div()
                            .id("btn-zoom-in")
                            .w(px(20.0))
                            .h(px(20.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .text_color(Text::SECONDARY)
                            .text_size(px(FontSize::SM_MD))
                            .on_click(cx.listener(|this, _, _, cx| {
                                let new_scale = this.state.zoom_scale * 1.5;
                                this.set_zoom(new_scale, cx);
                            }))
                            .child("⊕"),
                    )
                    // Zoom percentage label
                    .child(
                        div()
                            .w(px(40.0))
                            .text_color(Text::TERTIARY)
                            .text_size(px(FontSize::XS))
                            .child(zoom_pct_label),
                    ),
            )
    }
}

fn toolbar_sep() -> impl IntoElement {
    div()
        .w(px(1.0))
        .h(px(Spacing::XL))
        .bg(BorderColors::PRIMARY)
}

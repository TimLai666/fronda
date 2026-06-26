//! Toolbar gpui view — 38px strip with undo/redo, tool mode, edit actions, zoom.
//!
//! Covers UIX-001 (height 38), UIX-002 (button set), UIX-003 (V/C shortcuts),
//! UIX-007 (zoom bounds).

use crate::theme::{Background, BorderColors, Layout, Radius, Spacing, Text};
use crate::toolbar_model::{ToolMode, ToolbarState};
use gpui::{
    div, prelude::*, px, App, Context, FocusHandle, Focusable, IntoElement, InteractiveElement,
    ParentElement, Render, Styled, Window,
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

impl Render for ToolbarView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pointer_active = self.state.tool_mode == ToolMode::Pointer;
        let razor_active = self.state.tool_mode == ToolMode::Razor;
        let can_undo = self.state.can_undo;
        let can_redo = self.state.can_redo;

        let tool_active_bg = BorderColors::PRIMARY;

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
            // ── Undo / Redo ──
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(Spacing::MD))
                    // Undo
                    .child(
                        div()
                            .id("btn-undo")
                            .w(px(24.0))
                            .h(px(24.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(Radius::XS_SM))
                            .cursor_pointer()
                            .text_color(if can_undo { Text::SECONDARY } else { Text::MUTED })
                            .child("↩"),
                    )
                    // Redo
                    .child(
                        div()
                            .id("btn-redo")
                            .w(px(24.0))
                            .h(px(24.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(Radius::XS_SM))
                            .cursor_pointer()
                            .text_color(if can_redo { Text::SECONDARY } else { Text::MUTED })
                            .child("↪"),
                    ),
            )
            // ── Divider ──
            .child(toolbar_sep())
            // ── Tool mode ──
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(Spacing::MD))
                    // Pointer (V)
                    .child(
                        div()
                            .id("btn-pointer")
                            .w(px(24.0))
                            .h(px(24.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(Radius::XS_SM))
                            .cursor_pointer()
                            .bg(if pointer_active { tool_active_bg } else { Background::RAISED })
                            .text_color(if pointer_active { Text::PRIMARY } else { Text::SECONDARY })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.set_tool_mode(ToolMode::Pointer, cx);
                            }))
                            .child("▷"),
                    )
                    // Razor (C)
                    .child(
                        div()
                            .id("btn-razor")
                            .w(px(24.0))
                            .h(px(24.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(Radius::XS_SM))
                            .cursor_pointer()
                            .bg(if razor_active { tool_active_bg } else { Background::RAISED })
                            .text_color(if razor_active { Text::PRIMARY } else { Text::SECONDARY })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.set_tool_mode(ToolMode::Razor, cx);
                            }))
                            .child("✂"),
                    ),
            )
            // ── Divider ──
            .child(toolbar_sep())
            // ── Edit actions ──
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(Spacing::MD))
                    .child(
                        div()
                            .id("btn-split")
                            .w(px(24.0))
                            .h(px(24.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(Radius::XS_SM))
                            .cursor_pointer()
                            .text_color(Text::SECONDARY)
                            .child("⊞"),
                    )
                    .child(
                        div()
                            .id("btn-trim-start")
                            .w(px(24.0))
                            .h(px(24.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(Radius::XS_SM))
                            .cursor_pointer()
                            .text_color(Text::SECONDARY)
                            .child("["),
                    )
                    .child(
                        div()
                            .id("btn-trim-end")
                            .w(px(24.0))
                            .h(px(24.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(Radius::XS_SM))
                            .cursor_pointer()
                            .text_color(Text::SECONDARY)
                            .child("]"),
                    ),
            )
            // ── Divider ──
            .child(toolbar_sep())
            // ── Add text ──
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
                    .child("T"),
            )
            // ── Spacer ──
            .child(div().flex_1())
            // ── Zoom ──
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(Spacing::XS))
                    .child(div().text_color(Text::MUTED).child("⊖"))
                    .child(
                        div()
                            .id("zoom-track")
                            .w(px(80.0))
                            .h(px(4.0))
                            .rounded(px(2.0))
                            .bg(BorderColors::PRIMARY),
                    )
                    .child(div().text_color(Text::MUTED).child("⊕")),
            )
    }
}

fn toolbar_sep() -> impl IntoElement {
    div()
        .w(px(1.0))
        .h(px(Spacing::XL))
        .bg(BorderColors::PRIMARY)
}

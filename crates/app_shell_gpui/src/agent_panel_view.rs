//! Agent panel gpui view — renders MCP status and connection info.
//!
//! Requires the `desktop-app` feature (gpui).

use crate::theme::{Background, BorderColors, FontSize, Radius, Spacing, Status, Text};
use app_contract::agent_panel_model::{AgentPanelModel, McpServerStatus};
use gpui::{
    div, prelude::*, px, App, Context, FocusHandle, Focusable, Hsla, ParentElement, Render, Styled,
    Window,
};

/// Status indicator dot color based on MCP server state.
fn status_dot(status: &McpServerStatus) -> Hsla {
    match status {
        McpServerStatus::Running { .. } => Hsla {
            h: 120.0 / 360.0,
            s: 0.55,
            l: 0.44,
            a: 1.0,
        },
        McpServerStatus::Starting => Hsla {
            h: 42.0 / 360.0,
            s: 0.90,
            l: 0.48,
            a: 1.0,
        },
        McpServerStatus::Stopped => Hsla {
            h: 0.0,
            s: 0.0,
            l: 0.28,
            a: 1.0,
        },
        McpServerStatus::Failed(_) => Status::ERROR,
    }
}

/// gpui Agent Panel view — shows MCP status + tool list.
#[derive(Debug, Clone)]
pub struct AgentPanelView {
    focus_handle: FocusHandle,
    model: AgentPanelModel,
}

impl AgentPanelView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let handle = cx.focus_handle();
        let mut model = AgentPanelModel::default();
        if let Ok(svc) = crate::mcp_service::McpService::global().lock() {
            model.mcp_status = svc.status().clone();
        }
        Self {
            focus_handle: handle,
            model,
        }
    }
}

impl Focusable for AgentPanelView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for AgentPanelView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        if let Ok(svc) = crate::mcp_service::McpService::global().lock() {
            self.model.mcp_status = svc.status().clone();
        }
        let status = self.model.mcp_status.clone();
        let dot_color = status_dot(&status);
        let status_text = self.model.mcp_status_label();
        let tool_count = self.model.available_tools.len();

        // Panel header (28px)
        let header = div()
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .h(px(28.0))
            .px(px(Spacing::MD_LG))
            .gap(px(Spacing::SM))
            .bg(Background::SURFACE)
            .border_b_1()
            .border_color(BorderColors::SUBTLE)
            .child(div().w(px(6.0)).h(px(6.0)).rounded_full().bg(dot_color))
            .child(
                div()
                    .text_color(Text::MUTED)
                    .text_size(px(FontSize::XS))
                    .child("MCP"),
            )
            .child(
                div()
                    .text_color(Text::SECONDARY)
                    .text_size(px(FontSize::XS))
                    .child(status_text.to_string()),
            )
            .child(div().flex_1())
            .when(tool_count > 0, |el| {
                el.child(
                    div()
                        .px(px(Spacing::XS))
                        .py(px(Spacing::XXS))
                        .rounded(px(Radius::XS))
                        .bg(BorderColors::SUBTLE)
                        .text_color(Text::MUTED)
                        .text_size(px(FontSize::XXS))
                        .child(format!("{} tools", tool_count)),
                )
            });

        // Tool list
        let mut tool_list = div()
            .flex()
            .flex_col()
            .px(px(Spacing::SM_MD))
            .py(px(Spacing::SM))
            .gap(px(Spacing::XXS));

        if self.model.available_tools.is_empty() {
            tool_list = tool_list.child(
                div()
                    .text_color(Text::MUTED)
                    .text_size(px(FontSize::SM))
                    .px(px(Spacing::SM))
                    .child("No tools connected"),
            );
        } else {
            for tool_name in &self.model.available_tools {
                let name = tool_name.clone();
                tool_list = tool_list.child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .h(px(24.0))
                        .px(px(Spacing::SM))
                        .rounded(px(Radius::XS))
                        .gap(px(Spacing::XS))
                        .child(div().w(px(4.0)).h(px(4.0)).rounded_full().bg(dot_color))
                        .child(
                            div()
                                .text_color(Text::SECONDARY)
                                .text_size(px(FontSize::SM))
                                .child(name),
                        ),
                );
            }
        }

        div()
            .id("fronda-agent-panel")
            .track_focus(&self.focus_handle.clone())
            .flex()
            .flex_col()
            .size_full()
            .bg(Background::SURFACE)
            .child(header)
            .child(
                div()
                    .id("agent-panel-scroll")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .overflow_y_scroll()
                    .child(tool_list),
            )
    }
}

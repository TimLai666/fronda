//! Agent panel gpui view — renders MCP status and mention picker.
//!
//! Requires the `desktop-app` feature (gpui).

use app_contract::agent_panel_model::{AgentPanelModel, McpServerStatus};
use gpui::{
    div, prelude::*, px, App, Context, FocusHandle, Focusable, Hsla, InteractiveElement,
    ParentElement, Render, Styled, Window,
};

/// Colors for the agent panel.
pub struct AgentPanelColors;
impl AgentPanelColors {
    pub const BACKGROUND: Hsla = Hsla {
        h: 0.0,
        s: 0.0,
        l: 0.07,
        a: 1.0,
    };
    pub const STATUS_RUNNING: Hsla = Hsla {
        h: 120.0 / 360.0,
        s: 0.6,
        l: 0.35,
        a: 1.0,
    };
    pub const STATUS_STOPPED: Hsla = Hsla {
        h: 0.0,
        s: 0.0,
        l: 0.25,
        a: 1.0,
    };
    pub const STATUS_FAILED: Hsla = Hsla {
        h: 0.0,
        s: 0.6,
        l: 0.35,
        a: 1.0,
    };
    pub const TEXT_PRIMARY: Hsla = Hsla {
        h: 0.0,
        s: 0.0,
        l: 1.0,
        a: 1.0,
    };
    pub const TEXT_SECONDARY: Hsla = Hsla {
        h: 0.0,
        s: 0.0,
        l: 1.0,
        a: 0.62,
    };
    pub const SECTION_BG: Hsla = Hsla {
        h: 0.0,
        s: 0.0,
        l: 0.1,
        a: 1.0,
    };
}

/// Status indicator color based on MCP server state.
fn status_color(status: &McpServerStatus) -> Hsla {
    match status {
        McpServerStatus::Running { .. } => AgentPanelColors::STATUS_RUNNING,
        McpServerStatus::Stopped => AgentPanelColors::STATUS_STOPPED,
        McpServerStatus::Starting { .. } => AgentPanelColors::STATUS_STOPPED,
        McpServerStatus::Failed(_) => AgentPanelColors::STATUS_FAILED,
    }
}

/// gpui Agent Panel view component.
#[derive(Debug, Clone)]
pub struct AgentPanelView {
    focus_handle: FocusHandle,
    model: AgentPanelModel,
}

impl AgentPanelView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let handle = cx.focus_handle();
        Self {
            focus_handle: handle,
            model: AgentPanelModel::default(),
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
        let status = self.model.mcp_status.clone();
        let status_color = status_color(&status);
        let status_text = self.model.mcp_status_label();

        div()
            .id("fronda-agent-panel")
            .track_focus(&self.focus_handle.clone())
            .flex()
            .flex_col()
            .size_full()
            .bg(AgentPanelColors::BACKGROUND)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .px(px(8.0))
                    .py(px(8.0))
                    .gap(px(8.0))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap(px(6.0))
                            .items_center()
                            .child(
                                div()
                                    .w(px(8.0))
                                    .h(px(8.0))
                                    .rounded(px(4.0))
                                    .bg(status_color),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .child(status_text)
                                    .text_color(AgentPanelColors::TEXT_SECONDARY),
                            ),
                    ),
            )
    }
}

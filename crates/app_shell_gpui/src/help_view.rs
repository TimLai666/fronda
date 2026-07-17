//! Help gpui view — renders the Help window with Shortcuts and MCP tabs.
//!
//! Covers Help/Shortcuts and Help/MCP panes, matching Swift HelpView after
//! upstream #319 (grid-style shortcut columns; MCP setup as a Server URL
//! block plus a per-agent connect list with copyable commands).

use crate::theme::{Background, BorderColors, BorderWidth, FontSize, IconSize, Opacity, Radius, Spacing, Text};
use app_contract::help_model::{HelpTab, HelpViewModel};
use gpui::{
    div, prelude::*, px, svg, AnyElement, App, ClickEvent, Context, FocusHandle, Focusable, Hsla,
    IntoElement, ParentElement, Render, SharedString, Styled, Window,
};

/// The MCP server alias used in agent-registration commands. Must match the
/// Rust server identity (`mcp_server::McpConfig::default().server_name`).
pub const MCP_SERVER_ALIAS: &str = "fronda";

/// Swift `claudeCodeCommand`.
pub fn claude_code_command(endpoint: &str) -> String {
    format!("claude mcp add --transport http {MCP_SERVER_ALIAS} {endpoint}")
}

/// Swift `codexCommand`.
pub fn codex_command(endpoint: &str) -> String {
    format!("codex mcp add {MCP_SERVER_ALIAS} --url {endpoint}")
}

/// Swift `cursorJSONConfig`.
pub fn cursor_json_config(endpoint: &str) -> String {
    format!(
        "{{\n  \"mcpServers\": {{\n    \"{MCP_SERVER_ALIAS}\": {{\n      \"type\": \"http\",\n      \"url\": \"{endpoint}\"\n    }}\n  }}\n}}"
    )
}

/// Swift `claudeDesktopJSONConfig` (mcp-remote bridge).
pub fn claude_desktop_json_config(endpoint: &str) -> String {
    format!(
        "{{\n  \"mcpServers\": {{\n    \"{MCP_SERVER_ALIAS}\": {{\n      \"command\": \"npx\",\n      \"args\": [\n        \"-y\",\n        \"mcp-remote\",\n        \"{endpoint}\",\n        \"--allow-http\",\n        \"--transport\",\n        \"http-only\"\n      ]\n    }}\n  }}\n}}"
    )
}

/// Agent connect list (Swift `agentList` order): name, badge monogram,
/// description. Logos are text monograms — the upstream assets are PNGs and
/// no suitable inline SVGs exist (documented fallback).
pub const AGENT_SECTIONS: &[(&str, &str, &str)] = &[
    (
        "Claude Desktop",
        "Cl",
        "Add the Fronda MCP server to Claude Desktop.",
    ),
    ("Claude Code", "Cl", "Run this command once in Terminal."),
    ("Codex", "Cx", "Run this command once in Terminal."),
    (
        "Cursor",
        "Cu",
        "Add the Fronda MCP server to Cursor.",
    ),
];

/// gpui Help view component.
pub struct HelpView {
    focus_handle: FocusHandle,
    model: HelpViewModel,
    /// "Manual setup" disclosure state per agent (Claude Desktop, Cursor).
    manual_expanded_claude_desktop: bool,
    manual_expanded_cursor: bool,
    /// Copy-feedback: id of the code block copied within the last 1.4s.
    copied: Option<&'static str>,
}

impl HelpView {
    pub fn new(mcp_port: u16, cx: &mut Context<Self>) -> Self {
        let handle = cx.focus_handle();
        Self {
            focus_handle: handle,
            model: HelpViewModel::new(mcp_port),
            manual_expanded_claude_desktop: false,
            manual_expanded_cursor: false,
            copied: None,
        }
    }

    fn copy_code(&mut self, key: &'static str, value: String, cx: &mut Context<Self>) {
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(value));
        self.copied = Some(key);
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(1400))
                .await;
            let _ = this.update(cx, |view, cx| {
                if view.copied == Some(key) {
                    view.copied = None;
                    cx.notify();
                }
            });
        })
        .detach();
    }
}

impl Focusable for HelpView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

/// Active tab background: white at Opacity::SOFT (10%) — matches hoverHighlight isActive=true.
const TAB_ACTIVE_BG: Hsla = Hsla {
    h: 0.0,
    s: 0.0,
    l: 1.0,
    a: Opacity::SOFT,
};

const HOVER_BG: Hsla = Hsla {
    h: 0.0,
    s: 0.0,
    l: 1.0,
    a: Opacity::FAINT,
};

fn tab_icon_path(tab: HelpTab) -> &'static str {
    match tab {
        HelpTab::Shortcuts => "icons/keyboard.svg",
        HelpTab::Mcp => "icons/network.svg",
    }
}

// ── Shortcut data matching Swift ShortcutsPane.allShortcuts (Rust-real subset) ──
pub const SHORTCUT_GROUPS: &[(&str, &[(&str, &str)])] = &[
    (
        "Playback",
        &[
            ("Space", "Play / Pause"),
            ("←", "Step Backward"),
            ("→", "Step Forward"),
            ("Shift + ←", "Skip Backward"),
            ("Shift + →", "Skip Forward"),
        ],
    ),
    ("Tools", &[("V", "Selection Tool"), ("C", "Razor Tool")]),
    (
        "Editing",
        &[
            ("Cmd + K", "Split at Playhead"),
            ("[ or Q", "Trim Start to Playhead"),
            ("] or W", "Trim End to Playhead"),
            ("Backspace", "Delete"),
            ("Shift + Backspace", "Ripple Delete"),
            ("Opt + Drag", "Duplicate Clip"),
        ],
    ),
    (
        "Timeline",
        &[
            ("Shift + Drag Ruler", "Select Range"),
            ("Drag Range Edge", "Adjust Range"),
            ("I", "Mark Range Start"),
            ("O", "Mark Range End"),
            ("Opt + Scroll", "Zoom to Cursor"),
            ("Pinch", "Zoom to Cursor"),
            ("Cmd + Scroll", "Scroll Horizontally"),
        ],
    ),
    (
        "File",
        &[
            ("Cmd + N", "New"),
            ("Cmd + O", "Open"),
            ("Cmd + S", "Save"),
            ("Cmd + Shift + S", "Save As"),
            ("Cmd + I", "Import Media"),
            ("Cmd + Shift + I", "Import Timeline (XML/FCPXML)"),
            ("Cmd + E", "Export"),
        ],
    ),
    (
        "Edit",
        &[
            ("Cmd + Z", "Undo"),
            ("Cmd + Shift + Z", "Redo"),
            ("Cmd + X", "Cut"),
            ("Cmd + C", "Copy"),
            ("Cmd + V", "Paste"),
            ("Cmd + A", "Select All"),
        ],
    ),
    (
        "View",
        &[
            ("Cmd + F", "Full Screen"),
            ("`", "Maximize Focused Panel"),
            ("Cmd + Scroll", "Zoom Preview to Cursor"),
            ("Esc", "Deselect & Reset Tool"),
        ],
    ),
];

/// Groups per column (Swift: left = first 4, right = rest).
pub const SHORTCUT_LEFT_COLUMN_GROUPS: usize = 4;

/// Key column width — gpui has no Grid; a fixed column approximates the
/// Swift Grid's intrinsic key column.
const SHORTCUT_KEY_COLUMN_WIDTH: f32 = 118.0;

/// Shortcut group column (Swift #319 shortcutColumn): smMd primary group
/// titles, mono-xs keys, sm secondary descriptions.
fn shortcut_column(groups: &[(&str, &[(&str, &str)])]) -> impl IntoElement {
    let mut col = div().flex().flex_col().flex_1().gap(px(Spacing::MD));

    for (title, rows) in groups {
        let mut group = div()
            .flex()
            .flex_col()
            .gap(px(Spacing::SM))
            .child(
                div()
                    .text_color(Text::PRIMARY)
                    .text_size(px(FontSize::SM_MD))
                    .child(title.to_string()),
            );

        for (shortcut, desc) in *rows {
            group = group.child(
                div()
                    .flex()
                    .flex_row()
                    .items_start()
                    .gap(px(Spacing::MD))
                    .child(
                        div()
                            .w(px(SHORTCUT_KEY_COLUMN_WIDTH))
                            .flex_none()
                            .text_color(Text::PRIMARY)
                            .text_size(px(FontSize::XS))
                            .child(shortcut.to_string()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_color(Text::SECONDARY)
                            .text_size(px(FontSize::SM))
                            .child(desc.to_string()),
                    ),
            );
        }
        col = col.child(group);
    }
    col
}

/// Shortcuts pane: 2 columns capped at the settings content width.
fn shortcuts_pane() -> impl IntoElement {
    let left = &SHORTCUT_GROUPS[..SHORTCUT_LEFT_COLUMN_GROUPS];
    let right = &SHORTCUT_GROUPS[SHORTCUT_LEFT_COLUMN_GROUPS..];

    div()
        .id("shortcuts-pane-scroll")
        .flex_1()
        .overflow_y_scroll()
        .child(
            div()
                .flex()
                .flex_row()
                .items_start()
                .gap(px(Spacing::XL_XXL))
                .w_full()
                .max_w(px(crate::settings_view::SettingsMetrics::CONTENT_MAX_WIDTH))
                .px(px(Spacing::XL_XXL))
                .pb(px(Spacing::XXL))
                .child(shortcut_column(left))
                .child(shortcut_column(right)),
        )
}

// ── MCP pane (Swift MCPInstructionsPane, #319) ────────────────────────────────

impl HelpView {
    /// Code block row: content + copy button (Swift CodeBlockView).
    fn code_block(
        &self,
        key: &'static str,
        content: String,
        primary_text: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let copied = self.copied == Some(key);
        let copy_value = content.clone();
        div()
            .flex()
            .flex_row()
            .items_start()
            .gap(px(Spacing::SM_MD))
            .w_full()
            .px(px(Spacing::MD_LG))
            .py(px(Spacing::MD))
            .rounded(px(Radius::SM))
            .border(px(BorderWidth::THIN))
            .border_color(BorderColors::SUBTLE)
            .bg(Background::RAISED)
            .child(
                div()
                    .flex_1()
                    .text_size(px(if primary_text {
                        FontSize::SM
                    } else {
                        FontSize::XS
                    }))
                    .text_color(if primary_text {
                        Text::PRIMARY
                    } else {
                        Text::SECONDARY
                    })
                    .child(content),
            )
            .child(
                div()
                    .id(SharedString::from(format!("copy-{key}")))
                    .w(px(IconSize::LG))
                    .h(px(IconSize::LG))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(Radius::SM))
                    .cursor_pointer()
                    .hover(|s| s.bg(HOVER_BG))
                    .text_size(px(FontSize::SM))
                    .text_color(if copied { Text::PRIMARY } else { Text::SECONDARY })
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.copy_code(key, copy_value.clone(), cx);
                    }))
                    .child(if copied { "✓" } else { "⧉" }),
            )
            .into_any_element()
    }

    /// Collapsible "Manual setup" disclosure (Swift ManualFallback).
    fn manual_fallback(
        &self,
        key: &'static str,
        expanded: bool,
        intro: &str,
        code: String,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let toggle_claude = key == "claude-desktop";
        let mut el = div()
            .flex()
            .flex_col()
            .gap(px(Spacing::SM_MD))
            .child(
                div()
                    .id(SharedString::from(format!("manual-{key}")))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(Spacing::SM))
                    .cursor_pointer()
                    .text_color(Text::SECONDARY)
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if toggle_claude {
                            this.manual_expanded_claude_desktop =
                                !this.manual_expanded_claude_desktop;
                        } else {
                            this.manual_expanded_cursor = !this.manual_expanded_cursor;
                        }
                        cx.notify();
                    }))
                    .child(
                        div()
                            .text_size(px(FontSize::XXS))
                            .child(if expanded { "▾" } else { "▸" }),
                    )
                    .child(div().text_size(px(FontSize::SM)).child("Manual setup")),
            );
        if expanded {
            el = el.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(Spacing::SM))
                    .child(
                        div()
                            .text_color(Text::TERTIARY)
                            .text_size(px(FontSize::SM))
                            .child(intro.to_string()),
                    )
                    .child(self.code_block(key, code, false, cx)),
            );
        }
        el.into_any_element()
    }

    /// Agent identity row: monogram badge + name + description (Swift
    /// agentIdentity; logos fall back to text — upstream assets are PNGs).
    fn agent_identity(name: &str, monogram: &str, description: &str) -> AnyElement {
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(Spacing::MD))
            .child(
                div()
                    .w(px(IconSize::LG_XL))
                    .h(px(IconSize::LG_XL))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(Radius::XS))
                    .border(px(BorderWidth::THIN))
                    .border_color(BorderColors::SUBTLE)
                    .bg(Background::RAISED)
                    .text_color(Text::SECONDARY)
                    .text_size(px(FontSize::SM_MD))
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .child(monogram.to_string()),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(Spacing::XS))
                    .child(
                        div()
                            .text_color(Text::PRIMARY)
                            .text_size(px(FontSize::MD))
                            .child(name.to_string()),
                    )
                    .child(
                        div()
                            .text_color(Text::TERTIARY)
                            .text_size(px(FontSize::SM))
                            .child(description.to_string()),
                    ),
            )
            .into_any_element()
    }

    fn agent_section(identity: AnyElement, details: AnyElement) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .gap(px(Spacing::MD))
            .py(px(Spacing::MD_LG))
            .child(identity)
            .child(details)
            .into_any_element()
    }

    fn group_title(text: &str) -> AnyElement {
        div()
            .text_color(Text::PRIMARY)
            .text_size(px(FontSize::SM_MD))
            .child(text.to_string())
            .into_any_element()
    }

    fn agent_divider() -> AnyElement {
        div()
            .w_full()
            .h(px(BorderWidth::THIN))
            .bg(BorderColors::SUBTLE)
            .into_any_element()
    }

    /// MCP instructions pane (Swift #319: Server URL + Connect an agent).
    fn mcp_pane(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let endpoint = self.model.mcp_endpoint();
        let expanded_claude = self.manual_expanded_claude_desktop;
        let expanded_cursor = self.manual_expanded_cursor;

        let claude_desktop = Self::agent_section(
            Self::agent_identity(
                AGENT_SECTIONS[0].0,
                AGENT_SECTIONS[0].1,
                AGENT_SECTIONS[0].2,
            ),
            self.manual_fallback(
                "claude-desktop",
                expanded_claude,
                "In Claude Desktop, open Settings › Developer › Edit Config, then add this configuration to mcpServers.",
                claude_desktop_json_config(&endpoint),
                cx,
            ),
        );
        let claude_code = Self::agent_section(
            Self::agent_identity(
                AGENT_SECTIONS[1].0,
                AGENT_SECTIONS[1].1,
                AGENT_SECTIONS[1].2,
            ),
            self.code_block("claude-code", claude_code_command(&endpoint), false, cx),
        );
        let codex = Self::agent_section(
            Self::agent_identity(
                AGENT_SECTIONS[2].0,
                AGENT_SECTIONS[2].1,
                AGENT_SECTIONS[2].2,
            ),
            self.code_block("codex", codex_command(&endpoint), false, cx),
        );
        let cursor = Self::agent_section(
            Self::agent_identity(
                AGENT_SECTIONS[3].0,
                AGENT_SECTIONS[3].1,
                AGENT_SECTIONS[3].2,
            ),
            self.manual_fallback(
                "cursor",
                expanded_cursor,
                "Add this configuration to ~/.cursor/mcp.json.",
                cursor_json_config(&endpoint),
                cx,
            ),
        );

        div()
            .id("mcp-pane-scroll")
            .flex_1()
            .overflow_y_scroll()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(Spacing::XXL))
                    .w_full()
                    .max_w(px(crate::settings_view::SettingsMetrics::CONTENT_MAX_WIDTH))
                    .px(px(Spacing::XL_XXL))
                    .pb(px(Spacing::XXL))
                    .child(
                        div()
                            .text_color(Text::SECONDARY)
                            .text_size(px(FontSize::SM_MD))
                            .child(
                                "Connect an external agent to inspect and edit the open Fronda project.",
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(Spacing::SM_MD))
                            .child(Self::group_title("Server URL"))
                            .child(self.code_block("endpoint", endpoint.clone(), true, cx)),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(Spacing::SM_MD))
                            .child(Self::group_title("Connect an agent"))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .child(claude_desktop)
                                    .child(Self::agent_divider())
                                    .child(claude_code)
                                    .child(Self::agent_divider())
                                    .child(codex)
                                    .child(Self::agent_divider())
                                    .child(cursor),
                            ),
                    ),
            )
            .into_any_element()
    }
}

impl Render for HelpView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active = self.model.active_tab;

        // Sidebar: 220px wide, matching Swift frame(width: 220) — no explicit border_r
        let mut sidebar = div()
            .flex()
            .flex_col()
            .w(px(crate::settings_view::SettingsMetrics::SIDEBAR_WIDTH))
            .h_full()
            .bg(Background::SURFACE)
            .px(px(Spacing::SM_MD))
            .py(px(Spacing::MD))
            .gap(px(Spacing::XXS));

        for tab in HelpTab::ALL {
            let is_active = self.model.active_tab == *tab;
            let tab_value = *tab;
            let icon_path = tab_icon_path(*tab);
            let icon_color = if is_active {
                Text::PRIMARY
            } else {
                Text::SECONDARY
            };
            sidebar = sidebar.child(
                div()
                    .id(SharedString::from(format!("help-tab-{}", tab.label())))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(Spacing::MD))
                    .w_full()
                    .px(px(Spacing::MD))
                    .py(px(Spacing::SM))
                    .rounded(px(Radius::SM))
                    .cursor_pointer()
                    .bg(if is_active {
                        TAB_ACTIVE_BG
                    } else {
                        Background::SURFACE
                    })
                    .when(!is_active, |el| el.hover(|s| s.bg(HOVER_BG)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.model.switch_to(tab_value);
                        cx.notify();
                    }))
                    // Icon: 12px in 16px frame
                    .child(
                        div()
                            .w(px(IconSize::XXS + Spacing::XS))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                svg()
                                    .path(icon_path)
                                    .w(px(IconSize::XXS))
                                    .h(px(IconSize::XXS))
                                    .text_color(icon_color),
                            ),
                    )
                    // Label: md size, medium weight when active
                    .child(
                        div()
                            .text_size(px(FontSize::MD))
                            .text_color(if is_active {
                                Text::PRIMARY
                            } else {
                                Text::SECONDARY
                            })
                            .font_weight(if is_active {
                                gpui::FontWeight::MEDIUM
                            } else {
                                gpui::FontWeight::NORMAL
                            })
                            .child(tab.label()),
                    )
                    .child(div().flex_1()),
            );
        }

        // Detail area: title header + pane content (matches Swift `detail` var)
        let tab_title = active.label();
        let pane_content = match active {
            HelpTab::Shortcuts => shortcuts_pane().into_any_element(),
            HelpTab::Mcp => self.mcp_pane(cx),
        };

        let detail = div()
            .flex()
            .flex_col()
            .flex_1()
            .h_full()
            .overflow_hidden()
            .bg(Background::BASE)
            // Tab title header
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .px(px(Spacing::XL_XXL))
                    .pt(px(Spacing::XXL))
                    .pb(px(Spacing::LG_XL))
                    .child(
                        div()
                            .text_size(px(FontSize::TITLE_2))
                            .font_weight(gpui::FontWeight::LIGHT)
                            .text_color(Text::PRIMARY)
                            .child(tab_title),
                    )
                    .child(div().flex_1()),
            )
            .child(pane_content);

        div()
            .id("fronda-help")
            .track_focus(&self.focus_handle.clone())
            .flex()
            .flex_row()
            .size_full()
            .bg(Background::BASE)
            .child(sidebar)
            .child(detail)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcut_groups_mirror_swift_titles_and_split() {
        let titles: Vec<&str> = SHORTCUT_GROUPS.iter().map(|(t, _)| *t).collect();
        assert_eq!(
            titles,
            ["Playback", "Tools", "Editing", "Timeline", "File", "Edit", "View"]
        );
        // Swift: left column = first 4 groups, right = the remaining 3.
        assert_eq!(SHORTCUT_LEFT_COLUMN_GROUPS, 4);
        assert_eq!(SHORTCUT_GROUPS.len() - SHORTCUT_LEFT_COLUMN_GROUPS, 3);
    }

    #[test]
    fn mcp_alias_matches_rust_server_identity() {
        assert_eq!(
            MCP_SERVER_ALIAS,
            mcp_server::McpConfig::default().server_name,
            "help commands must register the server under its real name"
        );
    }

    #[test]
    fn agent_commands_embed_alias_and_endpoint() {
        let endpoint = "http://127.0.0.1:19789/mcp";
        let claude = claude_code_command(endpoint);
        assert_eq!(
            claude,
            "claude mcp add --transport http fronda http://127.0.0.1:19789/mcp"
        );
        let codex = codex_command(endpoint);
        assert_eq!(codex, "codex mcp add fronda --url http://127.0.0.1:19789/mcp");
    }

    #[test]
    fn agent_json_configs_are_valid_json() {
        let endpoint = "http://127.0.0.1:19789/mcp";
        let cursor: serde_json::Value =
            serde_json::from_str(&cursor_json_config(endpoint)).expect("cursor config parses");
        assert_eq!(
            cursor.pointer("/mcpServers/fronda/url").and_then(|v| v.as_str()),
            Some(endpoint)
        );
        assert_eq!(
            cursor.pointer("/mcpServers/fronda/type").and_then(|v| v.as_str()),
            Some("http")
        );

        let desktop: serde_json::Value = serde_json::from_str(&claude_desktop_json_config(endpoint))
            .expect("claude desktop config parses");
        assert_eq!(
            desktop
                .pointer("/mcpServers/fronda/command")
                .and_then(|v| v.as_str()),
            Some("npx")
        );
        let args = desktop
            .pointer("/mcpServers/fronda/args")
            .and_then(|v| v.as_array())
            .expect("args array");
        assert!(args.iter().any(|a| a.as_str() == Some("mcp-remote")));
        assert!(args.iter().any(|a| a.as_str() == Some(endpoint)));
    }

    #[test]
    fn agent_sections_mirror_swift_order() {
        let names: Vec<&str> = AGENT_SECTIONS.iter().map(|(n, _, _)| *n).collect();
        assert_eq!(names, ["Claude Desktop", "Claude Code", "Codex", "Cursor"]);
    }
}

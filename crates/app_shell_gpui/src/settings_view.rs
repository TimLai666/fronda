//! Settings gpui view — sidebar + full pane content for all 5 tabs.
//!
//! Matches Swift SettingsView + all pane files (AccountPane, NotificationsPane,
//! PrivacyPane, ModelsPane, AgentPane, StoragePane).

use app_contract::settings_storage::SettingsTab;
use crate::theme::{Accent, Background, BorderColors, FontSize, Opacity, Radius, Spacing, Text};
use gpui::{
    div, prelude::*, px, App, ClickEvent, Context, FocusHandle, Focusable, InteractiveElement,
    IntoElement, ParentElement, Render, Styled, Window,
};

/// gpui Settings view — carries mutable tab + toggle state.
pub struct SettingsView {
    focus_handle: FocusHandle,
    active_tab: SettingsTab,
    backend_configured: bool,
    // General
    notifications_on: bool,
    privacy_on: bool,
    // Models — toggled state per model (index-based for simplicity)
    image_models_on: [bool; 3],
    video_models_on: [bool; 3],
    audio_models_on: [bool; 2],
    // Agent
    mcp_running: bool,
    mcp_enabled: bool,
    // Storage
    search_enabled: bool,
}

impl SettingsView {
    pub fn new(backend_configured: bool, cx: &mut Context<Self>) -> Self {
        let initial_tab = if backend_configured {
            SettingsTab::Account
        } else {
            SettingsTab::General
        };
        Self {
            focus_handle: cx.focus_handle(),
            active_tab: initial_tab,
            backend_configured,
            notifications_on: true,
            privacy_on: true,
            image_models_on: [true, false, true],
            video_models_on: [true, true, false],
            audio_models_on: [true, false],
            mcp_running: true,
            mcp_enabled: true,
            search_enabled: true,
        }
    }
}

impl Focusable for SettingsView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

// ── Shared helpers ────────────────────────────────────────────────────────────

fn section_title(text: &str) -> impl IntoElement {
    div()
        .text_color(Text::PRIMARY)
        .text_size(px(FontSize::MD))
        .font_weight(gpui::FontWeight::MEDIUM)
        .child(text.to_string())
}

fn body_text(text: &str) -> impl IntoElement {
    div()
        .text_color(Text::TERTIARY)
        .text_size(px(FontSize::SM))
        .child(text.to_string())
}

fn link_text(text: &str) -> impl IntoElement {
    div()
        .text_color(Accent::PRIMARY)
        .text_size(px(FontSize::SM))
        .cursor_pointer()
        .child(text.to_string())
}

fn divider() -> impl IntoElement {
    div().w_full().h(px(1.0)).bg(BorderColors::SUBTLE)
}

/// Toggle pill — 28×16 pill, knob slides right when on.
fn toggle_pill(is_on: bool) -> impl IntoElement {
    let bg = if is_on { Accent::PRIMARY } else {
        gpui::Hsla { h: 0.0, s: 0.0, l: 1.0, a: Opacity::MUTED }
    };
    div()
        .w(px(28.0))
        .h(px(16.0))
        .rounded_full()
        .bg(bg)
        .flex()
        .items_center()
        .when(is_on, |el| el.justify_end())
        .px(px(2.0))
        .child(
            div()
                .w(px(12.0))
                .h(px(12.0))
                .rounded_full()
                .bg(Background::BASE),
        )
}

/// Labeled toggle row — label left, pill right.
fn toggle_row_label(label: &str, subtitle: &str, is_on: bool) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .items_start()
        .gap(px(Spacing::MD))
        .w_full()
        .py(px(Spacing::SM_MD))
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .gap(px(Spacing::XXS))
                .child(
                    div()
                        .text_color(Text::PRIMARY)
                        .text_size(px(FontSize::SM))
                        .child(label.to_string()),
                )
                .when(!subtitle.is_empty(), |el| {
                    el.child(
                        div()
                            .text_color(Text::TERTIARY)
                            .text_size(px(FontSize::XS))
                            .child(subtitle.to_string()),
                    )
                }),
        )
        .child(toggle_pill(is_on))
}

/// Rounded card container for grouped rows.
fn card(children: Vec<gpui::AnyElement>) -> impl IntoElement {
    let mut el = div()
        .rounded(px(Radius::SM))
        .border_1()
        .border_color(BorderColors::SUBTLE)
        .overflow_hidden()
        .bg(Background::RAISED);
    for (i, child) in children.into_iter().enumerate() {
        if i > 0 {
            el = el.child(divider());
        }
        el = el.child(child);
    }
    el
}

// ── Account pane ──────────────────────────────────────────────────────────────

fn pane_account() -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(Spacing::LG))
        .child(body_text("Sign in to subscribe and use AI generation."))
        .child(
            div()
                .id("btn-sign-in")
                .self_start()
                .px(px(Spacing::MD_LG))
                .py(px(Spacing::SM))
                .rounded_full()
                .border_1()
                .border_color(BorderColors::PRIMARY)
                .cursor_pointer()
                .text_color(Accent::PRIMARY)
                .text_size(px(FontSize::SM))
                .child("Sign in with Google"),
        )
}

// ── General pane ─────────────────────────────────────────────────────────────

fn pane_general(notifications_on: bool, privacy_on: bool) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(Spacing::XL))
        // Notifications section
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(Spacing::SM))
                .child(
                    div()
                        .text_color(Text::MUTED)
                        .text_size(px(FontSize::XS))
                        .child("NOTIFICATIONS"),
                )
                .child(
                    div()
                        .rounded(px(Radius::SM))
                        .border_1()
                        .border_color(BorderColors::SUBTLE)
                        .bg(Background::RAISED)
                        .overflow_hidden()
                        .px(px(Spacing::MD_LG))
                        .child(toggle_row_label(
                            "Show notifications",
                            "Get a system notification when a generation finishes.",
                            notifications_on,
                        )),
                ),
        )
        // Privacy section
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(Spacing::SM))
                .child(
                    div()
                        .text_color(Text::MUTED)
                        .text_size(px(FontSize::XS))
                        .child("PRIVACY"),
                )
                .child(
                    div()
                        .rounded(px(Radius::SM))
                        .border_1()
                        .border_color(BorderColors::SUBTLE)
                        .bg(Background::RAISED)
                        .overflow_hidden()
                        .px(px(Spacing::MD_LG))
                        .child(toggle_row_label(
                            "Send anonymous crash and error reports",
                            "Helps us find and fix issues. Your media and project content are never collected.",
                            privacy_on,
                        )),
                ),
        )
}

// ── Models pane ───────────────────────────────────────────────────────────────

fn model_row(name: &str, is_on: bool) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(Spacing::MD))
        .w_full()
        .px(px(Spacing::MD_LG))
        .py(px(Spacing::SM_MD))
        .child(
            div()
                .flex_1()
                .text_color(Text::PRIMARY)
                .text_size(px(FontSize::SM))
                .child(name.to_string()),
        )
        .child(toggle_pill(is_on))
}

fn model_section(section_label: &str, models: &[(&str, bool)]) -> impl IntoElement {
    let mut rows: Vec<gpui::AnyElement> = vec![];
    for &(name, on) in models {
        rows.push(model_row(name, on).into_any_element());
    }

    div()
        .flex()
        .flex_col()
        .gap(px(Spacing::SM))
        .child(
            div()
                .text_color(Text::MUTED)
                .text_size(px(FontSize::XS))
                .child(section_label.to_string().to_uppercase()),
        )
        .child(card(rows))
}

fn pane_models(image_on: &[bool; 3], video_on: &[bool; 3], audio_on: &[bool; 2]) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(Spacing::XL))
        // Search field
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(Spacing::XS))
                .px(px(Spacing::SM_MD))
                .h(px(30.0))
                .rounded(px(Radius::SM))
                .border_1()
                .border_color(BorderColors::SUBTLE)
                .bg(Background::RAISED)
                .text_color(Text::MUTED)
                .text_size(px(FontSize::SM))
                .child("⌕")
                .child("Search models"),
        )
        .child(model_section("Image", &[
            ("Flux 1.1 Pro", image_on[0]),
            ("Flux 1.1 Pro Ultra", image_on[1]),
            ("Stable Diffusion XL", image_on[2]),
        ]))
        .child(model_section("Video", &[
            ("Kling 1.6 Pro", video_on[0]),
            ("Minimax Video 01", video_on[1]),
            ("Wan 2.1", video_on[2]),
        ]))
        .child(model_section("Audio", &[
            ("Stable Audio 2.0", audio_on[0]),
            ("AudioGen", audio_on[1]),
        ]))
}

// ── Agent pane ────────────────────────────────────────────────────────────────

fn pane_agent(mcp_running: bool, mcp_enabled: bool) -> impl IntoElement {
    let dot_color = if mcp_running {
        gpui::Hsla { h: 0.33, s: 0.70, l: 0.45, a: 1.0 } // green
    } else {
        gpui::Hsla { h: 0.0, s: 0.0, l: 0.5, a: 1.0 }
    };

    div()
        .flex()
        .flex_col()
        .gap(px(Spacing::XL))
        // API Key section
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(Spacing::SM_MD))
                .child(section_title("Anthropic API Key"))
                .child(body_text("Use your own API key for AI chat. Stored in your keychain."))
                .child(link_text("Get API key →"))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .px(px(Spacing::SM_MD))
                        .h(px(32.0))
                        .rounded(px(Radius::SM))
                        .border_1()
                        .border_color(BorderColors::SUBTLE)
                        .bg(Background::RAISED)
                        .text_color(Text::MUTED)
                        .text_size(px(FontSize::SM))
                        .child("sk-ant-···"),
                ),
        )
        .child(divider())
        // MCP Server section
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(Spacing::SM_MD))
                .child(section_title("MCP Server"))
                .child(body_text("Lets external clients edit your timeline."))
                .child(link_text("Setup instructions →"))
                .child(
                    div()
                        .rounded(px(Radius::SM))
                        .border_1()
                        .border_color(BorderColors::SUBTLE)
                        .bg(Background::RAISED)
                        .overflow_hidden()
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(Spacing::SM))
                                .px(px(Spacing::MD_LG))
                                .py(px(Spacing::SM_MD))
                                .child(
                                    div()
                                        .text_size(px(FontSize::XS))
                                        .text_color(dot_color)
                                        .child("●"),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .text_color(Text::PRIMARY)
                                        .text_size(px(FontSize::SM))
                                        .child(if mcp_running {
                                            "Running on 127.0.0.1:49152"
                                        } else {
                                            "Stopped"
                                        }),
                                )
                                .child(toggle_pill(mcp_enabled)),
                        ),
                ),
        )
}

// ── Storage pane ──────────────────────────────────────────────────────────────

fn pane_storage(search_enabled: bool) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(Spacing::XL))
        // Cache section
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(Spacing::SM_MD))
                .child(section_title("Cache"))
                .child(body_text(
                    "Saved playback previews, waveforms, and filmstrip thumbnails. Safe to clear.",
                ))
                .child(
                    div()
                        .rounded(px(Radius::SM))
                        .border_1()
                        .border_color(BorderColors::SUBTLE)
                        .bg(Background::RAISED)
                        .overflow_hidden()
                        // Path row
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .px(px(Spacing::MD_LG))
                                .py(px(Spacing::SM_MD))
                                .child(
                                    div()
                                        .flex_1()
                                        .text_color(Text::TERTIARY)
                                        .text_size(px(FontSize::XS))
                                        .child("~/Library/Caches/palmier"),
                                )
                                .child(
                                    div()
                                        .text_color(Text::SECONDARY)
                                        .text_size(px(FontSize::XS))
                                        .child("0 B"),
                                ),
                        )
                        .child(divider())
                        // Clear button row
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .px(px(Spacing::MD_LG))
                                .py(px(Spacing::SM_MD))
                                .child(
                                    div()
                                        .id("btn-clear-cache")
                                        .px(px(Spacing::SM_MD))
                                        .py(px(Spacing::XXS))
                                        .rounded(px(Radius::XS_SM))
                                        .border_1()
                                        .border_color(BorderColors::SUBTLE)
                                        .cursor_pointer()
                                        .text_color(Text::SECONDARY)
                                        .text_size(px(FontSize::XS))
                                        .child("Clear cache"),
                                ),
                        ),
                ),
        )
        .child(divider())
        // Media search section
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(Spacing::SM_MD))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .child(
                            div()
                                .flex_1()
                                .text_color(Text::PRIMARY)
                                .text_size(px(FontSize::MD))
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .child("Media search"),
                        )
                        .child(toggle_pill(search_enabled)),
                )
                .child(body_text(
                    "Indexes media on import so you can search it.",
                ))
                .child(
                    div()
                        .rounded(px(Radius::SM))
                        .border_1()
                        .border_color(BorderColors::SUBTLE)
                        .bg(Background::RAISED)
                        .overflow_hidden()
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .px(px(Spacing::MD_LG))
                                .py(px(Spacing::SM_MD))
                                .child(
                                    div()
                                        .flex_1()
                                        .text_color(Text::SECONDARY)
                                        .text_size(px(FontSize::XS))
                                        .child("Index 0 B"),
                                )
                                .child(
                                    div()
                                        .id("btn-clear-index")
                                        .px(px(Spacing::SM_MD))
                                        .py(px(Spacing::XXS))
                                        .rounded(px(Radius::XS_SM))
                                        .border_1()
                                        .border_color(BorderColors::SUBTLE)
                                        .cursor_pointer()
                                        .text_color(Text::SECONDARY)
                                        .text_size(px(FontSize::XS))
                                        .child("Clear index"),
                                ),
                        ),
                ),
        )
}

// ── Sidebar tab icon ──────────────────────────────────────────────────────────

fn tab_icon(tab: SettingsTab) -> &'static str {
    match tab {
        SettingsTab::Account => "◯",
        SettingsTab::General => "⚙",
        SettingsTab::Models => "⊞",
        SettingsTab::Agent => "✈",
        SettingsTab::Storage => "⊟",
    }
}

// ── Render ────────────────────────────────────────────────────────────────────

impl Render for SettingsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active_tab = self.active_tab;
        let backend = self.backend_configured;

        // Sidebar (220px, matching Swift)
        let mut sidebar = div()
            .flex()
            .flex_col()
            .w(px(220.0))
            .h_full()
            .bg(Background::SURFACE)
            .border_r_1()
            .border_color(BorderColors::PRIMARY)
            .py(px(Spacing::MD));

        for &tab in SettingsTab::ALL {
            if !backend && tab == SettingsTab::Account {
                continue;
            }
            let is_active = active_tab == tab;
            sidebar = sidebar.child(
                div()
                    .id(gpui::SharedString::from(format!("stab-{}", tab.label())))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(Spacing::SM))
                    .w_full()
                    .h(px(32.0))
                    .px(px(Spacing::MD_LG))
                    .mx(px(Spacing::SM))
                    .rounded(px(Radius::SM))
                    .cursor_pointer()
                    .bg(if is_active {
                        gpui::Hsla { h: 0.0, s: 0.0, l: 1.0, a: Opacity::FAINT }
                    } else {
                        Background::SURFACE
                    })
                    .on_click(cx.listener(move |this, _: &ClickEvent, _: &mut Window, cx| {
                        this.active_tab = tab;
                        cx.notify();
                    }))
                    .child(
                        div()
                            .text_color(if is_active { Text::PRIMARY } else { Text::TERTIARY })
                            .text_size(px(FontSize::SM_MD))
                            .child(tab_icon(tab)),
                    )
                    .child(
                        div()
                            .text_size(px(FontSize::SM))
                            .text_color(if is_active { Text::PRIMARY } else { Text::SECONDARY })
                            .child(tab.label()),
                    ),
            );
        }

        // Content pane
        let notifications_on = self.notifications_on;
        let privacy_on = self.privacy_on;
        let image_on = self.image_models_on;
        let video_on = self.video_models_on;
        let audio_on = self.audio_models_on;
        let mcp_running = self.mcp_running;
        let mcp_enabled = self.mcp_enabled;
        let search_enabled = self.search_enabled;

        let pane_content: gpui::AnyElement = match active_tab {
            SettingsTab::Account => pane_account().into_any_element(),
            SettingsTab::General => pane_general(notifications_on, privacy_on).into_any_element(),
            SettingsTab::Models => pane_models(&image_on, &video_on, &audio_on).into_any_element(),
            SettingsTab::Agent => pane_agent(mcp_running, mcp_enabled).into_any_element(),
            SettingsTab::Storage => pane_storage(search_enabled).into_any_element(),
        };

        let content = div()
            .id("settings-content-scroll")
            .flex()
            .flex_col()
            .flex_1()
            .h_full()
            .overflow_y_scroll()
            .px(px(Spacing::XL_XXL))
            .pt(px(Spacing::XXL))
            .pb(px(Spacing::XL_XXL))
            .gap(px(Spacing::XL))
            .child(
                div()
                    .text_color(Text::PRIMARY)
                    .text_size(px(FontSize::XL))
                    .child(active_tab.label()),
            )
            .child(pane_content);

        div()
            .id("fronda-settings")
            .track_focus(&self.focus_handle.clone())
            .flex()
            .flex_row()
            .size_full()
            .bg(Background::BASE)
            .child(sidebar)
            .child(content)
    }
}

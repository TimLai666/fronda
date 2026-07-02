//! Settings gpui view — sidebar + full pane content for all 5 tabs.
//!
//! Matches Swift SettingsView + all pane files (AccountPane, NotificationsPane,
//! PrivacyPane, ModelsPane, AgentPane, StoragePane).

use crate::theme::{Accent, Background, BorderColors, FontSize, Opacity, Radius, Spacing, Text};
use app_contract::agent_panel_model::McpServerStatus;
use app_contract::settings_storage::SettingsTab;
use gpui::{
    div, prelude::*, px, App, ClickEvent, Context, FocusHandle, Focusable, InteractiveElement,
    IntoElement, ParentElement, Render, Styled, Window,
};

/// gpui Settings view — carries mutable tab + toggle state.
pub struct SettingsView {
    focus_handle: FocusHandle,
    active_tab: SettingsTab,
    backend_configured: bool,
    // Account
    /// True when a signed-in user exists (controls sidebar IdentityStrip + account pane).
    pub is_signed_in: bool,
    /// True when the account is a paid subscriber (controls account pane branch).
    pub is_paid: bool,
    /// Account loading state — shows "Loading…" in account pane.
    pub is_loading: bool,
    /// User display initial for IdentityStrip avatar (None = not signed in).
    pub account_initial: Option<char>,
    /// User display name for IdentityStrip.
    pub display_name: String,
    /// User email or plan label for IdentityStrip secondary line.
    pub display_secondary: Option<String>,
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
    /// True when the user has stored an Anthropic API key (AgentPane SecureField state).
    has_stored_api_key: bool,
    // Storage
    search_enabled: bool,
    /// Size in bytes of locally cached ML model (None = no model downloaded).
    pub model_bytes: Option<u64>,
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
            is_signed_in: false,
            is_paid: false,
            is_loading: false,
            account_initial: None,
            display_name: String::new(),
            display_secondary: None,
            notifications_on: true,
            privacy_on: true,
            image_models_on: [true, false, true],
            video_models_on: [true, true, false],
            audio_models_on: [true, false],
            mcp_running: true,
            mcp_enabled: true,
            has_stored_api_key: false,
            search_enabled: true,
            model_bytes: None,
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
    let bg = if is_on {
        Accent::PRIMARY
    } else {
        gpui::Hsla {
            h: 0.0,
            s: 0.0,
            l: 1.0,
            a: Opacity::MUTED,
        }
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

/// Mini card with caption + content — matches Swift AccountPane.card.
fn account_card(caption: &str, children: Vec<gpui::AnyElement>) -> impl IntoElement {
    let mut card = div()
        .flex()
        .flex_col()
        .flex_1()
        .gap(px(Spacing::SM))
        .p(px(Spacing::MD))
        .rounded(px(Radius::MD))
        .border_1()
        .border_color(BorderColors::SUBTLE)
        .bg(gpui::Hsla {
            h: 0.0,
            s: 0.0,
            l: 1.0,
            a: 0.03,
        })
        .child(
            div()
                .text_color(Text::TERTIARY)
                .text_size(px(FontSize::XS))
                .font_weight(gpui::FontWeight::MEDIUM)
                .child(caption.to_uppercase()),
        );
    for child in children {
        card = card.child(child);
    }
    card
}

fn account_section_label(text: &str) -> impl IntoElement {
    div()
        .text_color(Text::TERTIARY)
        .text_size(px(FontSize::XS))
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .child(text.to_uppercase())
}

fn capsule_btn(id: &str, label: &str, prominent: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id.to_string())
        .self_start()
        .px(px(Spacing::MD_LG))
        .py(px(Spacing::SM))
        .rounded_full()
        .border_1()
        .border_color(if prominent {
            Accent::PRIMARY
        } else {
            BorderColors::PRIMARY
        })
        .bg(if prominent {
            Accent::PRIMARY
        } else {
            Background::BASE
        })
        .cursor_pointer()
        .text_color(if prominent {
            Background::BASE
        } else {
            Text::SECONDARY
        })
        .text_size(px(FontSize::SM))
        .child(label.to_string())
}

/// Account pane — three branches matching Swift AccountPane.
/// `is_signed_in`, `is_paid`, `is_loading` control which branch renders.
fn pane_account(is_loading: bool, is_signed_in: bool, is_paid: bool) -> impl IntoElement {
    let mut col = div().flex().flex_col().gap(px(Spacing::LG));

    if is_loading {
        col = col.child(body_text("Loading…"));
    } else if is_signed_in && is_paid {
        // ── Signed in, paid: subscription + credits ──
        col = col
            // Subscription section
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(Spacing::SM))
                    .child(account_section_label("Subscription"))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(Spacing::SM))
                            .child(
                                div()
                                    .text_color(Text::PRIMARY)
                                    .text_size(px(FontSize::MD))
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .child("Pro"),
                            )
                            .child(
                                capsule_btn("btn-manage-sub", "Manage subscription", false)
                                    .into_any_element(),
                            ),
                    ),
            )
            // Credits section: Remaining + Buy more cards side-by-side
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(Spacing::SM))
                    .child(account_section_label("Credits"))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap(px(Spacing::MD))
                            .child(account_card(
                                "Remaining",
                                vec![
                                    div()
                                        .text_color(Text::PRIMARY)
                                        .text_size(px(FontSize::LG))
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .child("1,000")
                                        .into_any_element(),
                                    div()
                                        .text_color(Text::TERTIARY)
                                        .text_size(px(FontSize::XS))
                                        .child("of 1,500 credits")
                                        .into_any_element(),
                                ],
                            ))
                            .child(account_card(
                                "Buy more",
                                vec![
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(Spacing::XS))
                                    .child(
                                        div()
                                            .text_color(Text::PRIMARY)
                                            .text_size(px(FontSize::MD))
                                            .child("$20"),
                                    )
                                    .into_any_element(),
                                div()
                                    .id("btn-buy-credits")
                                    .self_start()
                                    .px(px(Spacing::SM_MD))
                                    .py(px(Spacing::XXS))
                                    .rounded_full()
                                    .bg(Accent::PRIMARY)
                                    .cursor_pointer()
                                    .text_color(Background::BASE)
                                    .text_size(px(FontSize::XS))
                                    .child("Buy credits")
                                    .into_any_element(),
                                div()
                                    .text_color(Text::TERTIARY)
                                    .text_size(px(FontSize::XS))
                                    .child("$10–$500 · Unused credits expire at next billing date.")
                                    .into_any_element(),
                            ],
                            )),
                    ),
            )
            .child(capsule_btn("btn-sign-out", "Sign out", false).into_any_element());
    } else if is_signed_in && !is_paid {
        // ── Signed in, unpaid: upgrade options ──
        col = col
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(Spacing::SM))
                    .child(account_section_label("Subscription"))
                    .child(body_text("Subscribe to use AI generation."))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap(px(Spacing::MD))
                            // Pro card
                            .child(account_card(
                                "Pro",
                                vec![
                                    div()
                                        .flex()
                                        .items_baseline()
                                        .gap(px(Spacing::XS))
                                        .child(
                                            div()
                                                .text_color(Text::PRIMARY)
                                                .text_size(px(FontSize::XL))
                                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                                .child("$29"),
                                        )
                                        .child(
                                            div()
                                                .text_color(Text::TERTIARY)
                                                .text_size(px(FontSize::SM))
                                                .child("/ month"),
                                        )
                                        .into_any_element(),
                                    div()
                                        .text_color(Text::SECONDARY)
                                        .text_size(px(FontSize::SM))
                                        .child("1,500 credits / month")
                                        .into_any_element(),
                                    div()
                                        .id("btn-upgrade-pro")
                                        .w_full()
                                        .px(px(Spacing::SM))
                                        .py(px(Spacing::XS))
                                        .rounded_full()
                                        .bg(Accent::PRIMARY)
                                        .cursor_pointer()
                                        .text_color(Background::BASE)
                                        .text_size(px(FontSize::SM))
                                        .child("Upgrade to Pro")
                                        .into_any_element(),
                                ],
                            ))
                            // Max card
                            .child(account_card(
                                "Max",
                                vec![
                                    div()
                                        .flex()
                                        .items_baseline()
                                        .gap(px(Spacing::XS))
                                        .child(
                                            div()
                                                .text_color(Text::PRIMARY)
                                                .text_size(px(FontSize::XL))
                                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                                .child("$99"),
                                        )
                                        .child(
                                            div()
                                                .text_color(Text::TERTIARY)
                                                .text_size(px(FontSize::SM))
                                                .child("/ month"),
                                        )
                                        .into_any_element(),
                                    div()
                                        .text_color(Text::SECONDARY)
                                        .text_size(px(FontSize::SM))
                                        .child("6,000 credits / month")
                                        .into_any_element(),
                                    div()
                                        .id("btn-upgrade-max")
                                        .w_full()
                                        .px(px(Spacing::SM))
                                        .py(px(Spacing::XS))
                                        .rounded_full()
                                        .border_1()
                                        .border_color(BorderColors::PRIMARY)
                                        .cursor_pointer()
                                        .text_color(Text::SECONDARY)
                                        .text_size(px(FontSize::SM))
                                        .child("Upgrade to Max")
                                        .into_any_element(),
                                ],
                            )),
                    )
                    .child(body_text("Credits cover AI generation and chat.")),
            )
            .child(capsule_btn("btn-sign-out-2", "Sign out", false).into_any_element());
    } else {
        // ── Signed out ──
        col = col
            .child(body_text("Sign in to subscribe and use AI generation."))
            .child(capsule_btn("btn-sign-in", "Sign in with Google", false).into_any_element());
    }

    col
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

fn pane_models(
    image_on: &[bool; 3],
    video_on: &[bool; 3],
    audio_on: &[bool; 2],
) -> impl IntoElement {
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
        .child(model_section(
            "Image",
            &[
                ("Flux 1.1 Pro", image_on[0]),
                ("Flux 1.1 Pro Ultra", image_on[1]),
                ("Stable Diffusion XL", image_on[2]),
            ],
        ))
        .child(model_section(
            "Video",
            &[
                ("Kling 1.6 Pro", video_on[0]),
                ("Minimax Video 01", video_on[1]),
                ("Wan 2.1", video_on[2]),
            ],
        ))
        .child(model_section(
            "Audio",
            &[("Stable Audio 2.0", audio_on[0]), ("AudioGen", audio_on[1])],
        ))
}

// ── Agent pane ────────────────────────────────────────────────────────────────

fn pane_agent(
    mcp_running: bool,
    mcp_status_label: String,
    mcp_toggle: gpui::AnyElement,
    has_stored_api_key: bool,
) -> impl IntoElement {
    let dot_color = if mcp_running {
        gpui::Hsla {
            h: 0.33,
            s: 0.70,
            l: 0.45,
            a: 1.0,
        } // green
    } else {
        gpui::Hsla {
            h: 0.0,
            s: 0.0,
            l: 0.5,
            a: 1.0,
        }
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
                .child(body_text(
                    "Use your own API key for AI chat. Stored in your keychain.",
                ))
                .child(link_text("Get API key →"))
                // Key field row — stored key shows masked value + Remove; empty shows placeholder + Save
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(Spacing::XS))
                        .child(
                            div()
                                .flex_1()
                                .flex()
                                .flex_row()
                                .items_center()
                                .px(px(Spacing::SM_MD))
                                .h(px(32.0))
                                .rounded(px(Radius::SM))
                                .border_1()
                                .border_color(BorderColors::SUBTLE)
                                .bg(Background::RAISED)
                                .text_size(px(FontSize::SM))
                                .when(has_stored_api_key, |el| {
                                    el.text_color(Text::SECONDARY).child("sk-ant-api-03-···")
                                })
                                .when(!has_stored_api_key, |el| {
                                    el.text_color(Text::MUTED).child("Paste your API key")
                                }),
                        )
                        // Save button (only when no key stored)
                        .when(!has_stored_api_key, |el| {
                            el.child(
                                div()
                                    .id("btn-api-key-save")
                                    .px(px(Spacing::SM_MD))
                                    .h(px(32.0))
                                    .flex()
                                    .items_center()
                                    .rounded(px(Radius::SM))
                                    .bg(Accent::PRIMARY)
                                    .cursor_pointer()
                                    .text_color(Background::BASE)
                                    .text_size(px(FontSize::SM))
                                    .child("Save"),
                            )
                        })
                        // Remove (trash) button — only when key is stored
                        .when(has_stored_api_key, |el| {
                            el.child(
                                div()
                                    .id("btn-api-key-remove")
                                    .w(px(32.0))
                                    .h(px(32.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(px(Radius::SM))
                                    .border_1()
                                    .border_color(BorderColors::SUBTLE)
                                    .cursor_pointer()
                                    .text_color(Text::MUTED)
                                    .text_size(px(FontSize::SM))
                                    .child("🗑"),
                            )
                        }),
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
                                        .child(mcp_status_label),
                                )
                                .child(mcp_toggle),
                        ),
                ),
        )
}

// ── Storage pane ──────────────────────────────────────────────────────────────

fn format_bytes(bytes: u64) -> String {
    if bytes < 1_024 {
        format!("{bytes} B")
    } else if bytes < 1_048_576 {
        format!("{:.1} KB", bytes as f64 / 1_024.0)
    } else if bytes < 1_073_741_824 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else {
        format!("{:.2} GB", bytes as f64 / 1_073_741_824.0)
    }
}

fn pane_storage(search_enabled: bool, model_bytes: Option<u64>) -> impl IntoElement {
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
                .child(body_text("Indexes media on import so you can search it."))
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
        // ML model row — shown only when a model is downloaded (Swift: modelBytes > 0)
        .when_some(model_bytes, |el, mb| {
            el.child(divider()).child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(Spacing::SM_MD))
                    .child(section_title("ML Model"))
                    .child(body_text(
                        "Visual search embedding model downloaded to disk.",
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
                                            .child(format!("Model  {}", format_bytes(mb))),
                                    )
                                    .child(
                                        div()
                                            .id("btn-remove-model")
                                            .px(px(Spacing::SM_MD))
                                            .py(px(Spacing::XXS))
                                            .rounded(px(Radius::XS_SM))
                                            .border_1()
                                            .border_color(BorderColors::SUBTLE)
                                            .cursor_pointer()
                                            .text_color(Text::SECONDARY)
                                            .text_size(px(FontSize::XS))
                                            .child("Remove model"),
                                    ),
                            ),
                    ),
            )
        })
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
        let mut mcp_status_label = String::from("Stopped");
        if let Ok(svc) = crate::mcp_service::McpService::global().lock() {
            self.mcp_enabled = svc.is_enabled_preference();
            self.mcp_running = matches!(svc.status(), McpServerStatus::Running { .. });
            mcp_status_label = match svc.status() {
                McpServerStatus::Starting => "Starting\u{2026}".into(),
                McpServerStatus::Running { port } => format!("Running on 127.0.0.1:{port}"),
                McpServerStatus::Stopped => "Stopped".into(),
                McpServerStatus::Failed(reason) => format!("Failed: {reason}"),
            };
        }
        let active_tab = self.active_tab;
        let backend = self.backend_configured;

        let is_signed_in = self.is_signed_in;
        let is_paid = self.is_paid;
        let is_loading = self.is_loading;
        let account_initial = self.account_initial;
        let display_name = self.display_name.clone();
        let display_secondary = self.display_secondary.clone();

        // ── IdentityStrip (Swift: shown when !account.isMisconfigured) ──
        let identity_strip = if backend && is_signed_in {
            let initial_str = account_initial
                .map(|c| c.to_string())
                .unwrap_or("?".to_string());
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(Spacing::MD))
                .px(px(Spacing::LG))
                .py(px(Spacing::LG))
                .child(
                    // Avatar circle
                    div()
                        .w(px(32.0))
                        .h(px(32.0))
                        .rounded_full()
                        .bg(gpui::Hsla {
                            h: Accent::PRIMARY.h,
                            s: Accent::PRIMARY.s,
                            l: Accent::PRIMARY.l,
                            a: 0.5,
                        })
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(Text::PRIMARY)
                        .text_size(px(FontSize::MD_LG))
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .child(initial_str),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .gap(px(Spacing::XXS))
                        .child(
                            div()
                                .text_color(Text::PRIMARY)
                                .text_size(px(FontSize::MD))
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .child(if display_name.is_empty() {
                                    "Account".to_string()
                                } else {
                                    display_name
                                }),
                        )
                        .when_some(display_secondary, |el, sec| {
                            el.child(
                                div()
                                    .text_color(Text::TERTIARY)
                                    .text_size(px(FontSize::XS))
                                    .child(sec),
                            )
                        }),
                )
                .into_any_element()
        } else {
            div().into_any_element()
        };

        // Sidebar (220px, matching Swift — no explicit border_r, bg contrast creates separation)
        let mut sidebar = div()
            .flex()
            .flex_col()
            .w(px(220.0))
            .h_full()
            .bg(Background::SURFACE)
            .child(identity_strip)
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
                        gpui::Hsla {
                            h: 0.0,
                            s: 0.0,
                            l: 1.0,
                            a: Opacity::FAINT,
                        }
                    } else {
                        Background::SURFACE
                    })
                    .on_click(
                        cx.listener(move |this, _: &ClickEvent, _: &mut Window, cx| {
                            this.active_tab = tab;
                            cx.notify();
                        }),
                    )
                    .child(
                        div()
                            .text_color(if is_active {
                                Text::PRIMARY
                            } else {
                                Text::TERTIARY
                            })
                            .text_size(px(FontSize::SM_MD))
                            .child(tab_icon(tab)),
                    )
                    .child(
                        div()
                            .text_size(px(FontSize::SM))
                            .text_color(if is_active {
                                Text::PRIMARY
                            } else {
                                Text::SECONDARY
                            })
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
        let has_stored_api_key = self.has_stored_api_key;
        let search_enabled = self.search_enabled;
        let model_bytes = self.model_bytes;

        let pane_content: gpui::AnyElement = match active_tab {
            SettingsTab::Account => {
                pane_account(is_loading, is_signed_in, is_paid).into_any_element()
            }
            SettingsTab::General => pane_general(notifications_on, privacy_on).into_any_element(),
            SettingsTab::Models => pane_models(&image_on, &video_on, &audio_on).into_any_element(),
            SettingsTab::Agent => {
                let mcp_toggle = div()
                    .id("mcp-enabled-toggle")
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _: &ClickEvent, _: &mut Window, cx| {
                        let next = !this.mcp_enabled;
                        if let Ok(mut svc) = crate::mcp_service::McpService::global().lock() {
                            svc.set_enabled(next);
                            this.mcp_enabled = next;
                            this.mcp_running =
                                matches!(svc.status(), McpServerStatus::Running { .. });
                        }
                        cx.notify();
                    }))
                    .child(toggle_pill(mcp_enabled))
                    .into_any_element();
                pane_agent(
                    mcp_running,
                    mcp_status_label,
                    mcp_toggle,
                    has_stored_api_key,
                )
                .into_any_element()
            }
            SettingsTab::Storage => pane_storage(search_enabled, model_bytes).into_any_element(),
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
                    .text_size(px(FontSize::TITLE_2))
                    .font_weight(gpui::FontWeight::THIN)
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

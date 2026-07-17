//! Settings gpui view — sidebar + full pane content for all 6 tabs (#319).
//!
//! Matches Swift SettingsView + pane files after upstream #319: grouped
//! SettingsSection cards, unified sidebar rows, and the Skills pane
//! (list + editor sheet backed by `SkillStore`).

use crate::home_view::sidebar_row_button;
use crate::skill_store::{update_skill_md, SkillStore};
use crate::text_area::{TextArea, TextAreaEvent};
use crate::text_field::{TextField, TextFieldEvent};
use crate::theme::{
    Accent, Background, BorderColors, BorderWidth, FontSize, IconSize, Opacity, Radius, Spacing,
    Text,
};
use app_contract::agent_panel_model::McpServerStatus;
use gpui::{
    div, prelude::*, px, svg, AnyElement, App, ClickEvent, Context, Entity, FocusHandle, Focusable,
    Hsla, InteractiveElement, IntoElement, ParentElement, Render, SharedString, Styled,
    Subscription, Window,
};

/// Swift `AppTheme.Settings` metrics (#319). Local until the shared theme
/// module is open to this change; values mirror AppTheme.swift exactly.
pub struct SettingsMetrics;
impl SettingsMetrics {
    pub const SIDEBAR_WIDTH: f32 = 220.0;
    pub const CONTENT_MAX_WIDTH: f32 = 640.0;
    pub const SKILLS_SEARCH_WIDTH: f32 = 260.0;
    pub const SKILL_ROW_ICON_FRAME: f32 = 42.0;
    pub const SKILL_STATUS_WIDTH: f32 = 124.0;
    pub const SKILL_ACTION_WIDTH: f32 = 72.0;
    pub const SKILL_DETAIL_WIDTH: f32 = 720.0;
    pub const SKILL_DETAIL_MIN_HEIGHT: f32 = 600.0;
}

/// Swift `AppTheme.Accent.link` (NSColor.linkColor, dark appearance ≈ #0A84FF).
pub const LINK: Hsla = Hsla {
    h: 210.5 / 360.0,
    s: 1.0,
    l: 0.52,
    a: 1.0,
};

/// Settings tabs after #319 — Swift `SettingsTab` (adds Skills).
///
/// Local to the view: the app_contract `SettingsTab` (SETUI-001) still models
/// the pre-#319 five-tab surface and is owned by another spec slice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsPane {
    Account,
    General,
    Models,
    Agent,
    Skills,
    Storage,
}

impl SettingsPane {
    pub const ALL: &'static [SettingsPane] = &[
        SettingsPane::Account,
        SettingsPane::General,
        SettingsPane::Models,
        SettingsPane::Agent,
        SettingsPane::Skills,
        SettingsPane::Storage,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SettingsPane::Account => "Account",
            SettingsPane::General => "General",
            SettingsPane::Models => "Models",
            SettingsPane::Agent => "Agent",
            SettingsPane::Skills => "Skills",
            SettingsPane::Storage => "Storage",
        }
    }

    /// Embedded SVG standing in for the Swift SF Symbol of the same tab.
    pub fn icon_path(self) -> &'static str {
        match self {
            SettingsPane::Account => "icons/person_circle.svg",
            SettingsPane::General => "icons/gear.svg",
            SettingsPane::Models => "icons/squares_stack.svg",
            SettingsPane::Agent => "icons/paperplane.svg",
            SettingsPane::Skills => "icons/book_closed.svg",
            SettingsPane::Storage => "icons/internal_drive.svg",
        }
    }
}

/// Swift `visibleTabs`: Account hides when the backend is misconfigured.
pub fn visible_panes(backend_configured: bool) -> Vec<SettingsPane> {
    SettingsPane::ALL
        .iter()
        .copied()
        .filter(|pane| backend_configured || *pane != SettingsPane::Account)
        .collect()
}

/// Swift SkillsPane `matches`: case-insensitive query over name + description.
pub fn skill_matches(query: &str, name: &str, description: &str) -> bool {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return true;
    }
    name.to_lowercase().contains(&q) || description.to_lowercase().contains(&q)
}

/// Skill editor sheet state (Swift `SkillDetailSheet`).
struct SkillSheet {
    id: String,
    name_field: Entity<TextField>,
    description_field: Entity<TextField>,
    body_area: Entity<TextArea>,
    /// (name, description, body) at open/last save — dirty comparison base.
    original: (String, String, String),
    error: Option<String>,
    confirming_delete: bool,
}

/// gpui Settings view — carries mutable tab + toggle state.
pub struct SettingsView {
    focus_handle: FocusHandle,
    active_tab: SettingsPane,
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
    analytics_on: bool,
    crash_reports_on: bool,
    /// Crash-reports value at launch (Swift `Telemetry.enabledForCurrentLaunch`)
    /// — a differing toggle shows the restart note.
    crash_reports_at_launch: bool,
    // Models — toggled state per model (index-based for simplicity)
    image_models_on: [bool; 3],
    video_models_on: [bool; 3],
    audio_models_on: [bool; 2],
    // Agent
    mcp_running: bool,
    mcp_enabled: bool,
    /// True when the user has stored an Anthropic API key (AgentPane SecureField state).
    pub has_stored_api_key: bool,
    /// Whisper model path entry — commits `whisperModelPath` on Enter/blur.
    whisper_model_field: Entity<TextField>,
    /// Last committed value ("" = key removed); commit skips no-op writes.
    whisper_model_saved: String,
    /// Focus-out commit hook, registered on first render (`new` has no Window).
    whisper_blur_sub: Option<Subscription>,
    // Skills
    skill_store: SkillStore,
    skills_search: Entity<TextField>,
    skill_sheet: Option<SkillSheet>,
    // Storage
    search_enabled: bool,
    /// Size in bytes of locally cached ML model (None = no model downloaded).
    pub model_bytes: Option<u64>,
}

impl SettingsView {
    pub fn new(backend_configured: bool, cx: &mut Context<Self>) -> Self {
        let initial_tab = if backend_configured {
            SettingsPane::Account
        } else {
            SettingsPane::General
        };
        let skills_search = cx.new(|cx| TextField::new(cx, "Search skills"));
        cx.subscribe(&skills_search, |_, _, event, cx| {
            if matches!(event, TextFieldEvent::Edited) {
                cx.notify();
            }
        })
        .detach();
        let whisper_model_saved =
            crate::pane_prefs::load_whisper_model_path(&crate::pane_prefs::default_prefs_path())
                .map(|p| p.display().to_string())
                .unwrap_or_default();
        let whisper_model_field = cx.new(|cx| TextField::new(cx, "/path/to/ggml-model.bin"));
        whisper_model_field.update(cx, |f, cx| f.set_text(whisper_model_saved.clone(), cx));
        cx.subscribe(&whisper_model_field, |this: &mut Self, _, event, cx| {
            if matches!(event, TextFieldEvent::Submitted) {
                this.commit_whisper_model_path(cx);
            }
        })
        .detach();
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
            analytics_on: true,
            crash_reports_on: true,
            crash_reports_at_launch: true,
            image_models_on: [true, false, true],
            video_models_on: [true, true, false],
            audio_models_on: [true, false],
            mcp_running: true,
            mcp_enabled: true,
            has_stored_api_key: false,
            whisper_model_field,
            whisper_model_saved,
            whisper_blur_sub: None,
            skill_store: SkillStore::default_location(),
            skills_search,
            skill_sheet: None,
            search_enabled: true,
            model_bytes: None,
        }
    }

    /// Persist the whisper model path (Enter or focus loss). Blank removes
    /// the key; an unchanged value skips the write.
    fn commit_whisper_model_path(&mut self, cx: &mut Context<Self>) {
        let trimmed = self.whisper_model_field.read(cx).text().trim().to_string();
        if trimmed == self.whisper_model_saved {
            return;
        }
        crate::pane_prefs::save_whisper_model_path(
            &crate::pane_prefs::default_prefs_path(),
            &trimmed,
        );
        self.whisper_model_saved = trimmed.clone();
        self.whisper_model_field.update(cx, |f, cx| f.set_text(trimmed, cx));
        cx.notify();
    }

    // ── Skills sheet lifecycle ────────────────────────────────────────────────

    fn open_skill_sheet(&mut self, id: &str, cx: &mut Context<Self>) {
        let raw = self.skill_store.raw(id).unwrap_or_default();
        let (name, description, body) =
            crate::skill_store::required_fields(&raw).unwrap_or_default();
        let name_field = cx.new(|cx| TextField::new(cx, "Skill name"));
        name_field.update(cx, |f, cx| f.set_text(name.clone(), cx));
        let description_field = cx.new(|cx| TextField::new(cx, "One line on when to use this skill"));
        description_field.update(cx, |f, cx| f.set_text(description.clone(), cx));
        let body_area = cx.new(|cx| {
            TextArea::new(cx, "Skill instructions (markdown)")
                .with_min_lines(10)
                .with_max_lines(16)
        });
        body_area.update(cx, |a, cx| a.set_text(body.clone(), cx));
        for field in [&name_field, &description_field] {
            cx.subscribe(field, |_, _, event: &TextFieldEvent, cx| {
                if matches!(event, TextFieldEvent::Edited) {
                    cx.notify();
                }
            })
            .detach();
        }
        cx.subscribe(&body_area, |_, _, event: &TextAreaEvent, cx| {
            if matches!(event, TextAreaEvent::Edited) {
                cx.notify();
            }
        })
        .detach();
        self.skill_sheet = Some(SkillSheet {
            id: id.to_string(),
            name_field,
            description_field,
            body_area,
            original: (name, description, body),
            error: None,
            confirming_delete: false,
        });
        cx.notify();
    }

    fn sheet_values(&self, cx: &App) -> Option<(String, String, String)> {
        let sheet = self.skill_sheet.as_ref()?;
        Some((
            sheet.name_field.read(cx).text().to_string(),
            sheet.description_field.read(cx).text().to_string(),
            sheet.body_area.read(cx).text().to_string(),
        ))
    }

    fn save_skill_sheet(&mut self, cx: &mut Context<Self>) {
        let Some((name, description, body)) = self.sheet_values(cx) else {
            return;
        };
        let Some(sheet) = self.skill_sheet.as_mut() else {
            return;
        };
        let id = sheet.id.clone();
        let raw = self.skill_store.raw(&id).unwrap_or_default();
        let updated = update_skill_md(&raw, &name, &description, &body);
        match self.skill_store.save(&id, &updated) {
            Ok(()) => self.skill_sheet = None,
            Err(message) => {
                if let Some(sheet) = self.skill_sheet.as_mut() {
                    sheet.error = Some(message);
                }
            }
        }
        cx.notify();
    }

    fn delete_sheet_skill(&mut self, cx: &mut Context<Self>) {
        let Some(sheet) = self.skill_sheet.as_mut() else {
            return;
        };
        if !sheet.confirming_delete {
            sheet.confirming_delete = true;
            cx.notify();
            return;
        }
        let id = sheet.id.clone();
        match self.skill_store.delete(&id) {
            Ok(()) => self.skill_sheet = None,
            Err(message) => {
                if let Some(sheet) = self.skill_sheet.as_mut() {
                    sheet.error = Some(message);
                    sheet.confirming_delete = false;
                }
            }
        }
        cx.notify();
    }

    fn new_skill(&mut self, cx: &mut Context<Self>) {
        match self.skill_store.new_skill() {
            Ok(id) => {
                self.skills_search.update(cx, |f, cx| f.set_text("", cx));
                self.open_skill_sheet(&id, cx);
            }
            Err(err) => eprintln!("new skill failed: {err}"),
        }
    }
}

impl Focusable for SettingsView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

// ── Shared chrome (Swift SettingsSection / SettingsGroup / themedSurface) ────

/// Swift `themedSurface`: fill + subtle 1pt border, continuous corners.
fn themed_surface(fill: Hsla, radius: f32) -> gpui::Div {
    div()
        .rounded(px(radius))
        .border(px(BorderWidth::THIN))
        .border_color(BorderColors::SUBTLE)
        .bg(fill)
}

/// Swift `SettingsSection`: smMd regular primary title + prominent card.
fn settings_section(title: &str, children: Vec<AnyElement>) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap(px(Spacing::SM_MD))
        .child(
            div()
                .text_color(Text::PRIMARY)
                .text_size(px(FontSize::SM_MD))
                .child(title.to_string()),
        )
        .child(
            themed_surface(Background::PROMINENT, Radius::MD_LG)
                .flex()
                .flex_col()
                .w_full()
                .gap(px(Spacing::MD))
                .px(px(Spacing::LG_XL))
                .py(px(Spacing::MD_LG))
                .children(children),
        )
}

/// Swift `SettingsGroup`: smMd regular primary title, no card chrome.
fn settings_group(title: &str, children: Vec<AnyElement>) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap(px(Spacing::SM_MD))
        .child(
            div()
                .text_color(Text::PRIMARY)
                .text_size(px(FontSize::SM_MD))
                .child(title.to_string()),
        )
        .children(children)
}

fn body_text(text: &str) -> impl IntoElement {
    div()
        .text_color(Text::TERTIARY)
        .text_size(px(FontSize::SM))
        .child(text.to_string())
}

/// Link-styled inline button with the Swift ↗ arrow. Caller attaches on_click.
fn link_button(id: &str, label: &str) -> gpui::Stateful<gpui::Div> {
    div()
        .id(SharedString::from(id.to_string()))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(Spacing::XXS))
        .cursor_pointer()
        .text_color(LINK)
        .text_size(px(FontSize::SM))
        .child(label.to_string())
        .child(div().text_size(px(FontSize::XS)).child("↗"))
}

fn divider() -> impl IntoElement {
    div().w_full().h(px(BorderWidth::THIN)).bg(BorderColors::SUBTLE)
}

/// Toggle pill — 28×16 pill, knob slides right when on.
fn toggle_pill(is_on: bool) -> impl IntoElement {
    let bg = if is_on {
        Accent::PRIMARY
    } else {
        Hsla {
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

/// Swift `SettingsToggleRow` chrome: md title + sm tertiary subtitle, pill on
/// the right. Caller wraps with id + on_click for interactivity.
fn toggle_row_label(title: &str, subtitle: &str, is_on: bool) -> gpui::Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(Spacing::MD))
        .w_full()
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .gap(px(Spacing::XS))
                .child(
                    div()
                        .text_color(Text::PRIMARY)
                        .text_size(px(FontSize::MD))
                        .child(title.to_string()),
                )
                .when(!subtitle.is_empty(), |el| {
                    el.child(
                        div()
                            .text_color(Text::TERTIARY)
                            .text_size(px(FontSize::SM))
                            .child(subtitle.to_string()),
                    )
                }),
        )
        .child(toggle_pill(is_on))
}

/// Capsule button (Swift `CapsuleButtonStyle`). `prominent` = accent fill +
/// base text; secondary = raised fill + secondary text. `small` uses xs font.
fn capsule_btn(id: &str, label: &str, prominent: bool, small: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(SharedString::from(id.to_string()))
        .flex()
        .items_center()
        .justify_center()
        .px(px(if small { Spacing::SM_MD } else { Spacing::LG_XL }))
        .py(px(if small { Spacing::XS } else { Spacing::SM_MD }))
        .rounded_full()
        .cursor_pointer()
        .bg(if prominent {
            Accent::PRIMARY
        } else {
            Background::RAISED
        })
        .text_color(if prominent {
            Background::BASE
        } else {
            Text::SECONDARY
        })
        .text_size(px(if small { FontSize::XS } else { FontSize::SM_MD }))
        .font_weight(gpui::FontWeight::MEDIUM)
        .child(label.to_string())
}

// ── Account pane ──────────────────────────────────────────────────────────────

/// Mini card (Swift AccountPane.card): prominent surface, mdLg radius.
fn account_card(caption: &str, children: Vec<AnyElement>) -> impl IntoElement {
    themed_surface(Background::PROMINENT, Radius::MD_LG)
        .flex()
        .flex_col()
        .flex_1()
        .gap(px(Spacing::SM))
        .px(px(Spacing::LG_XL))
        .py(px(Spacing::MD_LG))
        .child(
            div()
                .text_color(Text::TERTIARY)
                .text_size(px(FontSize::XS))
                .child(caption.to_string()),
        )
        .children(children)
}

/// Account pane — three branches matching Swift AccountPane.
fn pane_account(is_loading: bool, is_signed_in: bool, is_paid: bool) -> impl IntoElement {
    let mut col = div().flex().flex_col().gap(px(Spacing::XXL));

    if is_loading {
        col = col.child(body_text("Loading…").into_any_element());
    } else if is_signed_in && is_paid {
        // ── Signed in, paid: subscription + credits ──
        col = col
            .child(settings_group(
                "Subscription",
                vec![themed_surface(Background::PROMINENT, Radius::MD_LG)
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(Spacing::MD))
                    .px(px(Spacing::LG_XL))
                    .py(px(Spacing::MD_LG))
                    .child(
                        div()
                            .flex_1()
                            .text_color(Text::PRIMARY)
                            .text_size(px(FontSize::MD))
                            .child("Pro"),
                    )
                    .child(
                        div()
                            .id("btn-manage-sub")
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(Spacing::XS))
                            .px(px(Spacing::LG_XL))
                            .py(px(Spacing::SM_MD))
                            .rounded_full()
                            .bg(Background::RAISED)
                            .cursor_pointer()
                            .text_color(Text::SECONDARY)
                            .text_size(px(FontSize::SM_MD))
                            .child("Manage subscription")
                            .child(div().text_size(px(FontSize::XS)).child("↗")),
                    )
                    .into_any_element()],
            ))
            .child(settings_group(
                "Credits",
                vec![div()
                    .flex()
                    .flex_row()
                    .items_start()
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
                                .gap(px(Spacing::SM))
                                .child(
                                    div()
                                        .text_color(Text::PRIMARY)
                                        .text_size(px(FontSize::MD))
                                        .child("$20"),
                                )
                                .child(capsule_btn("btn-buy-credits", "Buy credits", true, true))
                                .into_any_element(),
                            div()
                                .text_color(Text::TERTIARY)
                                .text_size(px(FontSize::XS))
                                .child("$10–$500 · Credits expire at renewal.")
                                .into_any_element(),
                        ],
                    ))
                    .into_any_element()],
            ))
            .child(
                div()
                    .flex()
                    .child(capsule_btn("btn-sign-out", "Sign out", false, false)),
            );
    } else if is_signed_in && !is_paid {
        // ── Signed in, unpaid: upgrade plan cards ──
        col = col
            .child(settings_group(
                "Subscription",
                vec![
                    div()
                        .flex()
                        .flex_row()
                        .items_start()
                        .gap(px(Spacing::MD))
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
                                capsule_btn("btn-upgrade-pro", "Upgrade to Pro", true, false)
                                    .w_full()
                                    .into_any_element(),
                            ],
                        ))
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
                                capsule_btn("btn-upgrade-max", "Upgrade to Max", false, false)
                                    .w_full()
                                    .into_any_element(),
                            ],
                        ))
                        .into_any_element(),
                    div()
                        .text_color(Text::TERTIARY)
                        .text_size(px(FontSize::XS))
                        .child("Credits cover AI generation and chat.")
                        .into_any_element(),
                ],
            ))
            .child(
                div()
                    .flex()
                    .child(capsule_btn("btn-sign-out-2", "Sign out", false, false)),
            );
    } else {
        // ── Signed out ──
        col = col
            .child(body_text("Sign in to subscribe and use AI generation.").into_any_element())
            .child(
                div()
                    .flex()
                    .child(capsule_btn("btn-sign-in", "Sign in with Google", false, false)),
            );
    }

    col
}

// ── Storage pane helper ───────────────────────────────────────────────────────

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

// ── Render ────────────────────────────────────────────────────────────────────

impl SettingsView {
    fn render_general_pane(&self, cx: &mut Context<Self>) -> AnyElement {
        let analytics_on = self.analytics_on;
        let crash_on = self.crash_reports_on;
        let restart_needed = self.crash_reports_on != self.crash_reports_at_launch;

        let mut privacy_rows: Vec<AnyElement> = vec![
            toggle_row_label(
                "Share usage data",
                "Send product usage data to help improve Fronda. Media and project content are never included.",
                analytics_on,
            )
            .id("toggle-analytics")
            .cursor_pointer()
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.analytics_on = !this.analytics_on;
                cx.notify();
            }))
            .into_any_element(),
            divider().into_any_element(),
            toggle_row_label(
                "Send crash reports",
                "Send crash and error reports to help diagnose problems. Media and project content are never included.",
                crash_on,
            )
            .id("toggle-crash-reports")
            .cursor_pointer()
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.crash_reports_on = !this.crash_reports_on;
                cx.notify();
            }))
            .into_any_element(),
        ];
        if restart_needed {
            privacy_rows.push(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(Spacing::XS))
                    .text_color(Text::SECONDARY)
                    .text_size(px(FontSize::SM))
                    .child("↻")
                    .child("Restart Fronda to apply this change.")
                    .into_any_element(),
            );
        }

        div()
            .flex()
            .flex_col()
            .gap(px(Spacing::XXL))
            .child(settings_section(
                "Notifications",
                vec![toggle_row_label(
                    "Show notifications",
                    "Get a notification when a generation finishes.",
                    self.notifications_on,
                )
                .id("toggle-notifications")
                .cursor_pointer()
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.notifications_on = !this.notifications_on;
                    cx.notify();
                }))
                .into_any_element()],
            ))
            .child(settings_section("Privacy & Diagnostics", privacy_rows))
            .into_any_element()
    }

    fn render_models_pane(&self, cx: &mut Context<Self>) -> AnyElement {
        fn model_rows(
            cx: &mut Context<SettingsView>,
            section: &'static str,
            models: &[(usize, &str, bool)],
        ) -> Vec<AnyElement> {
            let mut rows: Vec<AnyElement> = Vec::new();
            for (i, &(ix, name, on)) in models.iter().enumerate() {
                if i > 0 {
                    rows.push(divider().into_any_element());
                }
                rows.push(
                    div()
                        .id(SharedString::from(format!("model-{section}-{ix}")))
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(Spacing::MD))
                        .w_full()
                        .py(px(Spacing::SM_MD))
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            match section {
                                "image" => this.image_models_on[ix] = !this.image_models_on[ix],
                                "video" => this.video_models_on[ix] = !this.video_models_on[ix],
                                _ => this.audio_models_on[ix] = !this.audio_models_on[ix],
                            }
                            cx.notify();
                        }))
                        .child(
                            div()
                                .flex_1()
                                .text_color(Text::PRIMARY)
                                .text_size(px(FontSize::MD))
                                .child(name.to_string()),
                        )
                        .child(toggle_pill(on))
                        .into_any_element(),
                );
            }
            rows
        }

        let image = [
            (0usize, "Flux 1.1 Pro", self.image_models_on[0]),
            (1, "Flux 1.1 Pro Ultra", self.image_models_on[1]),
            (2, "Stable Diffusion XL", self.image_models_on[2]),
        ];
        let video = [
            (0usize, "Kling 1.6 Pro", self.video_models_on[0]),
            (1, "Minimax Video 01", self.video_models_on[1]),
            (2, "Wan 2.1", self.video_models_on[2]),
        ];
        let audio = [
            (0usize, "Stable Audio 2.0", self.audio_models_on[0]),
            (1, "AudioGen", self.audio_models_on[1]),
        ];

        let image_rows = model_rows(cx, "image", &image);
        let video_rows = model_rows(cx, "video", &video);
        let audio_rows = model_rows(cx, "audio", &audio);

        div()
            .flex()
            .flex_col()
            .gap(px(Spacing::LG))
            // Search field chrome (Swift ModelsPane.searchBar).
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(Spacing::SM))
                    .px(px(Spacing::MD))
                    .py(px(Spacing::SM_MD))
                    .rounded(px(Radius::MD))
                    .border(px(BorderWidth::THIN))
                    .border_color(BorderColors::PRIMARY)
                    .bg(Hsla {
                        h: 0.0,
                        s: 0.0,
                        l: 1.0,
                        a: Opacity::SUBTLE,
                    })
                    .text_color(Text::MUTED)
                    .text_size(px(FontSize::SM))
                    .child("⌕")
                    .child("Search models"),
            )
            .child(settings_section("Image", image_rows))
            .child(settings_section("Video", video_rows))
            .child(settings_section("Audio", audio_rows))
            .into_any_element()
    }

    fn render_agent_pane(&self, mcp_status_label: String, cx: &mut Context<Self>) -> AnyElement {
        let mcp_running = self.mcp_running;
        let mcp_enabled = self.mcp_enabled;
        let has_stored_api_key = self.has_stored_api_key;
        let dot_color = if mcp_running {
            Hsla {
                h: 0.33,
                s: 0.70,
                l: 0.45,
                a: 1.0,
            }
        } else {
            Text::MUTED
        };

        // ── AI Chat: API key ──
        let api_key_section = settings_section(
            "AI Chat",
            vec![
                div()
                    .flex()
                    .flex_col()
                    .gap(px(Spacing::XS))
                    .child(
                        div()
                            .text_color(Text::PRIMARY)
                            .text_size(px(FontSize::MD))
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .child("Anthropic API Key"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_baseline()
                            .gap(px(Spacing::SM))
                            .child(body_text("Use your own API key for AI chat. Stored locally."))
                            .child(link_button("link-get-api-key", "Get Anthropic API key").on_click(
                                |_: &ClickEvent, _, _| {
                                    crate::platform_adapter::open_url(
                                        "https://console.anthropic.com/settings/keys",
                                    );
                                },
                            )),
                    )
                    .into_any_element(),
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(Spacing::SM))
                    .child(
                        themed_surface(Background::RAISED, Radius::SM)
                            .flex_1()
                            .flex()
                            .flex_row()
                            .items_center()
                            .px(px(Spacing::MD))
                            .py(px(Spacing::SM_MD))
                            .text_size(px(FontSize::SM))
                            .when(has_stored_api_key, |el| {
                                el.text_color(Text::SECONDARY).child("••••••••••••••••")
                            })
                            .when(!has_stored_api_key, |el| {
                                el.text_color(Text::MUTED).child("sk-ant-…")
                            }),
                    )
                    .when(!has_stored_api_key, |el| {
                        el.child(capsule_btn("btn-api-key-save", "Save", true, false))
                    })
                    .when(has_stored_api_key, |el| {
                        el.child(
                            capsule_btn("btn-api-key-remove", "Remove", false, false).on_click(
                                cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.has_stored_api_key = false;
                                    cx.notify();
                                }),
                            ),
                        )
                    })
                    .into_any_element(),
                div()
                    .flex()
                    .flex_col()
                    .gap(px(Spacing::XS))
                    .pt(px(Spacing::SM))
                    .child(
                        div()
                            .text_color(Text::PRIMARY)
                            .text_size(px(FontSize::MD))
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .child("Whisper model"),
                    )
                    .child(body_text(
                        "Path to a whisper GGML or GGUF model file for local transcription. Leave empty to unset.",
                    ))
                    .into_any_element(),
                themed_surface(Background::RAISED, Radius::SM)
                    .w_full()
                    .px(px(Spacing::MD))
                    .py(px(Spacing::SM_MD))
                    .text_size(px(FontSize::SM))
                    .text_color(Text::PRIMARY)
                    .child(self.whisper_model_field.clone())
                    .into_any_element(),
            ],
        );

        // ── Integrations: MCP server ──
        let mcp_section = settings_section(
            "Integrations",
            vec![
                div()
                    .flex()
                    .flex_col()
                    .gap(px(Spacing::XS))
                    .child(
                        div()
                            .text_color(Text::PRIMARY)
                            .text_size(px(FontSize::MD))
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .child("MCP Server"),
                    )
                    .child(body_text(
                        "Lets external clients like Cursor, Claude Desktop, Claude Code, and Codex edit your timeline.",
                    ))
                    .into_any_element(),
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(Spacing::SM))
                    .pt(px(Spacing::XS))
                    .child(
                        div()
                            .w(px(Spacing::SM_MD))
                            .h(px(Spacing::SM_MD))
                            .rounded_full()
                            .bg(dot_color),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_color(if mcp_running {
                                Text::PRIMARY
                            } else {
                                Text::TERTIARY
                            })
                            .text_size(px(FontSize::SM))
                            .child(mcp_status_label),
                    )
                    .child(
                        div()
                            .id("mcp-enabled-toggle")
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                let next = !this.mcp_enabled;
                                if let Ok(mut svc) = crate::mcp_service::McpService::global().lock()
                                {
                                    svc.set_enabled(next);
                                    this.mcp_enabled = next;
                                    this.mcp_running =
                                        matches!(svc.status(), McpServerStatus::Running { .. });
                                }
                                cx.notify();
                            }))
                            .child(toggle_pill(mcp_enabled)),
                    )
                    .into_any_element(),
            ],
        );

        div()
            .flex()
            .flex_col()
            .gap(px(Spacing::XXL))
            .child(api_key_section)
            .child(mcp_section)
            .into_any_element()
    }

    fn render_storage_pane(&self, cx: &mut Context<Self>) -> AnyElement {
        let search_enabled = self.search_enabled;
        let model_bytes = self.model_bytes;

        let cache_section = settings_section(
            "Cache",
            vec![div()
                .flex()
                .flex_row()
                .items_start()
                .gap(px(Spacing::MD))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .gap(px(Spacing::XS))
                        .child(
                            div()
                                .text_color(Text::PRIMARY)
                                .text_size(px(FontSize::MD))
                                .child("Temporary files"),
                        )
                        .child(body_text(
                            "Playback previews, waveforms, filmstrip thumbnails, and transcripts. Safe to clear; files rebuild as needed.",
                        ))
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(Spacing::SM))
                                .pt(px(Spacing::XS))
                                .child(
                                    div()
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
                        ),
                )
                .child(capsule_btn("btn-clear-cache", "Clear cache", false, true))
                .into_any_element()],
        );

        let mut search_rows: Vec<AnyElement> = vec![
            div()
                .flex()
                .flex_row()
                .items_start()
                .gap(px(Spacing::MD))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .gap(px(Spacing::XS))
                        .child(
                            div()
                                .text_color(Text::PRIMARY)
                                .text_size(px(FontSize::MD))
                                .child("Media indexing"),
                        )
                        .child(body_text("Indexes imported media for on-device search.")),
                )
                .child(
                    div()
                        .id("toggle-search-index")
                        .cursor_pointer()
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.search_enabled = !this.search_enabled;
                            cx.notify();
                        }))
                        .child(toggle_pill(search_enabled)),
                )
                .into_any_element(),
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(Spacing::SM))
                .pt(px(Spacing::XS))
                .child(
                    div()
                        .text_color(Text::TERTIARY)
                        .text_size(px(FontSize::XS))
                        .child("Index"),
                )
                .child(
                    div()
                        .flex_1()
                        .text_color(Text::SECONDARY)
                        .text_size(px(FontSize::XS))
                        .child("0 B"),
                )
                .child(capsule_btn("btn-clear-index", "Clear index", false, true))
                .into_any_element(),
        ];
        if let Some(mb) = model_bytes {
            search_rows.push(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(Spacing::SM))
                    .child(
                        div()
                            .text_color(Text::TERTIARY)
                            .text_size(px(FontSize::XS))
                            .child("Model"),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_color(Text::SECONDARY)
                            .text_size(px(FontSize::XS))
                            .child(format_bytes(mb)),
                    )
                    .child(capsule_btn("btn-remove-model", "Remove model", false, true))
                    .into_any_element(),
            );
        }

        div()
            .flex()
            .flex_col()
            .gap(px(Spacing::XXL))
            .child(cache_section)
            .child(settings_section("Search", search_rows))
            .into_any_element()
    }

    // ── Skills pane (Swift SkillsPane) ────────────────────────────────────────

    fn render_skills_pane(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let query = self.skills_search.read(cx).text().to_string();
        let mut visible: Vec<(String, String, String)> = self
            .skill_store
            .skills()
            .iter()
            .filter(|s| skill_matches(&query, &s.name, &s.description))
            .map(|s| (s.id.clone(), s.name.clone(), s.description.clone()))
            .collect();
        visible.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()));
        let installed_count = self.skill_store.skills().len();

        // ── Introduction ──
        let introduction = div()
            .flex()
            .flex_col()
            .gap(px(Spacing::XS))
            .child(body_text(
                "Install skills to give the in-app agent specialized workflows.",
            ))
            .child(
                link_button("link-community-skills", "Browse Community Skills").on_click(
                    |_: &ClickEvent, _, _| {
                        crate::platform_adapter::open_url(
                            "https://github.com/palmier-io/palmier-skills",
                        );
                    },
                ),
            );

        // ── Controls: collection pill, search, new ──
        let controls = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(Spacing::SM_MD))
            .child(
                // Installed collection pill (Swift SkillCollectionButton; the
                // Community collection needs the SkillCatalog backend — not ported).
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(Spacing::SM))
                    .px(px(Spacing::MD_LG))
                    .py(px(Spacing::SM))
                    .rounded(px(Radius::XL))
                    .bg(Background::RAISED)
                    .text_size(px(FontSize::MD))
                    .text_color(Text::PRIMARY)
                    .child("Installed")
                    .child(
                        div()
                            .text_color(Text::TERTIARY)
                            .child(installed_count.to_string()),
                    ),
            )
            .child(div().flex_1())
            .child(
                themed_surface(Background::RAISED, Radius::SM)
                    .id("skills-search-box")
                    .w(px(SettingsMetrics::SKILLS_SEARCH_WIDTH))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(Spacing::SM))
                    .px(px(Spacing::MD))
                    .py(px(Spacing::SM_MD))
                    .cursor_text()
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                        window.focus(&this.skills_search.focus_handle(cx), cx);
                        cx.notify();
                    }))
                    .child(
                        div()
                            .text_color(Text::MUTED)
                            .text_size(px(FontSize::SM))
                            .child("⌕"),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(FontSize::SM))
                            .text_color(Text::PRIMARY)
                            .child(self.skills_search.clone()),
                    ),
            )
            .child(
                div()
                    .id("btn-new-skill")
                    .w(px(IconSize::MD))
                    .h(px(IconSize::MD))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(Radius::SM))
                    .cursor_pointer()
                    .hover(|s| {
                        s.bg(Hsla {
                            h: 0.0,
                            s: 0.0,
                            l: 1.0,
                            a: Opacity::FAINT,
                        })
                    })
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.new_skill(cx);
                    }))
                    .child(
                        svg()
                            .path("icons/plus.svg")
                            .w(px(IconSize::XXS))
                            .h(px(IconSize::XXS))
                            .text_color(Text::SECONDARY),
                    ),
            );

        // ── List / empty states ──
        let list: AnyElement = if visible.is_empty() && query.trim().is_empty() {
            self.skill_empty_state(
                "No Installed Skills",
                "Create a skill to give the in-app agent specialized workflows.",
                "New Skill",
                cx,
            )
        } else if visible.is_empty() {
            self.skill_no_matches_state(cx)
        } else {
            let mut rows = div().flex().flex_col().gap(px(Spacing::SM)).w_full();
            for (id, name, description) in visible {
                let open_id = id.clone();
                rows = rows.child(
                    div()
                        .id(SharedString::from(format!("skill-row-{id}")))
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(Spacing::MD))
                        .w_full()
                        .px(px(Spacing::SM_MD))
                        .py(px(Spacing::SM_MD))
                        .rounded(px(Radius::MD))
                        .cursor_pointer()
                        .hover(|s| {
                            s.bg(Hsla {
                                h: 0.0,
                                s: 0.0,
                                l: 1.0,
                                a: Opacity::FAINT,
                            })
                        })
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.open_skill_sheet(&open_id, cx);
                        }))
                        // Icon + name + description (Swift SkillRowSummary).
                        .child(
                            div()
                                .w(px(SettingsMetrics::SKILL_ROW_ICON_FRAME))
                                .h(px(SettingsMetrics::SKILL_ROW_ICON_FRAME))
                                .flex_none()
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded_full()
                                .border(px(BorderWidth::THIN))
                                .border_color(BorderColors::SUBTLE)
                                .child(
                                    svg()
                                        .path("icons/book_closed.svg")
                                        .w(px(FontSize::MD))
                                        .h(px(FontSize::MD))
                                        .text_color(Text::TERTIARY),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .flex_1()
                                .gap(px(Spacing::XXS))
                                .overflow_hidden()
                                .child(
                                    div()
                                        .text_color(Text::PRIMARY)
                                        .text_size(px(FontSize::MD_LG))
                                        .whitespace_nowrap()
                                        .overflow_hidden()
                                        .text_ellipsis()
                                        .child(name),
                                )
                                .child(
                                    div()
                                        .text_color(Text::TERTIARY)
                                        .text_size(px(FontSize::SM_MD))
                                        .whitespace_nowrap()
                                        .overflow_hidden()
                                        .text_ellipsis()
                                        .child(description),
                                ),
                        )
                        .child(
                            div()
                                .w(px(SettingsMetrics::SKILL_STATUS_WIDTH))
                                .flex_none()
                                .flex()
                                .justify_end()
                                .text_color(Text::TERTIARY)
                                .text_size(px(FontSize::SM_MD))
                                .child("Local"),
                        )
                        .child(
                            div()
                                .w(px(SettingsMetrics::SKILL_ACTION_WIDTH))
                                .flex_none()
                                .flex()
                                .justify_end()
                                .child(capsule_btn(&format!("skill-open-{id}"), "Open", false, true)),
                        ),
                );
            }
            rows.into_any_element()
        };

        div()
            .flex()
            .flex_col()
            .flex_1()
            .gap(px(Spacing::XXL))
            .child(introduction)
            .child(controls)
            .child(
                div()
                    .id("skills-list-scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .child(list),
            )
            .into_any_element()
    }

    fn skill_empty_state(
        &self,
        title: &str,
        message: &str,
        action_title: &str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(Spacing::SM_MD))
            .w_full()
            .p(px(Spacing::XL_XXL))
            .child(
                svg()
                    .path("icons/book_closed.svg")
                    .w(px(IconSize::SM_MD))
                    .h(px(IconSize::SM_MD))
                    .text_color(Text::MUTED),
            )
            .child(
                div()
                    .text_color(Text::PRIMARY)
                    .text_size(px(FontSize::SM_MD))
                    .child(title.to_string()),
            )
            .child(
                div()
                    .text_color(Text::TERTIARY)
                    .text_size(px(FontSize::SM))
                    .child(message.to_string()),
            )
            .child(
                capsule_btn("btn-empty-new-skill", action_title, false, true).on_click(
                    cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.new_skill(cx);
                    }),
                ),
            )
            .into_any_element()
    }

    fn skill_no_matches_state(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(Spacing::SM_MD))
            .w_full()
            .p(px(Spacing::XL_XXL))
            .child(
                div()
                    .text_color(Text::MUTED)
                    .text_size(px(FontSize::XL))
                    .child("⌕"),
            )
            .child(
                div()
                    .text_color(Text::PRIMARY)
                    .text_size(px(FontSize::SM_MD))
                    .child("No Matching Skills"),
            )
            .child(
                div()
                    .text_color(Text::TERTIARY)
                    .text_size(px(FontSize::SM))
                    .child("Try another search."),
            )
            .child(
                capsule_btn("btn-clear-skill-search", "Clear Search", false, true).on_click(
                    cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.skills_search.update(cx, |f, cx| f.set_text("", cx));
                        cx.notify();
                    }),
                ),
            )
            .into_any_element()
    }

    // ── Skill editor sheet (Swift SkillDetailSheet) ───────────────────────────

    fn render_skill_sheet(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let values = self.sheet_values(cx)?;
        let sheet = self.skill_sheet.as_ref()?;
        let dirty = values != sheet.original;
        let (name, _, _) = values;
        let error = sheet.error.clone();
        let confirming_delete = sheet.confirming_delete;
        let title = if name.trim().is_empty() {
            sheet.id.clone()
        } else {
            name
        };

        fn field_label(text: &str) -> AnyElement {
            div()
                .text_color(Text::SECONDARY)
                .text_size(px(FontSize::SM))
                .font_weight(gpui::FontWeight::MEDIUM)
                .child(text.to_string())
                .into_any_element()
        }

        let name_field = sheet.name_field.clone();
        let description_field = sheet.description_field.clone();
        let body_area = sheet.body_area.clone();

        let card = div()
            .id("skill-sheet-card")
            .w(px(SettingsMetrics::SKILL_DETAIL_WIDTH))
            .min_h(px(SettingsMetrics::SKILL_DETAIL_MIN_HEIGHT))
            .max_h_full()
            .flex()
            .flex_col()
            .rounded(px(Radius::MD_LG))
            .border(px(BorderWidth::THIN))
            .border_color(BorderColors::PRIMARY)
            .bg(Background::PROMINENT)
            .overflow_hidden()
            .on_click(|_, _, cx| cx.stop_propagation())
            // ── Header ──
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(Spacing::MD))
                    .px(px(Spacing::XL_XXL))
                    .py(px(Spacing::MD_LG))
                    .child(
                        div()
                            .flex_1()
                            .text_color(Text::PRIMARY)
                            .text_size(px(FontSize::XL))
                            .whitespace_nowrap()
                            .overflow_hidden()
                            .text_ellipsis()
                            .child(title),
                    )
                    .child(
                        div()
                            .id("skill-sheet-close")
                            .w(px(IconSize::MD))
                            .h(px(IconSize::MD))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(Radius::SM))
                            .cursor_pointer()
                            .text_color(Text::TERTIARY)
                            .text_size(px(FontSize::MD))
                            .hover(|s| {
                                s.bg(Hsla {
                                    h: 0.0,
                                    s: 0.0,
                                    l: 1.0,
                                    a: Opacity::FAINT,
                                })
                            })
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.skill_sheet = None;
                                cx.notify();
                            }))
                            .child("✕"),
                    ),
            )
            .child(divider())
            // ── Fields ──
            .child(
                div()
                    .id("skill-sheet-fields")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .gap(px(Spacing::LG))
                    .px(px(Spacing::XL_XXL))
                    .py(px(Spacing::LG_XL))
                    .overflow_y_scroll()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(Spacing::XS))
                            .child(field_label("Name"))
                            .child(
                                themed_surface(Background::RAISED, Radius::SM)
                                    .px(px(Spacing::MD))
                                    .py(px(Spacing::SM_MD))
                                    .text_size(px(FontSize::MD))
                                    .text_color(Text::PRIMARY)
                                    .child(name_field),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(Spacing::XS))
                            .child(field_label("Description"))
                            .child(
                                themed_surface(Background::RAISED, Radius::SM)
                                    .px(px(Spacing::MD))
                                    .py(px(Spacing::SM_MD))
                                    .text_size(px(FontSize::MD))
                                    .text_color(Text::PRIMARY)
                                    .child(description_field),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(Spacing::XS))
                            .child(field_label("Instructions"))
                            .child(
                                themed_surface(Background::RAISED, Radius::MD)
                                    .p(px(Spacing::MD))
                                    .text_size(px(FontSize::SM))
                                    .text_color(Text::PRIMARY)
                                    .child(body_area),
                            ),
                    )
                    .when_some(error, |el, message| {
                        el.child(
                            div()
                                .text_color(crate::theme::Status::ERROR)
                                .text_size(px(FontSize::SM))
                                .child(message),
                        )
                    }),
            )
            .child(divider())
            // ── Footer ──
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(Spacing::SM_MD))
                    .px(px(Spacing::XL_XXL))
                    .py(px(Spacing::MD_LG))
                    .child(
                        div()
                            .id("skill-sheet-delete")
                            .px(px(Spacing::MD_LG))
                            .py(px(Spacing::SM))
                            .rounded_full()
                            .cursor_pointer()
                            .border(px(BorderWidth::THIN))
                            .border_color(crate::theme::Status::ERROR)
                            .text_color(crate::theme::Status::ERROR)
                            .text_size(px(FontSize::SM_MD))
                            .when(confirming_delete, |el| {
                                el.bg(crate::theme::Status::ERROR).text_color(Text::PRIMARY)
                            })
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.delete_sheet_skill(cx);
                            }))
                            .child(if confirming_delete {
                                "Confirm Delete"
                            } else {
                                "Delete Skill"
                            }),
                    )
                    .child(div().flex_1())
                    .child(
                        capsule_btn("skill-sheet-cancel", "Cancel", false, false).on_click(
                            cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.skill_sheet = None;
                                cx.notify();
                            }),
                        ),
                    )
                    .when(dirty, |el| {
                        el.child(
                            capsule_btn("skill-sheet-save", "Save Changes", true, false).on_click(
                                cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.save_skill_sheet(cx);
                                }),
                            ),
                        )
                    }),
            );

        Some(
            div()
                .id("skill-sheet-scrim")
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(Hsla {
                    h: 0.0,
                    s: 0.0,
                    l: 0.0,
                    a: Opacity::MEDIUM,
                })
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.skill_sheet = None;
                    cx.notify();
                }))
                .child(card)
                .into_any_element(),
        )
    }
}

impl Render for SettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.whisper_blur_sub.is_none() {
            let field_focus = self.whisper_model_field.read(cx).focus_handle(cx);
            let weak = cx.entity().downgrade();
            self.whisper_blur_sub = Some(window.on_focus_out(&field_focus, cx, move |_, _, cx| {
                if let Some(view) = weak.upgrade() {
                    view.update(cx, |this, cx| this.commit_whisper_model_path(cx));
                }
            }));
        }
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
        let backend = self.backend_configured;
        if !visible_panes(backend).contains(&self.active_tab) {
            self.active_tab = SettingsPane::General;
        }
        let active_tab = self.active_tab;

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
                    div()
                        .w(px(IconSize::XL))
                        .h(px(IconSize::XL))
                        .rounded_full()
                        .bg(Hsla {
                            a: 0.5,
                            ..Accent::PRIMARY
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

        // ── Sidebar: unified rows (Swift SettingsSidebar + SidebarRowButton) ──
        let mut tab_list = div()
            .flex()
            .flex_col()
            .gap(px(Spacing::XXS))
            .px(px(Spacing::SM_MD))
            .py(px(Spacing::MD));
        for pane in visible_panes(backend) {
            let is_active = active_tab == pane;
            tab_list = tab_list.child(
                sidebar_row_button(
                    format!("stab-{}", pane.label()),
                    pane.icon_path(),
                    pane.label(),
                    is_active,
                )
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.active_tab = pane;
                    cx.notify();
                })),
            );
        }
        let sidebar = div()
            .flex()
            .flex_col()
            .w(px(SettingsMetrics::SIDEBAR_WIDTH))
            .h_full()
            .bg(Background::SURFACE)
            .child(identity_strip)
            .child(tab_list);

        // ── Detail: title + pane, capped at contentMaxWidth and centered ──
        let title = div()
            .w_full()
            .flex()
            .justify_center()
            .px(px(Spacing::XXL))
            .pt(px(Spacing::XXL))
            .pb(px(Spacing::XXL))
            .child(
                div()
                    .w_full()
                    .max_w(px(SettingsMetrics::CONTENT_MAX_WIDTH))
                    .text_color(Text::PRIMARY)
                    .text_size(px(FontSize::TITLE_1))
                    .child(active_tab.label()),
            );

        let pane_content: AnyElement = match active_tab {
            SettingsPane::Account => {
                pane_account(is_loading, is_signed_in, is_paid).into_any_element()
            }
            SettingsPane::General => self.render_general_pane(cx),
            SettingsPane::Models => self.render_models_pane(cx),
            SettingsPane::Agent => self.render_agent_pane(mcp_status_label, cx),
            SettingsPane::Skills => self.render_skills_pane(cx),
            SettingsPane::Storage => self.render_storage_pane(cx),
        };

        // Skills manages its own scrolling (list only); other panes scroll whole.
        let body: AnyElement = if active_tab == SettingsPane::Skills {
            div()
                .flex_1()
                .w_full()
                .flex()
                .justify_center()
                .px(px(Spacing::XXL))
                .pb(px(Spacing::XXL))
                .child(
                    div()
                        .w_full()
                        .max_w(px(SettingsMetrics::CONTENT_MAX_WIDTH))
                        .h_full()
                        .flex()
                        .flex_col()
                        .child(pane_content),
                )
                .into_any_element()
        } else {
            div()
                .id("settings-content-scroll")
                .flex_1()
                .w_full()
                .overflow_y_scroll()
                .px(px(Spacing::XXL))
                .pb(px(Spacing::XXL))
                .child(
                    div().w_full().flex().justify_center().child(
                        div()
                            .w_full()
                            .max_w(px(SettingsMetrics::CONTENT_MAX_WIDTH))
                            .child(pane_content),
                    ),
                )
                .into_any_element()
        };

        let detail = div()
            .flex()
            .flex_col()
            .flex_1()
            .h_full()
            .overflow_hidden()
            .bg(Background::BASE)
            .child(title)
            .child(body);

        let sheet = self.render_skill_sheet(cx);

        div()
            .id("fronda-settings")
            .track_focus(&self.focus_handle.clone())
            .relative()
            .flex()
            .flex_row()
            .size_full()
            .bg(Background::SURFACE)
            .child(sidebar)
            .child(detail)
            .when_some(sheet, |el, sheet| el.child(sheet))
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // Swift SettingsTab order + labels (#319 adds Skills between Agent and Storage).
    #[test]
    fn pane_list_mirrors_swift_settings_tabs() {
        let labels: Vec<&str> = SettingsPane::ALL.iter().map(|p| p.label()).collect();
        assert_eq!(
            labels,
            ["Account", "General", "Models", "Agent", "Skills", "Storage"]
        );
    }

    #[test]
    fn visible_panes_hide_account_when_misconfigured() {
        assert_eq!(visible_panes(true).len(), 6);
        let hidden = visible_panes(false);
        assert_eq!(hidden.len(), 5);
        assert!(!hidden.contains(&SettingsPane::Account));
        assert_eq!(hidden[0], SettingsPane::General);
    }

    #[test]
    fn pane_icons_are_embedded_assets() {
        use gpui::AssetSource;
        for pane in SettingsPane::ALL {
            let path = pane.icon_path();
            assert!(
                crate::assets::FrondaAssets.load(path).is_ok_and(|a| a.is_some()),
                "{path} must be embedded in FrondaAssets"
            );
        }
    }

    #[test]
    fn skill_matches_is_case_insensitive_over_name_and_description() {
        assert!(skill_matches("", "Captions", "burn in"));
        assert!(skill_matches("  ", "Captions", "burn in"));
        assert!(skill_matches("cap", "Captions", "burn in"));
        assert!(skill_matches("CAP", "Captions", "burn in"));
        assert!(skill_matches("BURN", "Captions", "burn in captions"));
        assert!(!skill_matches("montage", "Captions", "burn in"));
    }

    // Swift AppTheme.Settings values (#319).
    #[test]
    fn settings_metrics_mirror_swift_theme() {
        assert_eq!(SettingsMetrics::SIDEBAR_WIDTH, 220.0);
        assert_eq!(SettingsMetrics::CONTENT_MAX_WIDTH, 640.0);
        assert_eq!(SettingsMetrics::SKILLS_SEARCH_WIDTH, 260.0);
        assert_eq!(SettingsMetrics::SKILL_ROW_ICON_FRAME, 42.0);
        assert_eq!(SettingsMetrics::SKILL_STATUS_WIDTH, 124.0);
        assert_eq!(SettingsMetrics::SKILL_ACTION_WIDTH, 72.0);
        assert_eq!(SettingsMetrics::SKILL_DETAIL_WIDTH, 720.0);
        assert_eq!(SettingsMetrics::SKILL_DETAIL_MIN_HEIGHT, 600.0);
    }
}
